//! Textual diff of a commit, a range or the working tree, for display.

use anyhow::bail;
use std::io::Read;
use std::path::Path;
use structure_ir::model::{Diff, DiffLine, FileDiff, FileStatus, Hunk, LineKind};

use crate::impact::{check_rev, clean_path, git, untracked};

/// Payload bounds: a diff of a large refactoring or a vendored directory
/// would otherwise be tens of MB. Counts (`additions`/`deletions`) stay exact.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub files: usize,
    pub lines_per_file: usize,
    pub lines: usize,
    pub line_chars: usize,
    /// Bytes read from one untracked file (it is not in git's output).
    pub untracked_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            files: 1000,
            lines_per_file: 2000,
            lines: 20_000,
            line_chars: 1000,
            untracked_bytes: 1 << 20,
        }
    }
}

/// Diff of `base..head` relative to `project`, with the same `base`/`head`
/// meaning as [`crate::impact::changed_lines`]: no base = HEAD vs working tree
/// (untracked files included as added), no head = base vs working tree.
pub fn diff(project: &Path, base: Option<&str>, head: Option<&str>) -> anyhow::Result<Diff> {
    diff_with(project, base, head, Limits::default())
}

pub fn diff_with(
    project: &Path,
    base: Option<&str>,
    head: Option<&str>,
    limits: Limits,
) -> anyhow::Result<Diff> {
    let mut args = vec![
        "diff",
        "--relative",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "-M",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ];
    for rev in base.iter().chain(head.iter()) {
        check_rev(rev)?;
    }
    match (base, head) {
        (None, Some(_)) => bail!("head requires base"),
        (None, None) => args.push("HEAD"),
        (Some(b), None) => args.push(b),
        (Some(b), Some(h)) => {
            args.push(b);
            args.push(h);
        }
    }
    // Revisions end here: a revision named like a file is not read as a path.
    args.push("--");
    let mut budget = Budget::new(limits);
    let mut files = parse_patch(&git(project, &args)?, &mut budget);
    if base.is_none() {
        add_untracked(project, &mut files, &mut budget)?;
    }
    Ok(Diff {
        base: base.unwrap_or("HEAD").to_string(),
        head: head.map(str::to_string),
        files,
        truncated: budget.truncated,
    })
}

/// What is left of the [`Limits`] while building a diff.
struct Budget {
    limits: Limits,
    files: usize,
    lines: usize,
    truncated: bool,
}

impl Budget {
    fn new(limits: Limits) -> Self {
        Budget {
            limits,
            files: 0,
            lines: 0,
            truncated: false,
        }
    }

    /// Room for one more file; flags the diff as truncated otherwise.
    fn take_file(&mut self) -> bool {
        if self.files < self.limits.files {
            self.files += 1;
            true
        } else {
            self.truncated = true;
            false
        }
    }

    /// Room for one more line of `file` (which holds `in_file` lines already).
    fn take_line(&mut self, file: &mut FileDiff, in_file: usize) -> bool {
        if in_file < self.limits.lines_per_file && self.lines < self.limits.lines {
            self.lines += 1;
            true
        } else {
            file.truncated = true;
            self.truncated = true;
            false
        }
    }

    fn clip(&self, text: &str) -> String {
        match text.char_indices().nth(self.limits.line_chars) {
            Some((i, _)) => format!("{}…", &text[..i]),
            None => text.to_string(),
        }
    }
}

/// One file of the patch being parsed, with its position counters.
struct Cur {
    file: FileDiff,
    /// Header lines say so; a file over the file limit is parsed but dropped.
    keep: bool,
    old_path: Option<String>,
    lines: usize,
    old_no: u32,
    new_no: u32,
}

fn new_file(path: String) -> FileDiff {
    FileDiff {
        path,
        old_path: None,
        status: FileStatus::Modified,
        binary: false,
        additions: 0,
        deletions: 0,
        hunks: Vec::new(),
        truncated: false,
    }
}

