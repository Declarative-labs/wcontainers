//! The baseline's history in git, so that it can only shrink. Every line the baseline gained after it
//! started is reported with the commit that added it, unless a merge brought it from the other side,
//! where the same rule is checked back to the merge base. Two changes may add lines: the commit that
//! starts the file on a first-parent chain, and, when that commit is a merge (the file arrived from
//! another branch), the next change, which is the branch's `init`.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::process::{Command, Output};

use crate::baseline::{self, Keys};

const GIT: &str = "git";
const GIT_IN: &str = "-C";
const LOG: &str = "log";
const FIRST_PARENT: &str = "--first-parent";
/// A merge's diff shows only lines that differ from every parent: lines it made up itself.
const COMBINED_DIFF: &str = "--diff-merges=dense-combined";
const PATCH: &str = "-p";
const NO_CONTEXT: &str = "-U0";
const PLAIN_DIFF: &[&str] = &[
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--no-renames",
];
const CHANGE_FORMAT: &str = "--format=%H %P";
const PATCH_FORMAT: &str = "--format=commit %H";
const COMMIT_PREFIX: &str = "commit ";
const PATHS: &str = "--";
const EXCLUDE_PREFIX: &str = "^";
const REV_PARSE: &str = "rev-parse";
const IS_SHALLOW: &str = "--is-shallow-repository";
const VERIFY: &[&str] = &["--verify", "--quiet"];
const HEAD: &str = "HEAD";
/// Present while a merge is being made.
const MERGE_HEAD: &str = "MERGE_HEAD";
const MERGE_BASE: &str = "merge-base";
const SHOW: &str = "show";
const CAT_FILE: &str = "cat-file";
const EXISTS: &str = "-e";
/// `REV:./PATH` names a file relative to the folder git runs in.
const IN_REVISION: &str = ":./";
const TRUE: &str = "true";
/// How a diff starts, and how its hunks do (`@@` for one parent, one `@` more per extra parent).
const DIFF_PREFIX: &str = "diff ";
const HUNK_MARK: char = '@';
const ADDED: char = '+';
const REMOVED: char = '-';
/// Where uncommitted lines come from, in a report.
pub const WORKING_TREE: &str = "the working tree";
const SHORT_COMMIT: usize = 12;
const SHALLOW: &str = "the git history is shallow, so the baseline's growth cannot be checked: fetch the whole history (git fetch --unshallow)";

pub struct History {
    /// Lines added after the start that no parent had, each with the commit that added it.
    pub grown: Vec<(String, String)>,
    /// Lines removed at some point since the start.
    pub removed: BTreeSet<String>,
    /// Where the baseline started on this branch.
    pub start: String,
    /// Init may still write the baseline: it has not started here, or it arrived by a merge and has not
    /// changed since.
    pub may_init: bool,
}

fn run(root: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new(GIT)
        .arg(GIT_IN)
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))
}

