use std::path::{Path, PathBuf};
use std::process::Command;
use structure_core::impact::changed_lines;
use structure_core::pipeline::history;

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn temp_repo(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("structure-hist-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    git(&d, &["config", "user.name", "Tester"]);
    git(&d, &["config", "user.email", "t@example.com"]);
    git(&d, &["config", "commit.gpgsign", "false"]);
    d
}

#[test]
fn non_git_dir_errors() {
    let d = std::env::temp_dir().join(format!("structure-hist-nogit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    // GIT_CEILING_DIRECTORIES cannot be set in-process without `unsafe`; the
    // system temp dir is not inside a repository.
    let e = history(&d, 10).unwrap_err();
    assert!(format!("{e:#}").contains("not a git repository"), "{e:#}");
}

#[test]
fn dirty_ignores_index_scip_only() {
    let d = temp_repo("dirty");
    std::fs::write(d.join("a.txt"), "1\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "one"]);
    std::fs::write(d.join("index.scip"), "x").unwrap();
    std::fs::write(d.join(".gitignore"), "ignored.txt\n").unwrap();
    git(&d, &["add", ".gitignore"]);
    git(&d, &["commit", "-qm", "ignore"]);
    std::fs::write(d.join("ignored.txt"), "x").unwrap();
    assert!(!history(&d, 5).unwrap().dirty);
    std::fs::write(d.join("new.txt"), "x").unwrap();
    assert!(history(&d, 5).unwrap().dirty);
    std::fs::remove_file(d.join("new.txt")).unwrap();
    std::fs::write(d.join("a.txt"), "2\n").unwrap();
    assert!(history(&d, 5).unwrap().dirty);
}

#[test]
fn repo_without_commits_errors() {
    let d = temp_repo("empty");
    assert!(history(&d, 10).is_err());
}

#[test]
fn root_merge_dirty_and_odd_subject() {
    let d = temp_repo("main");
    std::fs::write(d.join("a.txt"), "1\n2\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "root"]);
    git(&d, &["checkout", "-q", "-b", "side"]);
    std::fs::write(d.join("b.txt"), "b\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "side"]);
    git(&d, &["checkout", "-q", "main"]);
    std::fs::write(d.join("a.txt"), "1\n2\n3\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "main work"]);
    git(
        &d,
        &[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "tab\there | \x1f? ,x",
            "side",
        ],
    );

    let h = history(&d, 50).unwrap();
    assert!(!h.dirty);
    assert_eq!(h.commits.len(), 4);
    let merge = &h.commits[0];
    assert_eq!(merge.parents.len(), 2);
    assert_eq!(merge.base, merge.parents[0]);
    assert!(
        merge.subject.starts_with("tab\there | "),
        "{:?}",
        merge.subject
    );
    assert_eq!(merge.author, "Tester");
    assert!(merge.refs.iter().any(|r| r.contains("main")));
    assert!(merge.id.starts_with(&merge.short));

    let root = h.commits.last().unwrap();
    assert!(root.parents.is_empty());
    let changes = changed_lines(&d, Some(&root.base), Some(&root.id)).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file, "a.txt");

    assert_eq!(history(&d, 2).unwrap().commits.len(), 2);

    std::fs::write(d.join("a.txt"), "changed\n").unwrap();
    assert!(history(&d, 5).unwrap().dirty);
}

#[test]
fn topo_order_keeps_branches_contiguous() {
    let d = temp_repo("topo");
    let commit = |file: &str, msg: &str| {
        std::fs::write(d.join(file), msg).unwrap();
        git(&d, &["add", "."]);
        git(&d, &["commit", "-qm", msg]);
    };
    commit("r.txt", "root");
    git(&d, &["checkout", "-q", "-b", "side"]);
    commit("s1.txt", "side1");
    git(&d, &["checkout", "-q", "main"]);
    commit("m1.txt", "main1");
    git(&d, &["checkout", "-q", "side"]);
    commit("s2.txt", "side2");
    git(&d, &["checkout", "-q", "main"]);
    commit("m2.txt", "main2");
    git(&d, &["merge", "-q", "--no-ff", "-m", "merge", "side"]);

    let h = history(&d, 50).unwrap();
    let subjects: Vec<&str> = h.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects.len(), 6);
    assert_eq!(subjects[0], "merge");
    assert_eq!(subjects[5], "root");
    // Each merged line of history is contiguous, between the merge and the fork point.
    let middle = &subjects[1..5];
    let main_first = middle == ["main2", "main1", "side2", "side1"];
    let side_first = middle == ["side2", "side1", "main2", "main1"];
    assert!(main_first || side_first, "{subjects:?}");
}
