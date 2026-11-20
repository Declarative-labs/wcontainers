//! The baseline's history in git, so that it can only shrink. Every line the baseline gained after it
//! started is reported with the commit that added it, unless a merge brought it from the other side,
//! where the same rule is checked back to the merge base. These changes may add lines: the commit that
//! starts the file on a first-parent chain; when that commit is a merge (the file arrived from another
//! branch), the next change, which is the branch's `init`; and a change that brings more versions of
//! the rules than its first parent has (unless that parent is an ancestor of another), or the commit
//! right after a merge that brought them, which may add only literals that were already in the tree of
//! one of its parents (for the commit after a merge, of one of that merge's parents). Every such
//! change, on every side, is listed by every run, so that none passes unseen.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use crate::baseline::{self, Keys};

const GIT: &str = "git";
const GIT_IN: &str = "-C";
/// Replace refs could turn any commit into another, such as a growth into the start.
const NO_REPLACE: &str = "--no-replace-objects";
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
const PARENTS_FORMAT: &str = "--format=%P";
const ONE: &str = "-1";
/// Shows the named commits only, not their history.
const NO_WALK: &str = "--no-walk=unsorted";
const BATCH: &str = "--batch";
const BLOB: &str = "blob";
const LINE_END: u8 = b'\n';
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
/// Why a change may add lines.
const AT_START: &str = "the start";
const AFTER_ARRIVAL: &str = "the init after the baseline arrived by a merge";
const NEW_RULES: &str = "new rules, for literals already in the tree";
/// Why a line added at a change of the rules is still growth.
const NOT_THERE_BEFORE: &str = "new rules, but the literal was not in the tree before the change";
/// Why a merge may join two starts.
const JOINED: &str = "joins starts made where tracks adopted the check, without growth";
const NO_CHECKER: &str =
    "the checker is not in the tree, so no start can be shown to be a track adopting it";
const REV_LIST: &str = "rev-list";
const MERGES: &str = "--merges";
const ANCESTRY_PATH: &str = "--ancestry-path";
const SHALLOW: &str = "the git history is shallow, so the baseline's growth cannot be checked: fetch the whole history (git fetch --unshallow)";

/// The keys of the literals in a file's text, given its path relative to the root.
pub type FileLiterals<'a> = Box<dyn Fn(&str, &str) -> HashSet<String> + 'a>;

/// What literals a file holds, to tell a literal that new rules found from one added with them.
pub struct Literals<'a> {
    /// The keys of the literals in a file's text, given its path relative to the root.
    pub of_file: FileLiterals<'a>,
    /// The path and key of the literal a baseline line stands for; none for a line that is not one.
    pub of_line: fn(&str) -> Option<(String, String)>,
    /// The file that counts the versions of the rules, relative to the root, when it is in the tree.
    pub rules: Option<String>,
}

pub struct History {
    /// Every change, on every side, that was allowed to add lines, with why, oldest first per chain.
    pub additions: Vec<String>,
    /// Lines added after the start that no parent had, each with the commit that added it and why
    /// that change could not.
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
        .arg(NO_REPLACE)
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
    literals: &'a Literals<'a>,
    additions: Vec<String>,
    seen: HashSet<(String, Option<String>)>,
    grown: Vec<(String, String)>,
    removed: BTreeSet<String>,
    /// The literal keys of a file at a revision.
    present: HashMap<(String, String), HashSet<String>>,
}

