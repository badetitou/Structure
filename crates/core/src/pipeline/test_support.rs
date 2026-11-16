use std::path::{Path, PathBuf};

use crate::impact;

use super::Options;

pub(crate) fn git_ok(dir: &Path, args: &[&str]) -> String {
    let cfg = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "-c",
        "commit.gpgsign=false",
    ];
    let all: Vec<&str> = cfg.iter().chain(args.iter()).copied().collect();
    impact::git(dir, &all).unwrap()
}

/// Repo with a Java project in `app/`. c0: class A (line 1); c1 adds class B
/// (line 2); c2 prepends a comment line, so B moves to line 3.
pub(crate) fn temp_repo(tag: &str) -> (PathBuf, String, String) {
    let dir = std::env::temp_dir().join(format!("structure-snap-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("app/src")).unwrap();
    git_ok(&dir, &["init", "-q"]);
    std::fs::write(dir.join("app/pom.xml"), "<project/>").unwrap();
    let f = dir.join("app/src/A.java");
    std::fs::write(&f, "class A { void a() {} }\n").unwrap();
    git_ok(&dir, &["add", "."]);
    git_ok(&dir, &["commit", "-q", "-m", "c0"]);
    std::fs::write(&f, "class A { void a() {} }\nclass B { void b() {} }\n").unwrap();
    git_ok(&dir, &["commit", "-q", "-am", "c1"]);
    let c1 = git_ok(&dir, &["rev-parse", "HEAD"]).trim().to_string();
    std::fs::write(
        &f,
        "// h\nclass A { void a() {} }\nclass B { void b() {} }\n",
    )
    .unwrap();
    git_ok(&dir, &["commit", "-q", "-am", "c2"]);
    let c2 = git_ok(&dir, &["rev-parse", "HEAD"]).trim().to_string();
    (dir, c1, c2)
}

pub(crate) fn no_scip() -> Options {
    Options {
        use_scip: false,
        ..Options::default()
    }
}