fn text(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = run(root, args)?;
    if !out.status.success() {
        return Err(format!(
            "{GIT} {args:?}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn short(commit: &str) -> String {
    commit.chars().take(SHORT_COMMIT).collect()
}

/// One change of the file on a first-parent chain.
struct Change {
    commit: String,
    parents: Vec<String>,
}

#[derive(Default)]
struct Diff {
    /// Lines in no parent.
    added: Vec<String>,
    removed: Vec<String>,
}

struct Walker<'a> {
    root: &'a Path,
    rel: &'a str,
    seen: HashSet<(String, Option<String>)>,
    grown: Vec<(String, String)>,
    removed: BTreeSet<String>,
}

/// What a first-parent chain says about its own start.
struct Chain {
    changes: usize,
    arrived: bool,
}

impl Walker<'_> {
    fn in_revision(&self, revision: &str) -> String {
        format!("{revision}{IN_REVISION}{}", self.rel)
    }

    fn has_file(&self, revision: &str) -> Result<bool, String> {
        Ok(
            run(self.root, &[CAT_FILE, EXISTS, &self.in_revision(revision)])?
                .status
                .success(),
        )
    }

    fn version(&self, revision: &str) -> Result<Option<String>, String> {
        let out = run(self.root, &[SHOW, &self.in_revision(revision)])?;
        Ok(out
            .status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned()))
    }

    fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>, String> {
        let out = run(self.root, &[MERGE_BASE, a, b])?;
        Ok(out
            .status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }

    fn range(tip: &str, stop: Option<&str>) -> Vec<String> {
        std::iter::once(tip.to_string())
            .chain(stop.map(|s| format!("{EXCLUDE_PREFIX}{s}")))
            .collect()
    }

    /// The commits on the chain from `tip` down to `stop` that change the file, oldest first.
    fn changes(&self, tip: &str, stop: Option<&str>) -> Result<Vec<Change>, String> {
        let range = Walker::range(tip, stop);
        let mut args = vec![LOG, FIRST_PARENT, CHANGE_FORMAT];
        args.extend(range.iter().map(String::as_str));
        args.extend([PATHS, self.rel]);
        let mut changes: Vec<Change> = text(self.root, &args)?
            .lines()
            .filter_map(|l| {
                let mut ids = l.split_whitespace().map(String::from);
                Some(Change {
                    commit: ids.next()?,
                    parents: ids.collect(),
                })
            })
            .collect();
        changes.reverse();
        Ok(changes)
    }

    /// Each change's diff against its first parent; a merge's against all its parents at once.
    fn diffs(&self, tip: &str, stop: Option<&str>) -> Result<Vec<(String, Diff)>, String> {
        let range = Walker::range(tip, stop);
        let mut args = vec![
            LOG,
            FIRST_PARENT,
            COMBINED_DIFF,
            PATCH,
            NO_CONTEXT,
            PATCH_FORMAT,
        ];
        args.extend(PLAIN_DIFF);
        args.extend(range.iter().map(String::as_str));
        args.extend([PATHS, self.rel]);
        let out = text(self.root, &args)?;
        let mut diffs: Vec<(String, Diff)> = Vec::new();
        let mut columns = None;
        for line in out.lines() {
            if let Some(commit) = line.strip_prefix(COMMIT_PREFIX) {
                diffs.push((commit.to_string(), Diff::default()));
                columns = None;
            } else if line.starts_with(DIFF_PREFIX) {
                columns = None;
            } else if line.starts_with(HUNK_MARK) {
                columns = line
                    .chars()
                    .take_while(|c| *c == HUNK_MARK)
                    .count()
                    .checked_sub(1);
            } else if let (Some(n), Some((_, diff))) = (columns, diffs.last_mut()) {
                let Some(prefix) = line.get(..n) else {
                    continue;
                };
                let content = line[n..].to_string();
                if prefix.chars().all(|c| c == ADDED) {
                    diff.added.push(content);
                } else if prefix.chars().all(|c| c == REMOVED) {
                    diff.removed.push(content);
                }
            }
        }
        Ok(diffs)
    }

    /// Checks the chain from `tip` down to `stop`, and every other side merged into it.
    fn chain(&mut self, tip: &str, stop: Option<&str>) -> Result<Chain, String> {
        if !self.seen.insert((tip.to_string(), stop.map(String::from))) {
            return Ok(Chain {
                changes: usize::default(),
                arrived: false,
            });
        }
        let changes = self.changes(tip, stop)?;
        let diffs = self.diffs(tip, stop)?;
        let started = match changes.first() {
            Some(first) => match first.parents.first() {
                Some(parent) => !self.has_file(parent)?,
                None => true,
            },
            None => false,
        };
        let arrived = started && changes.first().is_some_and(|c| c.parents.len() > 1);
        for (i, change) in changes.iter().enumerate() {
            let exempt = (started && i == usize::default()) || (arrived && i == 1);
            if let Some((_, diff)) = diffs.iter().find(|(c, _)| *c == change.commit) {
                if !exempt {
                    self.grown.extend(
                        diff.added
                            .iter()
                            .map(|l| (l.clone(), short(&change.commit))),
                    );
                }
                self.removed.extend(diff.removed.iter().cloned());
            }
            for other in change.parents.iter().skip(1) {
                let base = match change.parents.first() {
                    Some(first) => self.merge_base(first, other)?,
                    None => None,
                };
                self.chain(other, base.as_deref())?;
            }
        }
        Ok(Chain {
            changes: changes.len(),
            arrived,
        })
    }
}

fn multiset_minus(a: &Keys, b: &Keys) -> Vec<String> {
    let mut out = Vec::new();
    for (key, n) in a {
        for _ in b.get(key).copied().unwrap_or_default()..*n {
            out.push(key.clone());
        }
    }
    out
}

/// The baseline `rel` (relative to `root`) as it is in the working tree, against its history.
pub fn read(root: &Path, rel: &str, working: &Keys) -> Result<History, String> {
    let shallow = text(root, &[REV_PARSE, IS_SHALLOW])?;
    if shallow.trim() == TRUE {
        return Err(SHALLOW.to_string());
    }
    let mut walker = Walker {
        root,
        rel,
        seen: HashSet::new(),
        grown: Vec::new(),
        removed: BTreeSet::new(),
    };
    let mut verify = vec![REV_PARSE];
    verify.extend(VERIFY);
    let head = run(root, &[verify.as_slice(), &[HEAD]].concat())?
        .status
        .success();
    let chain = if head {
        walker.chain(HEAD, None)?
    } else {
        Chain {
            changes: usize::default(),
            arrived: false,
        }
    };
    let committed = match head {
        true => walker.version(HEAD)?.map(|t| baseline::parse(&t)),
        false => None,
    };
    let fresh = chain.changes == usize::default();
    let may_init = fresh || (chain.arrived && chain.changes == 1);
    let merging = run(root, &[verify.as_slice(), &[MERGE_HEAD]].concat())?
        .status
        .success();
    let mut from_merge = Keys::new();
    if merging {
        let base = walker.merge_base(HEAD, MERGE_HEAD)?;
        walker.chain(MERGE_HEAD, base.as_deref())?;
        from_merge = walker
            .version(MERGE_HEAD)?
            .map(|t| baseline::parse(&t))
            .unwrap_or_default();
    }
    let committed = committed.unwrap_or_default();
    if !may_init {
        let added = multiset_minus(working, &committed);
        walker.grown.extend(
            added
                .into_iter()
                .filter(|k| !from_merge.contains_key(k))
                .map(|k| (k, WORKING_TREE.to_string())),
        );
    }
    walker.removed.extend(multiset_minus(&committed, working));
    let start = if fresh {
        format!("{rel} has no history on this branch yet, so every line is its start")
    } else if may_init {
        format!(
            "{rel} arrived by a merge and has not changed since: run init once, then it only shrinks"
        )
    } else {
        format!(
            "{rel} has {} changes on this branch's first-parent history; growth is checked",
            chain.changes
        )
    };
    Ok(History {
        grown: walker.grown,
        removed: walker.removed,
        start,
        may_init,
    })
}
