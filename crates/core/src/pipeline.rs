//! End-to-end pipeline: detect language, index (SCIP), parse, link,
//! classify, build the matrix, compute impact.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::classify::{RuleSet, classify};
use crate::impact;
use crate::ir::{Graph, Language};
use crate::link::{LinkStats, link};
use crate::matrix;
use crate::model::{Classification, Commit, History, Impact, Matrix, Progress};
use crate::{java, scip, ts};

pub const SCIP_JAVA_VERSION: &str = "0.12.3";
pub const SCIP_TS_VERSION: &str = "0.6.1";

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Always re-run the indexer.
    pub reindex: bool,
    /// Re-run the indexer when a source file is newer than `index.scip`.
    /// When false, a stale index is reused with a warning.
    pub reindex_if_stale: bool,
    pub use_scip: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            reindex: false,
            reindex_if_stale: true,
            use_scip: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub graph: Graph,
    pub classification: Classification,
    pub matrix: Matrix,
    pub link_stats: Option<LinkStats>,
    pub warnings: Vec<String>,
}

pub fn detect_language(root: &Path) -> anyhow::Result<Language> {
    let has = |n: &str| root.join(n).is_file();
    if has("pom.xml")
        || has("build.gradle")
        || has("build.gradle.kts")
        || has("settings.gradle")
        || has("settings.gradle.kts")
    {
        Ok(Language::Java)
    } else if has("tsconfig.json") || has("package.json") {
        Ok(Language::TypeScript)
    } else {
        bail!(
            "cannot detect language in {}: expected pom.xml/build.gradle/build.gradle.kts/settings.gradle/settings.gradle.kts (Java) or tsconfig.json/package.json (TypeScript)",
            root.display()
        )
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

fn find_cs() -> Option<PathBuf> {
    find_on_path("cs").or_else(|| {
        let p = PathBuf::from(std::env::var_os("HOME")?).join(".local/bin/cs");
        p.is_file().then_some(p)
    })
}

/// Reports `(pct, detail)` while the indexer runs.
type IndexProgress<'a> = &'a mut dyn FnMut(u8, String);

const INDEX_PCT_START: u8 = 5;
const INDEX_PCT_CAP: u8 = 34;
const DETAIL_MAX_CHARS: usize = 120;

/// Strips ANSI escapes, trims, and drops blank or download-noise lines;
/// caps the length. `None` when the line is not worth showing.
fn clean_line(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
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
        } else if c != '\r' {
            out.push(c);
        }
    }
    let line = out.trim();
    if line.is_empty() || line.contains("Download") || line.contains("Progress (") {
        return None;
    }
    Some(if line.chars().count() > DETAIL_MAX_CHARS {
        let cut: String = line.chars().take(DETAIL_MAX_CHARS - 1).collect();
        format!("{cut}…")
    } else {
        line.to_string()
    })
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
struct IndexPct {
    last: u8,
    reactor: Option<(u32, u32)>,
}

impl IndexPct {
    fn update(&mut self, line: Option<&str>, elapsed_secs: u64) -> u8 {
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

fn spawn_line_reader<R: std::io::Read + Send + 'static>(
    r: R,
    tx: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut r = std::io::BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf) {
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

fn run_indexer(root: &Path, mut cmd: Command, progress: IndexProgress) -> Result<(), String> {
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
    let mut readers = Vec::new();
    if let Some(s) = child.stdout.take() {
        readers.push(spawn_line_reader(s, tx.clone()));
    }
    if let Some(s) = child.stderr.take() {
        readers.push(spawn_line_reader(s, tx.clone()));
    }
    drop(tx);
    let mut tail: VecDeque<String> = VecDeque::new();
    let mut keep = |line: &str| {
        if !line.trim().is_empty() {
            if tail.len() == 8 {
                tail.pop_front();
            }
            tail.push_back(line.trim_end().to_string());
        }
    };
    let mut pct = IndexPct::default();
    let mut last_line = String::from("starting indexer");
    let mut last_emit: Option<Instant> = None;
    let mut disconnected = false;
    let deadline = started + Duration::from_secs(INDEX_TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "timed out after {} min; check ~/.m2/settings.xml (unreachable repository?) or run the indexer manually",
                    INDEX_TIMEOUT_SECS / 60
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
    if status.success() {
        return Ok(());
    }
    for r in readers {
        let _ = r.join();
    }
    for raw in rx.try_iter() {
        keep(&raw);
    }
    Err(format!(
        "{} ({status}); for Java also check ~/.m2/settings.xml",
        tail.iter().cloned().collect::<Vec<_>>().join(" | ")
    ))
}

const INDEX_TIMEOUT_SECS: u64 = 600;

/// Number of source files modified after `index` was written.
fn newer_sources(root: &Path, lang: Language, index: &Path) -> usize {
    let Ok(indexed) = std::fs::metadata(index).and_then(|m| m.modified()) else {
        return 0;
    };
    let exts: &[&str] = match lang {
        Language::Java => &["java"],
        Language::TypeScript => &["ts", "tsx"],
    };
    ignore::WalkBuilder::new(root)
        // `.gradle` is explicit: keeps it out even if `.hidden(false)` is set later.
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_str(),
                Some("node_modules" | "target" | "build" | "dist" | ".git" | ".gradle")
            )
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| exts.contains(&x))
        })
        .filter(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| t > indexed)
        })
        .count()
}