/// A change allowed to add lines: the commit, why, and for new rules the revisions whose trees must
/// already hold every literal it adds.
type Allowed = (String, &'static str, Option<Vec<String>>);

/// What a first-parent chain says about its own start.
#[derive(Default)]
struct Chain {
    changes: usize,
    arrived: bool,
    allowed: Vec<Allowed>,
}

const ATTRIBUTES: &str = "remove every attribute of this file from .gitattributes (and .git/info/attributes): attributes can hide its history";
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

    fn file_at(&self, revision: &str, rel: &str) -> Result<Option<String>, String> {
        let out = run(self.root, &[SHOW, &format!("{revision}{IN_REVISION}{rel}")])?;
        Ok(out
            .status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned()))
    }

    fn version(&self, revision: &str) -> Result<Option<String>, String> {
        self.file_at(revision, self.rel)
    }

    /// How many versions the rules have in the working tree.
    fn working_rules(&self) -> usize {
        self.literals
            .rules
            .as_ref()
            .and_then(|rules| std::fs::read_to_string(self.root.join(rules)).ok())
            .map_or(usize::default(), |t| crate::rules::version(&t))
    }

    fn parents(&self, revision: &str) -> Result<Vec<String>, String> {
        Ok(text(self.root, &[LOG, ONE, PARENTS_FORMAT, revision])?
            .split_whitespace()
            .map(String::from)
            .collect())
    }

    fn is_ancestor(&self, a: &str, b: &str) -> Result<bool, String> {
        Ok(run(self.root, &[MERGE_BASE, IS_ANCESTOR, a, b])?
            .status
            .success())
    }

    /// When a change with `version` versions of the rules on top of `parents` brings new rules, the
    /// parents whose trees must hold every literal it adds: all of them, since a merge can make a
    /// literal of either side newly gated. It brings them when it has more versions than its first
    /// parent, unless that parent is an ancestor of another: a merge of a branch into its own older
    /// commit brings nothing that branch did not already have.
    fn bases(
        &self,
        version: usize,
        parents: &[String],
        version_of: &dyn Fn(&str) -> usize,
    ) -> Result<Option<Vec<String>>, String> {
        let Some(first) = parents.first() else {
            return Ok(None);
        };
        if version <= version_of(first) {
            return Ok(None);
        }
        for other in parents.iter().skip(1) {
            if self.is_ancestor(first, other)? {
                return Ok(None);
            }
        }
        Ok(Some(parents.to_vec()))
    }

    /// The bases of a change that brings new rules (see `bases`), or of the commit right after a merge
    /// that brought them: that merge's parents, so that a literal written into the merge gets no line.
    fn bases_after(
        &self,
        version: usize,
        parents: &[String],
        merge_parents: Option<&Vec<String>>,
        version_of: &dyn Fn(&str) -> usize,
    ) -> Result<Option<Vec<String>>, String> {
        if let Some(bases) = self.bases(version, parents, version_of)? {
            return Ok(Some(bases));
        }
        match (parents.first(), merge_parents) {
            (Some(merge), Some(theirs)) if theirs.len() > 1 => {
                self.bases(version_of(merge), theirs, version_of)
            }
            _ => Ok(None),
        }
    }

    /// Each revision's parents, in one call.
    fn parents_of(&self, revisions: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
        if revisions.is_empty() {
            return Ok(HashMap::new());
        }
        let mut args = vec![LOG, NO_WALK, CHANGE_FORMAT];
        args.extend(revisions.iter().map(String::as_str));
        Ok(text(self.root, &args)?
            .lines()
            .filter_map(|l| {
                let mut ids = l.split_whitespace().map(String::from);
                Some((ids.next()?, ids.collect()))
            })
            .collect())
    }

    /// How many versions the rules have at each revision, read in one call; a revision without the
    /// rules file is left out.
    fn rules_of(&self, revisions: &[String]) -> Result<HashMap<String, usize>, String> {
        let Some(rules) = &self.literals.rules else {
            return Ok(HashMap::new());
        };
        let input: String = revisions
            .iter()
            .map(|r| format!("{r}{IN_REVISION}{rules}\n"))
            .collect();
        let mut child = Command::new(GIT)
            .arg(NO_REPLACE)
            .arg(GIT_IN)
            .arg(self.root)
            .args([CAT_FILE, BATCH])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run git: {e}"))?;
        // Written from another thread, so that a full output pipe cannot stall the input.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("{GIT} {CAT_FILE} has no input"))?;
        let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
        let out = child
            .wait_with_output()
            .map_err(|e| format!("{GIT} {CAT_FILE}: {e}"))?;
        writer
            .join()
            .map_err(|_| format!("{GIT} {CAT_FILE}: the input thread failed"))?
            .map_err(|e| format!("{GIT} {CAT_FILE}: {e}"))?;
        let mut versions = HashMap::new();
        let mut rest = out.stdout.as_slice();
        for revision in revisions {
            let Some(end) = rest.iter().position(|b| *b == LINE_END) else {
                break;
            };
            let header = String::from_utf8_lossy(&rest[..end]).into_owned();
            rest = &rest[end + 1..];
            let size = match header.split_whitespace().collect::<Vec<_>>().as_slice() {
                [_, BLOB, size] => size.parse::<usize>().ok(),
                _ => None,
            };
            if let Some(size) = size {
                let body = rest.get(..size).unwrap_or(rest);
                versions.insert(
                    revision.clone(),
                    crate::rules::version(&String::from_utf8_lossy(body)),
                );
                rest = rest.get(size + 1..).unwrap_or_default();
            }
        }
        Ok(versions)
    }

    /// The literal a baseline line stands for was in the tree at one of these revisions.
    fn was_there(&mut self, revisions: &[String], line: &str) -> Result<bool, String> {
        let Some((path, key)) = (self.literals.of_line)(line) else {
            return Ok(false);
        };
        for revision in revisions {
            let at = (revision.clone(), path.clone());
            if !self.present.contains_key(&at) {
                let keys = self
                    .file_at(revision, &path)?
                    .map(|text| (self.literals.of_file)(&path, &text))
                    .unwrap_or_default();
                self.present.insert(at.clone(), keys);
            }
            if self
                .present
                .get(&at)
                .is_some_and(|keys| keys.contains(&key))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The lines of `added` whose literal none of `revisions` held, each with `commit` and why.
    fn not_there(
        &mut self,
        revisions: &[String],
        added: &[String],
        commit: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for line in added {
            if !self.was_there(revisions, line)? {
                out.push((line.clone(), format!("{commit} ({NOT_THERE_BEFORE})")));
            }
        }
        Ok(out)
    }

    /// Why several starts of the file do not count as one, or none. They count as one when each
    /// brought the checker's crate with it (a track adopting the check), and every merge where starts from
    /// different parents meet holds no line that none of its parents held, so the baseline cannot
    /// have grown through it. `pending` is a merge being made: its two tips and the baselines of
    /// the working tree and of each tip. The merges that join starts go to `additions`.
    fn starts_apart(
        &mut self,
        origins: &BTreeSet<String>,
        tips: &[&str],
        pending: Option<(&Keys, [&Keys; 2])>,
    ) -> Result<Option<String>, String> {
        let Some(checker) = self
            .literals
            .rules
            .as_deref()
            .and_then(|rules| Path::new(rules).parent()?.parent())
            .map(|folder| folder.to_string_lossy().into_owned())
        else {
            return Ok(Some(NO_CHECKER.to_string()));
        };
        let has_checker = |revision: &str| -> Result<bool, String> {
            let at = format!("{revision}{IN_REVISION}{checker}");
            Ok(run(self.root, &[CAT_FILE, EXISTS, &at])?.status.success())
        };
        for origin in origins {
            let mut brought = has_checker(origin)?;
            for parent in self.parents(origin)? {
                brought &= !has_checker(&parent)?;
            }
            if !brought {
                return Ok(Some(format!(
                    "{} did not bring the checker with it",
                    short(origin)
                )));
            }
        }
        // Each merge reachable from the tips that descends from at least two starts.
        let mut descends: BTreeMap<String, usize> = BTreeMap::new();
        for origin in origins {
            let mut args = vec![REV_LIST, MERGES, ANCESTRY_PATH];
            args.extend(tips);
            let exclude = format!("{EXCLUDE_PREFIX}{origin}");
            args.push(&exclude);
            for merge in text(self.root, &args)?.split_whitespace() {
                *descends.entry(merge.to_string()).or_default() += 1;
            }
        }
        let mut joins: Vec<(String, Vec<String>)> = Vec::new();
        for (merge, _) in descends.into_iter().filter(|(_, n)| *n > 1) {
            let parents = self.parents(&merge)?;
            joins.push((merge, parents));
        }
        if pending.is_some() {
            joins.push((
                WORKING_TREE.to_string(),
                tips.iter().map(|t| t.to_string()).collect(),
            ));
        }
        let all: Vec<&String> = origins.iter().collect();
        let mut met = HashSet::new();
        for (merge, parents) in joins {
            let mut sides = Vec::new();
            for parent in &parents {
                let mut side = BTreeSet::new();
                for origin in &all {
                    if self.is_ancestor(origin, parent)? {
                        side.insert(*origin);
                    }
                }
                sides.push(side);
            }
            let union: BTreeSet<&String> = sides.iter().flatten().copied().collect();
            if sides.contains(&union) {
                continue;
            }
            for (i, a) in all.iter().enumerate() {
                for b in all.iter().skip(i + 1) {
                    if union.contains(a) && union.contains(b) {
                        met.insert((*a, *b));
                    }
                }
            }
            let (merged, theirs) = match (&pending, merge == WORKING_TREE) {
                (Some((working, tips)), true) => {
                    ((*working).clone(), tips.map(Clone::clone).to_vec())
                }
                _ => {
                    let keys = |r: &str| -> Result<Keys, String> {
                        Ok(self
                            .version(r)?
                            .map(|t| baseline::parse(&t))
                            .unwrap_or_default())
                    };
                    let mut theirs = Vec::new();
                    for parent in &parents {
                        theirs.push(keys(parent)?);
                    }
                    (keys(&merge)?, theirs)
                }
            };
            let grown: Vec<&String> = merged
                .iter()
                .filter(|(key, n)| {
                    theirs
                        .iter()
                        .all(|t| t.get(*key).copied().unwrap_or_default() < **n)
                })
                .map(|(key, _)| key)
                .collect();
            if let Some(first) = grown.first() {
                return Ok(Some(format!(
                    "{} joins them with {} lines none of its parents held, such as {first}",
                    if merge == WORKING_TREE {
                        merge.clone()
                    } else {
                        short(&merge)
                    },
                    grown.len()
                )));
            }
            self.additions.push(format!("{} {JOINED}", short(&merge)));
        }
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                if !met.contains(&(*a, *b)) {
                    return Ok(Some(format!(
                        "{} and {} meet at no merge",
                        short(a),
                        short(b)
                    )));
                }
            }
        }
        Ok(None)
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

    /// Where the file starts on a chain of changes, and which changes may add lines.
    fn start(&self, changes: &[Change]) -> Result<Chain, String> {
        let started = match changes.first() {
            Some(first) => match first.parents.first() {
                Some(parent) => !self.has_file(parent)?,
                None => true,
            },
            None => false,
        };
        let arrived = started && changes.first().is_some_and(|c| c.parents.len() > 1);
        let mut allowed = Vec::new();
        // The versions of the rules at every change, its first parent, and that parent's first parent
        // when it is a merge: read in two or three calls, whatever the length of the chain.
        let firsts: Vec<String> = changes
            .iter()
            .filter_map(|c| c.parents.first().cloned())
            .collect();
        let parents = self.parents_of(&firsts)?;
        let mut revisions: Vec<String> = changes.iter().map(|c| c.commit.clone()).collect();
        revisions.extend(firsts.iter().cloned());
        revisions.extend(
            parents
                .values()
                .filter(|p| p.len() > 1)
                .filter_map(|p| p.first().cloned()),
        );
        revisions.sort();
        revisions.dedup();
        let versions = self.rules_of(&revisions)?;
        let version = |r: &str| versions.get(r).copied().unwrap_or_default();
        for (i, change) in changes.iter().enumerate() {
            if started && i == usize::default() {
                allowed.push((change.commit.clone(), AT_START, None));
            } else if arrived && i == 1 {
                allowed.push((change.commit.clone(), AFTER_ARRIVAL, None));
            } else if let Some(bases) = self.bases_after(
                version(&change.commit),
                &change.parents,
                change.parents.first().and_then(|f| parents.get(f)),
                &version,
            )? {
                // What the change may add was in the tree of a parent: a side it merges, or the
                // branch's own commit before it, which an init run in or right after the merge that
                // brings the rules finds.
                allowed.push((change.commit.clone(), NEW_RULES, Some(bases)));
            }
        }
        Ok(Chain {
            changes: changes.len(),
            arrived,
            allowed,
        })
    }

    /// Checks the chain from `tip` down to `stop`, and every other side merged into it.
    fn chain(&mut self, tip: &str, stop: Option<&str>) -> Result<Chain, String> {
        if !self.seen.insert((tip.to_string(), stop.map(String::from))) {
            return Ok(Chain::default());
        }
        let changes = self.changes(tip, stop)?;
        let diffs = self.diffs(tip, stop)?;
        let chain = self.start(&changes)?;
        self.additions.extend(
            chain
                .allowed
                .iter()
                .map(|(c, why, _)| format!("{} {why}", short(c))),
        );
        for change in &changes {
            let allowed = chain.allowed.iter().find(|(c, ..)| *c == change.commit);
            let exempt = allowed.is_some();
            if let Some((_, diff)) = diffs.iter().find(|(c, _)| *c == change.commit) {
                // A line removed after it grew is no longer growth.
                for line in &diff.removed {
                    if let Some(at) = self.grown.iter().position(|(l, _)| l == line) {
                        self.grown.remove(at);
                    }
                }
                match allowed {
                    None => self.grown.extend(
                        diff.added
                            .iter()
                            .map(|l| (l.clone(), short(&change.commit))),
                    ),
                    Some((_, _, Some(bases))) => {
                        let grown = self.not_there(bases, &diff.added, &short(&change.commit))?;
                        self.grown.extend(grown);
                    }
                    Some((_, _, None)) => {}
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
    literals: &Literals,
    working: &Keys,
) -> Result<History, String> {
    let shallow = text(root, &[REV_PARSE, IS_SHALLOW])?;
    if shallow.trim() == TRUE {
        return Err(SHALLOW.to_string());
    }
    let mut walker = Walker {
        root,
        rel,
        literals,
        additions: Vec::new(),
        seen: HashSet::new(),
        grown: Vec::new(),
        removed: BTreeSet::new(),
        present: HashMap::new(),
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
    let merging = run(root, &[verify.as_slice(), &[MERGE_HEAD]].concat())?
        .status
        .success();
    // The working tree may add lines like a change on top of HEAD, and of MERGE_HEAD while a merge is
    // being made (an init with new rules).
    let working_bases = match fresh {
        true => None,
        false => {
            let tips: Vec<String> = std::iter::once(HEAD)
                .chain(merging.then_some(MERGE_HEAD))
                .map(String::from)
                .collect();
            let head_parents = walker.parents(HEAD)?;
            let mut revisions = tips.clone();
            revisions.extend(head_parents.iter().cloned());
            let versions = walker.rules_of(&revisions)?;
            let version = |r: &str| versions.get(r).copied().unwrap_or_default();
            walker.bases_after(walker.working_rules(), &tips, Some(&head_parents), &version)?
        }
    };
    let working_rules = working_bases.is_some();
    let may_init = fresh || (chain.arrived && chain.changes == 1) || working_rules;
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
    let added: Vec<String> = multiset_minus(working, &committed)
        .into_iter()
        .filter(|k| !from_merge.contains_key(k))
        .collect();
    if fresh || (chain.arrived && chain.changes == 1) {
    } else if let Some(bases) = &working_bases {
        let grown = walker.not_there(bases, &added, WORKING_TREE)?;
        walker.grown.extend(grown);
    } else if !may_init {
        walker
            .grown
            .extend(added.into_iter().map(|k| (k, WORKING_TREE.to_string())));
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
    let pending = merging.then_some((working, [&committed, &from_merge]));
    if origins.len() > 1
        && let Some(why) = walker.starts_apart(&origins, &tips, pending)?
    {
        let all: Vec<String> = origins.iter().map(|c| short(c)).collect();
        problems.push((
            format!(
                "{rel} was started more than once, by {} ({why}): lines a second start brings are not checked",
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
            let theirs = walker.start(&walker.changes(up, None)?)?.allowed;
            for (commit, why, _) in theirs
                .iter()
                .filter(|(t, ..)| !chain.allowed.iter().any(|(c, ..)| c == t))
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
            "the rules have a new version in this change or in the merge it follows: this change may add lines to {rel} for literals already in the tree (run init and commit now), then it only shrinks ({compared})"
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