fn finish(cur: Option<Cur>, out: &mut Vec<FileDiff>) {
    if let Some(mut c) = cur
        && c.keep
    {
        if c.file.status == FileStatus::Renamed {
            c.file.old_path = c.old_path;
        }
        if !c.file.path.is_empty() {
            out.push(c.file);
        }
    }
}

/// Parse a full `git diff` patch (prefixes `a/` and `b/`).
fn parse_patch(text: &str, budget: &mut Budget) -> Vec<FileDiff> {
    let mut out = Vec::new();
    let mut cur: Option<Cur> = None;
    let mut in_hunk = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            finish(cur.take(), &mut out);
            in_hunk = false;
            let keep = budget.take_file();
            cur = Some(Cur {
                file: new_file(git_header_path(rest).unwrap_or_default()),
                keep,
                old_path: None,
                lines: 0,
                old_no: 0,
                new_no: 0,
            });
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        if line.starts_with("@@ ") {
            in_hunk = true;
            let (old_no, new_no) = hunk_starts(line).unwrap_or((0, 0));
            c.old_no = old_no;
            c.new_no = new_no;
            if c.keep {
                c.file.hunks.push(Hunk {
                    header: line.to_string(),
                    lines: Vec::new(),
                });
            }
        } else if in_hunk {
            let (kind, text) = match line.as_bytes().first() {
                Some(b'+') => (LineKind::Add, &line[1..]),
                Some(b'-') => (LineKind::Del, &line[1..]),
                Some(b' ') => (LineKind::Context, &line[1..]),
                // "\ No newline at end of file", or an empty context line.
                Some(b'\\') => continue,
                _ => (LineKind::Context, ""),
            };
            let (old, new) = match kind {
                LineKind::Add => (None, Some(c.new_no)),
                LineKind::Del => (Some(c.old_no), None),
                LineKind::Context => (Some(c.old_no), Some(c.new_no)),
            };
            match kind {
                LineKind::Add => {
                    c.file.additions += 1;
                    c.new_no += 1;
                }
                LineKind::Del => {
                    c.file.deletions += 1;
                    c.old_no += 1;
                }
                LineKind::Context => {
                    c.old_no += 1;
                    c.new_no += 1;
                }
            }
            if c.keep && budget.take_line(&mut c.file, c.lines) {
                c.lines += 1;
                let text = budget.clip(text);
                if let Some(h) = c.file.hunks.last_mut() {
                    h.lines.push(DiffLine {
                        kind,
                        old,
                        new,
                        text,
                    });
                }
            }
        } else if line.starts_with("new file mode") {
            c.file.status = FileStatus::Added;
        } else if line.starts_with("deleted file mode") {
            c.file.status = FileStatus::Deleted;
        } else if let Some(p) = line.strip_prefix("rename from ") {
            c.file.status = FileStatus::Renamed;
            c.old_path = Some(clean_path(p).to_string());
        } else if let Some(p) = line.strip_prefix("rename to ") {
            c.file.status = FileStatus::Renamed;
            c.file.path = clean_path(p).to_string();
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            c.file.binary = true;
        } else if let Some(p) = line.strip_prefix("--- ") {
            if let Some(old) = clean_path(p).strip_prefix("a/") {
                c.old_path = Some(old.to_string());
            }
        } else if let Some(p) = line.strip_prefix("+++ ") {
            match clean_path(p).strip_prefix("b/") {
                Some(new) => c.file.path = new.to_string(),
                None => {
                    if let Some(old) = &c.old_path {
                        c.file.path = old.clone();
                    }
                }
            }
        }
    }
    finish(cur, &mut out);
    out
}

