/// Reports `(pct, detail)` while the indexer runs.
pub(super) type IndexProgress<'a> = &'a mut dyn FnMut(u8, String);

pub(super) const INDEX_PCT_START: u8 = 5;
const INDEX_PCT_CAP: u8 = 34;
const DETAIL_MAX_CHARS: usize = 120;

/// Strips ANSI escapes, trims, and drops blank or download-noise lines;
/// caps the length. `None` when the line is not worth showing.
pub(super) fn clean_line(raw: &str) -> Option<String> {
    let line = visible(raw);
    let rest = line.strip_prefix("[INFO] ").unwrap_or(&line);
    if line.is_empty()
        || rest.starts_with("Downloading from")
        || rest.starts_with("Downloaded from")
        || rest.starts_with("Progress (")
    {
        return None;
    }
    Some(cap_chars(&line, DETAIL_MAX_CHARS))
}

pub(super) fn cap_chars(line: &str, max: usize) -> String {
    if line.chars().count() > max {
        let cut: String = line.chars().take(max - 1).collect();
        format!("{cut}…")
    } else {
        line.to_string()
    }
}

/// Last non-empty `\r`-separated segment (terminal redraws), ANSI escapes
/// stripped, trimmed.
pub(super) fn visible(raw: &str) -> String {
    let seg = raw
        .rsplit('\r')
        .map(|s| s.trim_matches('\n'))
        .find(|s| !s.trim().is_empty())
        .unwrap_or("");
    let mut out = String::with_capacity(seg.len());
    let mut chars = seg.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// Maven reactor counter: `[INFO] Building foo 1.0 [3/12]` gives `(3, 12)`.
fn reactor_counter(line: &str) -> Option<(u32, u32)> {
    if !line.contains("Building ") {
        return None;
    }
    let inner = line.trim_end().strip_suffix(']')?;
    let (_, counter) = inner.rsplit_once('[')?;
    let (a, b) = counter.split_once('/')?;
    let (a, b) = (a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?);
    (b > 0 && a <= b).then_some((a, b))
}

/// Monotonic percentage inside the indexing band (5..=34).
#[derive(Debug, Default)]
pub(super) struct IndexPct {
    pub(super) last: u8,
    reactor: Option<(u32, u32)>,
}

impl IndexPct {
    pub(super) fn update(&mut self, line: Option<&str>, elapsed_secs: u64) -> u8 {
        if let Some(c) = line.and_then(reactor_counter) {
            self.reactor = Some(c);
        }
        let band = u64::from(INDEX_PCT_CAP - INDEX_PCT_START);
        let candidate = match self.reactor {
            // Module `a` has started: `a - 1` modules are done.
            Some((a, b)) => u64::from(a.saturating_sub(1)) * band / u64::from(b),
            // One point every 6 s.
            None => elapsed_secs / 6,
        };
        let pct = (u64::from(INDEX_PCT_START) + candidate).min(u64::from(INDEX_PCT_CAP)) as u8;
        self.last = self.last.max(pct);
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_line_filters_and_caps() {
        assert_eq!(clean_line("   \n"), None);
        assert_eq!(clean_line("[INFO] Downloading from central: x.pom"), None);
        assert_eq!(
            clean_line("\u{1b}[1m> Task :app:compileJava\u{1b}[0m\r\n").as_deref(),
            Some("> Task :app:compileJava")
        );
        let long = "x".repeat(500);
        assert_eq!(clean_line(&long).unwrap().chars().count(), DETAIL_MAX_CHARS);
    }

    #[test]
    fn clean_line_gradle_cr_and_noise() {
        assert_eq!(clean_line("> Task :x\n").as_deref(), Some("> Task :x"));
        assert_eq!(
            clean_line("> Task :a\r> Task :b\r\n").as_deref(),
            Some("> Task :b")
        );
        assert_eq!(clean_line("Downloaded from central: x"), None);
        assert_eq!(clean_line("Progress (1): 3/4 kB"), None);
        let l = "[INFO] Building Downloader 1.0 [3/12]";
        assert_eq!(clean_line(l).as_deref(), Some(l));
        assert_eq!(reactor_counter(l), Some((3, 12)));
    }

    #[test]
    fn reactor_counter_parses_maven_lines() {
        assert_eq!(
            reactor_counter("[INFO] Building foo 1.0 [3/12]"),
            Some((3, 12))
        );
        assert_eq!(reactor_counter("[INFO] Building foo 1.0"), None);
        assert_eq!(reactor_counter("> Task :x"), None);
        assert_eq!(reactor_counter("Building x [5/4]"), None);
    }

    #[test]
    fn index_pct_is_bounded_and_monotonic() {
        let mut p = IndexPct::default();
        assert_eq!(p.update(Some("> Task :x"), 0), 5);
        assert_eq!(p.update(None, 60), 15);
        let a = p.update(Some("[INFO] Building a 1 [6/12]"), 61);
        assert_eq!(a, 17);
        // Reactor says earlier module: never goes backwards.
        assert_eq!(p.update(Some("[INFO] Building b 1 [1/12]"), 62), 17);
        assert_eq!(p.update(Some("[INFO] Building z 1 [12/12]"), 63), 31);
        assert_eq!(p.update(None, 100_000), 31);
        let mut q = IndexPct::default();
        assert_eq!(q.update(None, 100_000), INDEX_PCT_CAP);
        // First reactor line after time creep does not go backwards.
        let mut r = IndexPct::default();
        assert_eq!(r.update(None, 120), 25);
        assert_eq!(r.update(Some("[INFO] Building a 1 [1/12]"), 121), 25);
    }
}
