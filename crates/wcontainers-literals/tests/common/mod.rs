//! Small workspaces written to the test build folder, scanned, and read back finding by finding.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// One printed finding.
#[derive(Clone, Debug)]
pub struct Row {
    pub list: String,
    pub path: String,
    pub line: usize,
    pub item: String,
    pub literal: String,
    pub text: String,
}

/// The lists that fail the check.
pub const GATED: &[&str] = &[
    "code",
    "redefinition",
    "restated",
    "role",
    "script",
    "unparsed",
    "mark",
];

impl Row {
    pub fn gated(&self) -> bool {
        GATED.contains(&self.list.as_str())
    }
}

/// Writes a workspace of `files` (path, text) under the test build folder, replacing any earlier one.
pub fn workspace(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove the old workspace");
    }
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("a file has a folder"))
            .expect("make the folder");
        std::fs::write(&path, text).expect("write the file");
    }
    root
}

pub fn rows(lines: &[String]) -> Vec<Row> {
    lines
        .iter()
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let (path, rest) = f[1].split_once(':').expect("path:line:column");
            let line = rest.split(':').next().unwrap().parse().unwrap();
            Row {
                list: f[0].into(),
                path: path.into(),
                line,
                item: f[3].into(),
                literal: f[4].into(),
                text: l.clone(),
            }
        })
        .collect()
}

/// Every finding in the workspace.
pub fn list(root: &Path) -> Vec<Row> {
    rows(
        &wcontainers_literals::list(root)
            .expect("list the workspace")
            .lines,
    )
}

/// The findings whose item is `item` or sits inside it.
pub fn in_item<'a>(rows: &'a [Row], item: &str) -> Vec<&'a Row> {
    rows.iter()
        .filter(|r| {
            r.item == item
                || r.item
                    .split(wcontainers_literals::PATH_SEPARATOR)
                    .any(|p| p == item)
                || r.item.ends_with(&format!(" {item}"))
        })
        .collect()
}

/// The options every git command of the tests takes: who commits.
const IDENTITY: &[&str] = &[
    "-c",
    "user.email=test@example.invalid",
    "-c",
    "user.name=test",
    "-c",
    "commit.gpgsign=false",
];

/// Runs git with the tests' identity.
pub fn git_output(root: &Path, args: &[&str]) -> std::process::Output {
    wcontainers_literals::git(root, &[IDENTITY, args].concat()).expect("run git")
}

pub fn git(root: &Path, args: &[&str]) -> String {
    let out = git_output(root, args);
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A git repository holding `files` on branch main, committed.
pub fn repository(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = workspace(name, files);
    git(&root, &["init", "-q", "-b", "main"]);
    commit_all(&root, "start");
    root
}

pub fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "--allow-empty", "-m", message]);
}

pub fn append(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    let mut old = std::fs::read_to_string(&path).unwrap_or_default();
    old.push_str(text);
    std::fs::write(path, old).expect("append");
}

pub fn replace(root: &Path, path: &str, from: &str, to: &str) {
    let path = root.join(path);
    let old = std::fs::read_to_string(&path).expect("read");
    assert!(old.contains(from), "{from} is not in {}", path.display());
    std::fs::write(path, old.replacen(from, to, 1)).expect("write");
}

pub fn check(root: &Path) -> wcontainers_literals::Outcome {
    wcontainers_literals::check(root).expect("run the check")
}
