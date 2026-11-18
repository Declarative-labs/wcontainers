//! The baseline's history in git, so that it can only shrink. Every line the baseline gained after it
//! started is reported with the commit that added it, unless a merge brought it from the other side,
//! where the same rule is checked back to the merge base. These changes may add lines: the commit that
//! starts the file on a first-parent chain; when that commit is a merge (the file arrived from another
//! branch), the next change, which is the branch's `init`; and a change made where the checker's own
//! source changed since the previous change (new rules), or the change after a merge that brought new
//! rules. Every such change, on every side, is listed by every run, so that none passes unseen.

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
/// A diff as plain lines whatever the attributes say, so that marking the file binary hides nothing.
const PLAIN_DIFF: &[&str] = &[
    "--text",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--no-renames",
];
/// Every commit that adds the file, on every side of every merge.
const FULL_HISTORY: &str = "--full-history";
const EACH_PARENT: &str = "-m";
const ADDED_FILTER: &str = "--diff-filter=A";
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
const IS_ANCESTOR: &str = "--is-ancestor";
const SHOW: &str = "show";
const CAT_FILE: &str = "cat-file";
const EXISTS: &str = "-e";
/// `REV:./PATH` names a file relative to the folder git runs in.
const IN_REVISION: &str = ":./";
const TRUE: &str = "true";
const CHECK_ATTRIBUTES: &[&str] = &["check-attr", "--all", "--"];
/// The branch this one pushes to and pulls from.
const UPSTREAM: &str = "@{upstream}";
const NO_UPSTREAM: &str = "no upstream branch to compare it with";
/// How a diff starts, and how its hunks do (`@@` for one parent, one `@` more per extra parent).
const DIFF_PREFIX: &str = "diff ";
const HUNK_MARK: char = '@';
const ADDED: char = '+';
const REMOVED: char = '-';
/// Where uncommitted lines come from, in a report.
pub const WORKING_TREE: &str = "the working tree";
const SHORT_COMMIT: usize = 12;
const DIFF: &str = "diff";
const QUIET: &str = "--quiet";
/// Why a change may add lines.
const AT_START: &str = "the start";
const AFTER_ARRIVAL: &str = "the init after the baseline arrived by a merge";
const NEW_RULES: &str = "the checker's rules changed";
const AFTER_NEW_RULES: &str = "the change after a merge that brought new rules";
const SHALLOW: &str = "the git history is shallow, so the baseline's growth cannot be checked: fetch the whole history (git fetch --unshallow)";

pub struct History {
    /// Every change, on every side, that was allowed to add lines, with why, oldest first per chain.
    pub additions: Vec<String>,
    /// Lines added after the start that no parent had, each with the commit that added it.
    pub grown: Vec<(String, String)>,
    /// What is wrong with the history as a whole, each with what to do.
    pub problems: Vec<(String, &'static str)>,
    /// Lines removed at some point since the start, except by a change that could also add lines.
    pub removed: BTreeSet<String>,
    /// Where the baseline started on this branch.
    pub start: String,
    /// Init may still write the baseline: it has not started here, or it arrived by a merge and has not
    /// changed since.
    pub may_init: bool,
}

pub fn run(root: &Path, args: &[&str]) -> Result<Output, String> {
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
    /// The checker's own source folder, relative to the root, whose changes are new rules.
    rules: Option<&'a str>,
    additions: Vec<String>,
    seen: HashSet<(String, Option<String>)>,
    grown: Vec<(String, String)>,
    removed: BTreeSet<String>,
}

/// What a first-parent chain says about its own start.
#[derive(Default)]
struct Chain {
    changes: usize,
    arrived: bool,
    /// The changes allowed to add lines, with why.
    allowed: Vec<(String, &'static str)>,
    /// The last change, and whether a change after it may add lines (it was a merge with new rules).
    last: Option<String>,
    after_new_rules: bool,
}

const ATTRIBUTES: &str = "remove every attribute of literals.baseline from .gitattributes (and .git/info/attributes): attributes can hide its history";
const STARTED_TWICE: &str = "keep the one baseline that started on the literals branch: merge that branch, run init once, and never start another";
const START_MOVED: &str = "rebuild the branch on top of its upstream without replacing the baseline's start: the start never changes once pushed";

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