/// Path from `a/P b/P` (both sides equal, which holds unless the file was
/// renamed, and then `rename to` gives the path). Quoted paths are unquoted naively.
fn git_header_path(rest: &str) -> Option<String> {
    let rest = rest.trim_end_matches('\t');
    if let Some(r) = rest.strip_prefix("\"a/") {
        let (a, _) = r.split_once("\" ")?;
        return Some(a.to_string());
    }
    let n = rest.len();
    if n.is_multiple_of(2) {
        return None;
    }
    let (a, b) = (rest.get(..n / 2)?, rest.get(n / 2 + 1..)?);
    let p = a.strip_prefix("a/")?;
    (b.strip_prefix("b/")? == p).then(|| p.to_string())
}

/// First old and new line numbers of `@@ -a[,b] +c[,d] @@`.
fn hunk_starts(line: &str) -> Option<(u32, u32)> {
    let mut it = line.split_whitespace().skip(1);
    let start = |t: &str, sign: char| -> Option<u32> {
        let t = t.strip_prefix(sign)?;
        t.split(',').next()?.parse().ok()
    };
    let old = start(it.next()?, '-')?;
    let new = start(it.next()?, '+')?;
    // An empty side (`-0,0` of an added file) starts at 0; no line uses it, but keep numbers 1-based.
    Some((old.max(1), new.max(1)))
}

