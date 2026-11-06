use std::path::{Path, PathBuf};
use std::process::Command;
use structure_core::impact::{FileChange, changed_lines};

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
    String::from_utf8_lossy(&o.stdout).into_owned()
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
fn working_tree_commits_and_subdirectory() {
    let d = temp_repo("impact");
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("a.txt"), "1\n2\n3\n4\n").unwrap();
    std::fs::write(d.join("sub/b.txt"), "x\ny\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "one"]);

    std::fs::write(d.join("a.txt"), "1\nTWO\n3\n4\n").unwrap();
    std::fs::write(d.join("sub/new.txt"), "p\nq\nr\n").unwrap();

    let wt = sorted(changed_lines(&d, None, None).unwrap());
    assert_eq!(
        wt,
        vec![fc("a.txt", &[(1, 2)]), fc("sub/new.txt", &[(0, 4)])]
    );

    // Project is a subdirectory: only its files, relative paths.
    let sub = sorted(changed_lines(&d.join("sub"), None, None).unwrap());
    assert_eq!(sub, vec![fc("new.txt", &[(0, 4)])]);

    git(&d, &["add", "."]);
    git(&d, &["commit", "-qm", "two"]);
    let r = sorted(changed_lines(&d, Some("HEAD~1"), Some("HEAD")).unwrap());
    assert_eq!(
        r,
        vec![fc("a.txt", &[(1, 2)]), fc("sub/new.txt", &[(0, 3)])]
    );
    let r = changed_lines(&d.join("sub"), Some("HEAD~1"), Some("HEAD")).unwrap();
    assert_eq!(r, vec![fc("new.txt", &[(0, 3)])]);
    assert!(changed_lines(&d, Some("HEAD"), None).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&d);
}