    /// Every commit reachable from `tips` that made the file when none of its parents had it, and no
    /// earlier such commit is its ancestor.
    fn origins(&self, tips: &[&str]) -> Result<BTreeSet<String>, String> {
        let mut args = vec![LOG, FULL_HISTORY, EACH_PARENT, ADDED_FILTER, CHANGE_FORMAT];
        args.extend(tips);
        args.extend([PATHS, self.rel]);
        let mut origins = BTreeSet::new();
        for line in text(self.root, &args)?.lines() {
            let mut ids = line.split_whitespace();
            let Some(commit) = ids.next() else { continue };
            let mut made = true;
            for parent in ids {
                made &= !self.has_file(parent)?;
            }
            if made {
                origins.insert(commit.to_string());
            }
        }
        // A start after another one (the file deleted and added again) is growth on its own chain.
        let mut roots = BTreeSet::new();
        for o in &origins {
            let mut later = false;
            for other in origins.iter().filter(|other| *other != o) {
                later |= run(self.root, &[MERGE_BASE, IS_ANCESTOR, other, o])?
                    .status
                    .success();
            }
            if !later {
                roots.insert(o.clone());
            }
        }
        Ok(roots)
    }

    /// The checker's source differs between a commit and a later one, or the working tree.
    fn rules_differ(&self, from: Option<&str>, to: Option<&str>) -> Result<bool, String> {
        let (Some(rules), Some(from)) = (self.rules, from) else {
            return Ok(false);
        };
        let mut args = vec![DIFF, QUIET, from];
        args.extend(to);
        args.extend([PATHS, rules]);
        Ok(!run(self.root, &args)?.status.success())
    }

    /// Where the file starts on a chain of changes down to `stop`, and which changes may add lines.
    fn start(&self, changes: &[Change], stop: Option<&str>) -> Result<Chain, String> {
        let started = match changes.first() {
            Some(first) => match first.parents.first() {
                Some(parent) => !self.has_file(parent)?,
                None => true,
            },
            None => false,
        };
        let arrived = started && changes.first().is_some_and(|c| c.parents.len() > 1);
        let mut allowed = Vec::new();
        let mut previous = stop.map(String::from);
        let mut after_new_rules = false;
        for (i, change) in changes.iter().enumerate() {
            let new_rules = self.rules_differ(previous.as_deref(), Some(&change.commit))?;
            let why = if started && i == usize::default() {
                Some(AT_START)
            } else if arrived && i == 1 {
                Some(AFTER_ARRIVAL)
            } else if new_rules {
                Some(NEW_RULES)
            } else if after_new_rules {
                Some(AFTER_NEW_RULES)
            } else {
                None
            };
            if let Some(why) = why {
                allowed.push((change.commit.clone(), why));
            }
            after_new_rules = new_rules && change.parents.len() > 1;
            previous = Some(change.commit.clone());
        }
        Ok(Chain {
            changes: changes.len(),
            arrived,
            allowed,
            last: previous.filter(|_| !changes.is_empty()),
            after_new_rules,
        })
    }