/// Untracked, non-ignored files under `project`, as added files. Only regular
/// files are read (not symlinks, directories or nested repositories), and at
/// most `Limits::untracked_bytes` of each.
fn add_untracked(
    project: &Path,
    out: &mut Vec<FileDiff>,
    budget: &mut Budget,
) -> anyhow::Result<()> {
    for rel in untracked(project)? {
        if !budget.take_file() {
            break;
        }
        let path = project.join(&rel);
        let mut f = new_file(rel);
        f.status = FileStatus::Added;
        let regular = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file());
        let mut bytes = Vec::new();
        if regular {
            let cap = budget.limits.untracked_bytes;
            std::fs::File::open(&path)?
                .take(cap + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > cap {
                bytes.truncate(cap as usize);
                f.truncated = true;
                budget.truncated = true;
            }
        }
        if !regular || bytes.iter().take(8000).any(|&b| b == 0) {
            f.binary = true;
            out.push(f);
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let mut lines = Vec::new();
        for (i, l) in text.lines().enumerate() {
            f.additions += 1;
            if budget.take_line(&mut f, lines.len()) {
                lines.push(DiffLine {
                    kind: LineKind::Add,
                    old: None,
                    new: Some(i as u32 + 1),
                    text: budget.clip(l),
                });
            }
        }
        if f.additions > 0 {
            f.hunks.push(Hunk {
                header: format!("@@ -0,0 +1,{} @@", f.additions),
                lines,
            });
        }
        out.push(f);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<FileDiff> {
        parse_patch(text, &mut Budget::new(Limits::default()))
    }

    const MODIFY: &str = "\
diff --git a/src/A.java b/src/A.java
index 1111111..2222222 100644
--- a/src/A.java
+++ b/src/A.java
@@ -1,3 +1,3 @@ class A
 class A {
-  int x;
+  long x;
 }
@@ -10,0 +11,2 @@
+-- not a header
++++ not a header
";

    #[test]
    fn modify_with_line_numbers_and_lookalikes() {
        let f = parse(MODIFY);
        assert_eq!(f.len(), 1);
        let a = &f[0];
        assert_eq!(a.path, "src/A.java");
        assert_eq!(a.status, FileStatus::Modified);
        assert_eq!((a.additions, a.deletions), (3, 1));
        assert_eq!(a.hunks.len(), 2);
        let h = &a.hunks[0];
        assert_eq!(h.header, "@@ -1,3 +1,3 @@ class A");
        let nums: Vec<_> = h.lines.iter().map(|l| (l.kind, l.old, l.new)).collect();
        assert_eq!(
            nums,
            vec![
                (LineKind::Context, Some(1), Some(1)),
                (LineKind::Del, Some(2), None),
                (LineKind::Add, None, Some(2)),
                (LineKind::Context, Some(3), Some(3)),
            ]
        );
        assert_eq!(h.lines[2].text, "  long x;");
        let h2 = &a.hunks[1];
        assert_eq!(h2.lines[0].new, Some(11));
        assert_eq!(h2.lines[0].text, "-- not a header");
        assert_eq!(h2.lines[1].text, "+++ not a header");
    }

    #[test]
    fn add_delete_rename_binary() {
        let text = "\
diff --git a/N.java b/N.java
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/N.java
@@ -0,0 +1 @@
+class N {}
\\ No newline at end of file
diff --git a/D.java b/D.java
deleted file mode 100644
index 4444444..0000000
--- a/D.java
+++ /dev/null
@@ -1,2 +0,0 @@
-class D {
-}
diff --git a/old/R.java b/new/R.java
similarity index 90%
rename from old/R.java
rename to new/R.java
index 5555555..6666666 100644
--- a/old/R.java
+++ b/new/R.java
@@ -1 +1 @@
-class R {}
+class R { }
diff --git a/img.png b/img.png
index 7777777..8888888 100644
Binary files a/img.png and b/img.png differ
diff --git a/M.java b/M.java
old mode 100644
new mode 100755
";
        let f = parse(text);
        let summary: Vec<_> = f
            .iter()
            .map(|x| {
                (
                    x.path.as_str(),
                    x.status,
                    x.binary,
                    x.additions,
                    x.deletions,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("N.java", FileStatus::Added, false, 1, 0),
                ("D.java", FileStatus::Deleted, false, 0, 2),
                ("new/R.java", FileStatus::Renamed, false, 1, 1),
                ("img.png", FileStatus::Modified, true, 0, 0),
                ("M.java", FileStatus::Modified, false, 0, 0),
            ]
        );
        assert_eq!(f[2].old_path.as_deref(), Some("old/R.java"));
        assert_eq!(f[0].old_path, None);
        assert_eq!(f[1].hunks[0].lines[1].old, Some(2));
        assert_eq!(f[0].hunks[0].lines.len(), 1);
    }

    #[test]
    fn header_path_handles_spaces_and_quotes() {
        assert_eq!(
            git_header_path("a/x y.txt b/x y.txt").as_deref(),
            Some("x y.txt")
        );
        assert_eq!(
            git_header_path("\"a/\\303\\251.txt\" \"b/\\303\\251.txt\"").as_deref(),
            Some("\\303\\251.txt")
        );
        assert_eq!(git_header_path("a/old b/new"), None);
    }

    #[test]
    fn limits_truncate_lines_and_files_but_keep_counts() {
        let limits = Limits {
            files: 1,
            lines_per_file: 2,
            lines: 100,
            line_chars: 4,
            untracked_bytes: 100,
        };
        let mut b = Budget::new(limits);
        let two = format!(
            "{MODIFY}diff --git a/B.java b/B.java\n--- a/B.java\n+++ b/B.java\n@@ -1 +1 @@\n-a\n+b\n"
        );
        let f = parse_patch(&two, &mut b);
        assert!(b.truncated);
        assert_eq!(f.len(), 1);
        assert!(f[0].truncated);
        assert_eq!((f[0].additions, f[0].deletions), (3, 1));
        let shown: usize = f[0].hunks.iter().map(|h| h.lines.len()).sum();
        assert_eq!(shown, 2);
        assert_eq!(f[0].hunks[0].lines[0].text, "clas…");
    }

    #[test]
    fn total_line_budget_is_shared_across_files() {
        let limits = Limits {
            files: 10,
            lines_per_file: 100,
            lines: 3,
            line_chars: 100,
            untracked_bytes: 100,
        };
        let mut b = Budget::new(limits);
        let two = format!(
            "{MODIFY}diff --git a/B.java b/B.java\n--- a/B.java\n+++ b/B.java\n@@ -1 +1 @@\n-a\n+b\n"
        );
        let f = parse_patch(&two, &mut b);
        assert!(b.truncated);
        let shown: Vec<usize> = f
            .iter()
            .map(|x| x.hunks.iter().map(|h| h.lines.len()).sum())
            .collect();
        assert_eq!(shown, vec![3, 0]);
        assert!(f[0].truncated && f[1].truncated);
        assert_eq!((f[1].additions, f[1].deletions), (1, 1));
    }

    #[test]
    fn pure_rename_takes_its_paths_from_the_rename_lines() {
        let f = parse(
            "diff --git a/x y.java b/z.java\nsimilarity index 100%\nrename from x y.java\nrename to z.java\n",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(
            (f[0].path.as_str(), f[0].old_path.as_deref()),
            ("z.java", Some("x y.java"))
        );
        assert_eq!(f[0].status, FileStatus::Renamed);
        assert!(f[0].hunks.is_empty());
    }

    fn git_ok(dir: &Path, args: &[&str]) -> String {
        let cfg = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ];
        let all: Vec<&str> = cfg.iter().chain(args.iter()).copied().collect();
        git(dir, &all).unwrap()
    }

    #[test]
    fn diff_of_a_commit_and_of_the_working_tree() {
        let dir = std::env::temp_dir().join(format!("structure-git-diff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app")).unwrap();
        git_ok(&dir, &["init", "-q"]);
        let f = dir.join("app/A.java");
        std::fs::write(&f, "class A {\n}\n").unwrap();
        std::fs::write(dir.join("Outside.txt"), "x\n").unwrap();
        git_ok(&dir, &["add", "."]);
        git_ok(&dir, &["commit", "-q", "-m", "c0"]);
        std::fs::write(&f, "class A {\n  int x;\n}\n").unwrap();
        std::fs::write(dir.join("Outside.txt"), "y\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "c1"]);
        let root = dir.join("app");

        let d = diff(&root, Some("HEAD~1"), Some("HEAD")).unwrap();
        assert_eq!(d.head.as_deref(), Some("HEAD"));
        assert!(!d.truncated);
        assert_eq!(
            d.files.len(),
            1,
            "--relative keeps files under the project only"
        );
        assert_eq!(d.files[0].path, "A.java");
        assert_eq!((d.files[0].additions, d.files[0].deletions), (1, 0));

        std::fs::write(root.join("New.java"), "class N {}\n").unwrap();
        std::fs::write(root.join("bin.dat"), [0u8, 1, 2]).unwrap();
        let w = diff(&root, None, None).unwrap();
        assert_eq!(w.base, "HEAD");
        let paths: Vec<_> = w
            .files
            .iter()
            .map(|x| (x.path.as_str(), x.status, x.binary))
            .collect();
        assert_eq!(
            paths,
            vec![
                ("New.java", FileStatus::Added, false),
                ("bin.dat", FileStatus::Added, true)
            ]
        );
        assert_eq!(w.files[0].hunks[0].lines[0].text, "class N {}");
        assert!(diff(&root, None, Some("HEAD")).is_err());
        assert!(diff(&root, Some("--output=x"), None).is_err());
        assert!(!root.join("x").exists());

        // Untracked limits: bytes, lines and empty files.
        std::fs::remove_file(root.join("bin.dat")).unwrap();
        std::fs::write(root.join("big.txt"), "l1\nl2\nl3\nl4\nl5\n").unwrap();
        std::fs::write(root.join("empty.txt"), "").unwrap();
        std::fs::remove_file(root.join("New.java")).unwrap();
        let limits = Limits {
            files: 10,
            lines_per_file: 2,
            lines: 100,
            line_chars: 100,
            untracked_bytes: 9,
        };
        let w = diff_with(&root, None, None, limits).unwrap();
        assert!(w.truncated);
        let big = &w.files[0];
        assert_eq!(big.path, "big.txt");
        assert!(big.truncated);
        assert_eq!(big.additions, 3, "only the first 9 bytes are read");
        assert_eq!(big.hunks[0].lines.len(), 2);
        let empty = &w.files[1];
        assert_eq!(
            (empty.path.as_str(), empty.additions, empty.hunks.len()),
            ("empty.txt", 0, 0)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