/// Returns the path of an up-to-date `index.scip`, or `Ok(None)` (with a
/// warning through `warn`) if it could not be produced.
pub fn ensure_index(
    root: &Path,
    lang: Language,
    opts: &Options,
    warn: &mut dyn FnMut(String),
) -> anyhow::Result<Option<PathBuf>> {
    ensure_index_with_progress(root, lang, opts, warn, &mut |_, _| {})
}

/// Like [`ensure_index`], reporting `(pct, detail)` while the indexer runs.
pub fn ensure_index_with_progress(
    root: &Path,
    lang: Language,
    opts: &Options,
    warn: &mut dyn FnMut(String),
    progress: &mut dyn FnMut(u8, String),
) -> anyhow::Result<Option<PathBuf>> {
    let index = root.join("index.scip");
    if index.is_file() && !opts.reindex {
        let stale = newer_sources(root, lang, &index);
        if stale == 0 {
            return Ok(Some(index));
        }
        if !opts.reindex_if_stale {
            warn(format!(
                "index.scip is older than {stale} source file(s); call edges in those files may be off until the project is re-analyzed"
            ));
            return Ok(Some(index));
        }
    }
    let cmd = match lang {
        Language::Java => {
            if let Some(p) = find_on_path("scip-java") {
                let mut c = Command::new(p);
                c.args(["index", "--output", "index.scip"]);
                Some(c)
            } else if let Some(cs) = find_cs() {
                let mut c = Command::new(cs);
                c.arg("launch")
                    .arg(format!(
                        "com.sourcegraph:scip-java_2.13:{SCIP_JAVA_VERSION}"
                    ))
                    .args(["--", "index", "--output", "index.scip"]);
                Some(c)
            } else {
                None
            }
        }
        Language::TypeScript => find_on_path("npx").map(|p| {
            let mut c = Command::new(p);
            c.args(["-y"])
                .arg(format!("@sourcegraph/scip-typescript@{SCIP_TS_VERSION}"))
                .args(["index", "--output", "index.scip"]);
            c
        }),
    };
    let Some(cmd) = cmd else {
        warn("no SCIP indexer found (scip-java/cs/npx); using tree-sitter only".into());
        return Ok(None);
    };
    match run_indexer(root, cmd, progress) {
        Ok(()) if index.is_file() => Ok(Some(index)),
        Ok(()) => {
            warn("indexer succeeded but produced no index.scip; using tree-sitter only".into());
            Ok(None)
        }
        Err(e) => {
            warn(format!("indexing failed, using tree-sitter only: {e}"));
            Ok(None)
        }
    }
}

fn project_name(root: &Path) -> String {
    let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    canon
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into())
}

