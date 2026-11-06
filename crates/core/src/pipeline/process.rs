use std::path::Path;
use std::process::Command;
use std::time::Instant;

use super::progress::{INDEX_PCT_START, IndexPct, IndexProgress, cap_chars, clean_line, visible};

const MAX_LINE_BYTES: usize = 64 * 1024;
const TAIL_LINE_CHARS: usize = 300;

/// Reads up to and including `\n` into `buf`, keeping at most `max` bytes and
/// discarding the rest of the line. Returns the bytes consumed (0 at EOF).
fn read_capped_line<R: std::io::BufRead>(
    r: &mut R,
    buf: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<usize> {
    let mut total = 0;
    loop {
        let chunk = r.fill_buf()?;
        if chunk.is_empty() {
            return Ok(total);
        }
        let (used, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        let room = max.saturating_sub(buf.len());
        buf.extend_from_slice(&chunk[..used.min(room)]);
        r.consume(used);
        total += used;
        if done {
            return Ok(total);
        }
    }
}

fn spawn_line_reader<R: std::io::Read + Send + 'static>(
    r: R,
    tx: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut r = std::io::BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match read_capped_line(&mut r, &mut buf, MAX_LINE_BYTES) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

const DRAIN_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

/// Feeds pending lines to `f` until the channel closes or `grace` elapses in total.
fn drain_grace(
    rx: &std::sync::mpsc::Receiver<String>,
    grace: std::time::Duration,
    f: &mut dyn FnMut(&str),
) {
    let end = Instant::now() + grace;
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(raw) => f(&raw),
            Err(_) => break,
        }
    }
}

pub(super) fn run_indexer(
    root: &Path,
    mut cmd: Command,
    timeout_secs: u64,
    progress: IndexProgress,
    hint: Option<&str>,
) -> Result<(), String> {
    use std::collections::VecDeque;
    use std::process::Stdio;
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use std::time::Duration;
    let started = Instant::now();
    let mut child = cmd
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {:?}: {e}", cmd.get_program()))?;
    let (tx, rx) = channel::<String>();
    if let Some(s) = child.stdout.take() {
        spawn_line_reader(s, tx.clone());
    }
    if let Some(s) = child.stderr.take() {
        spawn_line_reader(s, tx.clone());
    }
    drop(tx);
    let mut tail: VecDeque<String> = VecDeque::new();
    let mut keep = |line: &str| {
        let v = visible(line);
        if !v.is_empty() {
            if tail.len() == 8 {
                tail.pop_front();
            }
            tail.push_back(cap_chars(&v, TAIL_LINE_CHARS));
        }
    };
    let mut pct = IndexPct::default();
    let mut last_line = String::from("starting indexer");
    let mut last_emit: Option<Instant> = None;
    let mut disconnected = false;
    let deadline = (timeout_secs > 0).then(|| started + Duration::from_secs(timeout_secs));
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if deadline.is_some_and(|d| Instant::now() >= d) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "indexer timed out after {timeout_secs} s; raise the limit with --indexer-timeout <secs> or STRUCTURE_INDEXER_TIMEOUT (0 = none); {}",
                    hint.unwrap_or("or run the indexer manually")
                ));
            }
            Ok(None) => {}
            Err(e) => return Err(format!("waiting for indexer: {e}")),
        }
        let mut cleaned: Option<String> = None;
        if disconnected {
            std::thread::sleep(Duration::from_millis(100));
        } else {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(raw) => {
                    keep(&raw);
                    cleaned = clean_line(&raw);
                    if let Some(c) = &cleaned {
                        last_line = c.clone();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => disconnected = true,
            }
        }
        let elapsed = started.elapsed().as_secs();
        let p = pct.update(cleaned.as_deref(), elapsed);
        if last_emit.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
            last_emit = Some(Instant::now());
            progress(p, format!("{last_line} · {elapsed}s"));
        }
    };
    // Do not join the readers: a forked daemon may keep the pipes open forever.
    drain_grace(&rx, DRAIN_GRACE, &mut |raw| {
        keep(raw);
        if let Some(c) = clean_line(raw) {
            last_line = c;
        }
    });
    if status.success() {
        progress(
            pct.last.max(INDEX_PCT_START),
            format!("{last_line} · {}s", started.elapsed().as_secs()),
        );
        return Ok(());
    }
    let tail = tail.iter().cloned().collect::<Vec<_>>().join(" | ");
    Err(match hint {
        Some(h) => format!("{tail} ({status}); {h}"),
        None => format!("{tail} ({status})"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn drain_grace_returns_when_sender_stays_open() {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        tx.send("a\n".into()).unwrap();
        let mut got = Vec::new();
        let t = Instant::now();
        drain_grace(&rx, Duration::from_millis(100), &mut |l| {
            got.push(l.to_string())
        });
        assert_eq!(got, ["a\n"]);
        assert!(t.elapsed() < Duration::from_secs(5));
        drop(tx);
    }

    #[test]
    fn read_capped_line_bounds_buffer() {
        let data = format!("{}\nok\n", "y".repeat(200_000));
        let mut r = std::io::BufReader::with_capacity(16, data.as_bytes());
        let mut buf = Vec::new();
        assert_eq!(read_capped_line(&mut r, &mut buf, 1000).unwrap(), 200_001);
        assert_eq!(buf.len(), 1000);
        buf.clear();
        assert_eq!(read_capped_line(&mut r, &mut buf, 1000).unwrap(), 3);
        assert_eq!(buf, b"ok\n");
        buf.clear();
        assert_eq!(read_capped_line(&mut r, &mut buf, 1000).unwrap(), 0);
    }
}
