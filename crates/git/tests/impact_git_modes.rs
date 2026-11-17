use std::path::{Path, PathBuf};
use std::process::Command;
use structure_git::impact::{FileChange, changed_lines};

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn temp_repo(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("structure-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q"]);
    git(&d, &["config", "user.name", "t"]);
    git(&d, &["config", "user.email", "t@example.com"]);
    git(&d, &["config", "commit.gpgsign", "false"]);
    d
}

fn fc(file: &str, lines: &[(u32, u32)]) -> FileChange {
    FileChange {
        file: file.into(),
        lines: lines.to_vec(),
    }
}

fn sorted(mut v: Vec<FileChange>) -> Vec<FileChange> {
    v.sort_by(|a, b| a.file.cmp(&b.file));
    v
}

#[test]
fn head_without_base_is_error() {
    let d = temp_repo("headerr");
    assert!(changed_lines(&d, None, Some("HEAD")).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn base_only_excludes_untracked_and_empty_untracked_registers() {
    let d = temp_repo("baseonly");
    std::fs::write(d.join("a.txt"), "1\n2\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "one"]);
    std::fs::write(d.join("a.txt"), "1\n2\n3\n").unwrap();
    git(&d, &["commit", "-qam", "two"]);
    std::fs::write(d.join("a.txt"), "1\n2\n3\n4\n").unwrap();
    std::fs::write(d.join("untracked.txt"), "u\n").unwrap();
    std::fs::write(d.join("empty.txt"), "").unwrap();
    let r = changed_lines(&d, Some("HEAD~1"), None).unwrap();
    assert_eq!(r, vec![fc("a.txt", &[(2, 4)])]);
    let r = sorted(changed_lines(&d, None, None).unwrap());
    assert_eq!(
        r,
        vec![
            fc("a.txt", &[(3, 4)]),
            fc("empty.txt", &[(0, 1)]),
            fc("untracked.txt", &[(0, 1)])
        ]
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn rename_delete_and_middle_deletion() {
    let d = temp_repo("renamedel");
    std::fs::create_dir_all(d.join("sub")).unwrap();
    let body = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\n";
    std::fs::write(d.join("old.txt"), body).unwrap();
    std::fs::write(d.join("gone.txt"), "x\n").unwrap();
    std::fs::write(d.join("sub/gone2.txt"), "y\n").unwrap();
    std::fs::write(d.join("sub/keep.txt"), "k\n").unwrap();
    std::fs::write(d.join("mid.txt"), body).unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "one"]);

    git(&d, &["mv", "old.txt", "new.txt"]);
    std::fs::write(d.join("new.txt"), body.replace("l5", "L5")).unwrap();
    git(&d, &["rm", "-q", "gone.txt", "sub/gone2.txt"]);
    std::fs::write(d.join("mid.txt"), body.replace("l4\n", "")).unwrap();

    let r = sorted(changed_lines(&d, None, None).unwrap());
    assert_eq!(
        r,
        vec![
            fc("gone.txt", &[]),
            fc("mid.txt", &[(3, 4)]),
            fc("new.txt", &[(4, 5)]),
            fc("sub/gone2.txt", &[]),
        ]
    );
    let r = changed_lines(&d.join("sub"), None, None).unwrap();
    assert_eq!(r, vec![fc("gone2.txt", &[])]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn untracked_scip_index_is_skipped_and_lines_counted_in_chunks() {
    let d = temp_repo("untrackedidx");
    std::fs::write(d.join("a.txt"), "1\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "one"]);
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("index.scip"), "x\n").unwrap();
    std::fs::write(d.join("sub/index.scip"), "x\n").unwrap();
    std::fs::write(d.join("index.java.scip"), "x\n").unwrap();
    std::fs::write(d.join("sub/index.typescript.scip"), "x\n").unwrap();
    // Larger than one read chunk, last line without '\n'.
    std::fs::write(d.join("big.txt"), format!("{}tail", "abc\n".repeat(20_000))).unwrap();
    std::fs::write(d.join("no_nl.txt"), "a\nb").unwrap();
    let r = sorted(changed_lines(&d, None, None).unwrap());
    assert_eq!(
        r,
        vec![fc("big.txt", &[(0, 20_001)]), fc("no_nl.txt", &[(0, 2)])]
    );
    let _ = std::fs::remove_dir_all(&d);
}