    /// Checks the chain from `tip` down to `stop`, and every other side merged into it.
    fn chain(&mut self, tip: &str, stop: Option<&str>) -> Result<Chain, String> {
        if !self.seen.insert((tip.to_string(), stop.map(String::from))) {
            return Ok(Chain::default());
        }
        let changes = self.changes(tip, stop)?;
        let diffs = self.diffs(tip, stop)?;
        let chain = self.start(&changes, stop)?;
        self.additions.extend(
            chain
                .allowed
                .iter()
                .map(|(c, why)| format!("{} {why}", short(c))),
        );
        for change in &changes {
            let exempt = chain.allowed.iter().any(|(c, _)| *c == change.commit);
            if let Some((_, diff)) = diffs.iter().find(|(c, _)| *c == change.commit) {
                if !exempt {
                    self.grown.extend(
                        diff.added
                            .iter()
                            .map(|l| (l.clone(), short(&change.commit))),
                    );
                }
                // A change that may rewrite the baseline may also drop lines new rules no longer need.
                if !exempt {
                    self.removed.extend(diff.removed.iter().cloned());
                }
            }
            for other in change.parents.iter().skip(1) {
                let base = match change.parents.first() {
                    Some(first) => self.merge_base(first, other)?,
                    None => None,
                };
                self.chain(other, base.as_deref())?;
            }
        }
        Ok(chain)
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
/// `rules` is the checker's own source folder, relative to `root`, when it is in the tree.
pub fn read(
    root: &Path,
    rel: &str,
    rules: Option<&str>,
    working: &Keys,
) -> Result<History, String> {
    let shallow = text(root, &[REV_PARSE, IS_SHALLOW])?;
    if shallow.trim() == TRUE {
        return Err(SHALLOW.to_string());
    }
    let mut walker = Walker {
        root,
        rel,
        rules,
        additions: Vec::new(),
        seen: HashSet::new(),
        grown: Vec::new(),
        removed: BTreeSet::new(),
    };
    let mut problems = Vec::new();
    let attributes = text(root, &[CHECK_ATTRIBUTES, &[rel]].concat())?;
    if !attributes.trim().is_empty() {
        let found: Vec<&str> = attributes.lines().map(str::trim).collect();
        problems.push((
            format!(
                "{rel} has git attributes: {}",
                found.join(crate::NOTE_LIST_SEPARATOR)
            ),
            ATTRIBUTES,
        ));
    }
    let mut verify = vec![REV_PARSE];
    verify.extend(VERIFY);
    let head = run(root, &[verify.as_slice(), &[HEAD]].concat())?
        .status
        .success();
    let chain = if head {
        walker.chain(HEAD, None)?
    } else {
        Chain::default()
    };
    let committed = match head {
        true => walker.version(HEAD)?.map(|t| baseline::parse(&t)),
        false => None,
    };
    let fresh = chain.changes == usize::default();
    // The working tree may add lines like a change on top of the chain (an init with new rules).
    let working_rules = walker.rules_differ(chain.last.as_deref(), None)?;
    let may_init =
        fresh || (chain.arrived && chain.changes == 1) || working_rules || chain.after_new_rules;
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
    if !may_init {
        walker.removed.extend(multiset_minus(&committed, working));
    }
    let tips: Vec<&str> = [(head, HEAD), (merging, MERGE_HEAD)]
        .into_iter()
        .filter_map(|(present, tip)| present.then_some(tip))
        .collect();
    let origins = match tips.is_empty() {
        true => BTreeSet::new(),
        false => walker.origins(&tips)?,
    };
    if origins.len() > 1 {
        let all: Vec<String> = origins.iter().map(|c| short(c)).collect();
        problems.push((
            format!(
                "{rel} was started more than once, by {}: lines a second start brings are not checked",
                all.join(crate::NOTE_LIST_SEPARATOR)
            ),
            STARTED_TWICE,
        ));
    }
    let upstream = run(root, &[verify.as_slice(), &[UPSTREAM]].concat())?;
    let upstream = upstream
        .status
        .success()
        .then(|| String::from_utf8_lossy(&upstream.stdout).trim().to_string());
    let compared = match &upstream {
        Some(up) => {
            let theirs = walker.start(&walker.changes(up, None)?, None)?.allowed;
            for (commit, why) in theirs
                .iter()
                .filter(|(t, _)| !chain.allowed.iter().any(|(c, _)| c == t))
            {
                problems.push((
                    format!(
                        "{rel} may grow at {} ({why}) on its upstream but not on this branch: the start was replaced",
                        short(commit)
                    ),
                    START_MOVED,
                ));
            }
            format!("compared with its upstream {}", short(up))
        }
        None => NO_UPSTREAM.to_string(),
    };
    let start = if fresh {
        format!("{rel} has no history on this branch yet, so every line is its start")
    } else if chain.arrived && chain.changes == 1 {
        format!(
            "{rel} arrived by a merge and has not changed since: run init once, then it only shrinks"
        )
    } else if may_init {
        format!(
            "the checker's rules changed since {rel} last changed: this change may add lines (run init), then it only shrinks ({compared})"
        )
    } else {
        format!(
            "{rel} has {} changes on this branch's first-parent history ({compared}); growth is checked",
            chain.changes
        )
    };
    let mut additions = Vec::new();
    for a in walker.additions {
        if !additions.contains(&a) {
            additions.push(a);
        }
    }
    Ok(History {
        additions,
        grown: walker.grown,
        problems,
        removed: walker.removed,
        start,
        may_init,
    })
}