pub fn analyze(
    root: &Path,
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<Analysis> {
    let started = Instant::now();
    let step = |s: &str, pct: u8| {
        progress(Progress {
            step: s.into(),
            pct,
            detail: None,
        })
    };
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let lang = detect_language(root)?;
    let mut warnings: Vec<String> = Vec::new();

    step("indexing", 5);
    let index_path = if opts.use_scip {
        ensure_index_with_progress(
            root,
            lang,
            opts,
            &mut |w| warnings.push(w),
            &mut |pct, detail| {
                progress(Progress {
                    step: "indexing".into(),
                    pct,
                    detail: Some(detail),
                })
            },
        )?
    } else {
        None
    };

    step("parsing", 35);
    let graph = match lang {
        Language::Java => java::parse_project(root),
        Language::TypeScript => ts::parse_project(root),
    }
    .with_context(|| format!("parsing {}", root.display()))?;

    step("linking", 60);
    let (graph, link_stats) = match index_path {
        Some(p) => match scip::load(&p) {
            Ok(idx) => {
                let (g, st) = link(graph, &idx);
                (g, Some(st))
            }
            Err(e) => {
                warnings.push(format!(
                    "cannot read {}: {e:#}; using tree-sitter only",
                    p.display()
                ));
                (graph, None)
            }
        },
        None => (graph, None),
    };

    step("classifying", 80);
    let mut rules = RuleSet::builtin(lang)?;
    let custom = root.join(".structure/rules.toml");
    if custom.is_file() {
        let text = std::fs::read_to_string(&custom)
            .with_context(|| format!("reading {}", custom.display()))?;
        let user = RuleSet::from_toml_str(&text)
            .with_context(|| format!("parsing {}", custom.display()))?;
        rules = rules.merge(user);
    }
    let classification = classify(&graph, &rules);

    step("building matrix", 95);
    let elapsed = started.elapsed().as_millis() as u64;
    let mut matrix = matrix::build(&graph, &classification, &project_name(root), elapsed);
    matrix.warnings = warnings.clone();
    step("done", 100);

    Ok(Analysis {
        graph,
        classification,
        matrix,
        link_stats,
        warnings,
    })
}

/// Recent commits reachable from HEAD (`--topo-order`, newest first) plus the dirty flag.
pub fn history(root: &Path, limit: u32) -> anyhow::Result<History> {
    const FMT: &str = "--format=%H%x1f%h%x1f%P%x1f%an%x1f%aI%x1f%D%x1f%s%x1e";
    let n = format!("-n{}", limit.max(1));
    let log = impact::git(
        root,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "log",
            "--no-show-signature",
            "--no-color",
            "--topo-order",
            &n,
            FMT,
            "HEAD",
            "--",
        ],
    )
    .context("cannot read history (not a git repository, or no commits yet)")?;
    let mut empty_tree: Option<String> = None;
    let mut commits = Vec::new();
    for rec in log.split('\x1e').map(|r| r.trim_start_matches('\n')) {
        if rec.is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.splitn(7, '\x1f').collect();
        let [id, short, parents, author, date, refs, subject] = f[..] else {
            bail!("unexpected log record: {rec:?}");
        };
        let parents: Vec<String> = parents.split_whitespace().map(str::to_string).collect();
        let base = match parents.first() {
            Some(p) => p.clone(),
            None => match &empty_tree {
                Some(t) => t.clone(),
                None => {
                    let t = impact::git(root, &["hash-object", "-t", "tree", "/dev/null"])?
                        .trim()
                        .to_string();
                    empty_tree = Some(t.clone());
                    t
                }
            },
        };
        commits.push(Commit {
            id: id.to_string(),
            short: short.to_string(),
            parents,
            base,
            subject: subject.to_string(),
            author: author.to_string(),
            date: date.to_string(),
            refs: refs
                .split(", ")
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }
    if commits.is_empty() {
        bail!("no commits in this repository");
    }
    // Same changes as `impact(None, None)`: tracked edits and non-ignored
    // untracked files under the project root, minus the index we write.
    let dirty = impact::git(root, &["status", "--porcelain", "-uall", "--", "."])?
        .lines()
        .any(|l| {
            let p = l.get(3..).unwrap_or("").trim_matches('"');
            p != "index.scip" && !p.ends_with("/index.scip")
        });
    Ok(History { dirty, commits })
}

pub fn impact(
    root: &Path,
    analysis: &Analysis,
    base: Option<&str>,
    head: Option<&str>,
) -> anyhow::Result<Impact> {
    let mut changes = impact::changed_lines(root, base, head)?;
    // The SCIP index we write into the project is a build artefact, not a change.
    changes.retain(|c| c.file != "index.scip");
    let mut result = impact::compute(
        &analysis.graph,
        &analysis.classification,
        &changes,
        base.unwrap_or("HEAD"),
        head,
    );
    result.warnings = analysis.warnings.clone();
    if let Some(h) = head {
        result.warnings.push(format!(
            "symbol positions come from the working tree; results for {h} are exact only if it is checked out"
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

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
    }

    fn touch(p: &Path, t: SystemTime) {
        std::fs::write(p, "x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn gradle_settings_detected_as_java() {
        for f in ["settings.gradle", "settings.gradle.kts"] {
            let dir =
                std::env::temp_dir().join(format!("structure-detect-{}-{f}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(f), "").unwrap();
            assert!(matches!(detect_language(&dir).unwrap(), Language::Java));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn build_gradle_kts_detected_as_java() {
        let dir = std::env::temp_dir().join(format!("structure-detect-kts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("build.gradle.kts"), "").unwrap();
        assert!(matches!(detect_language(&dir).unwrap(), Language::Java));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_language_error_lists_gradle_files() {
        let dir = std::env::temp_dir().join(format!("structure-detect-err-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let msg = detect_language(&dir).unwrap_err().to_string();
        assert!(msg.contains("settings.gradle"), "{msg}");
        assert!(msg.contains("settings.gradle.kts"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gradle_dir_not_counted_as_stale() {
        let dir =
            std::env::temp_dir().join(format!("structure-gradle-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".gradle/x")).unwrap();
        let t0 = SystemTime::now() - Duration::from_secs(100);
        let index = dir.join("index.scip");
        touch(&index, t0);
        touch(&dir.join(".gradle/x/A.java"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::Java, &index), 0);
        touch(&dir.join("B.java"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::Java, &index), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_index_detection_and_warning() {
        let dir = std::env::temp_dir().join(format!("structure-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        let t0 = SystemTime::now() - Duration::from_secs(100);
        let index = dir.join("index.scip");
        touch(&dir.join("src/old.ts"), t0);
        touch(&index, t0 + Duration::from_secs(10));
        touch(
            &dir.join("node_modules/dep.ts"),
            t0 + Duration::from_secs(50),
        );
        touch(&dir.join("src/notes.md"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::TypeScript, &index), 0);

        touch(&dir.join("src/new.ts"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::TypeScript, &index), 1);

        // Stale index reused with a warning when re-indexing is not wanted.
        let opts = Options {
            reindex_if_stale: false,
            ..Options::default()
        };
        let mut warnings = vec![];
        let got = ensure_index(&dir, Language::TypeScript, &opts, &mut |w| warnings.push(w));
        assert_eq!(got.unwrap(), Some(index));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("older than 1 source file"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
