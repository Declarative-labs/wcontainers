//! Which crate of the workspace depends on which, read from their manifests. Used to name the owner of a
//! fact (the crate the others depend on), to order the constants a note names, and to tell which
//! constants a crate can already import.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::findings::PATH_COMPONENT_SEPARATOR;
use crate::{MANIFEST, WORKSPACE_TABLE};

const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
const TARGET_TABLE: &str = "target";
const PATH_KEY: &str = "path";
/// `foo = { workspace = true }` takes its definition from the workspace manifest.
const WORKSPACE_KEY: &str = "workspace";
/// `foo = { package = "real-name" }` renames a dependency.
const PACKAGE_KEY: &str = "package";

#[derive(Default)]
pub struct Crates {
    /// Every crate folder each crate depends on, directly or not, itself left out.
    reach: HashMap<String, BTreeSet<String>>,
    /// Packages from outside the workspace that each crate depends on, directly or through its
    /// workspace dependencies.
    external: HashMap<String, BTreeSet<String>>,
}

fn table_entries(manifest: &toml::Table) -> Vec<(&String, &toml::Value)> {
    let mut tables: Vec<&toml::Table> = DEPENDENCY_TABLES
        .iter()
        .filter_map(|t| manifest.get(*t)?.as_table())
        .collect();
    if let Some(targets) = manifest.get(TARGET_TABLE).and_then(|t| t.as_table()) {
        for target in targets.values().filter_map(|t| t.as_table()) {
            tables.extend(
                DEPENDENCY_TABLES
                    .iter()
                    .filter_map(|t| target.get(*t)?.as_table()),
            );
        }
    }
    tables.into_iter().flat_map(|t| t.iter()).collect()
}

fn rel(path: &Path, root: &Path) -> Option<String> {
    let path = path.canonicalize().ok()?;
    let rel = path.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join(PATH_COMPONENT_SEPARATOR),
    )
}

impl Crates {
    /// Reads the manifest of every crate folder (relative to `root`, which is canonical).
    pub fn read(root: &Path, crate_dirs: &BTreeSet<String>) -> Crates {
        let workspace: toml::Table = std::fs::read_to_string(root.join(MANIFEST))
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or_default();
        let workspace_deps = workspace
            .get(WORKSPACE_TABLE)
            .and_then(|w| w.get(DEPENDENCY_TABLES.first()?))
            .and_then(|d| d.as_table())
            .cloned()
            .unwrap_or_default();
        let mut direct: HashMap<String, BTreeSet<String>> = HashMap::new();
        let mut external: HashMap<String, BTreeSet<String>> = HashMap::new();
        for dir in crate_dirs {
            let Some(manifest) = std::fs::read_to_string(root.join(dir).join(MANIFEST))
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok())
            else {
                continue;
            };
            for (name, spec) in table_entries(&manifest) {
                let inherited = spec
                    .get(WORKSPACE_KEY)
                    .and_then(|w| w.as_bool())
                    .unwrap_or_default();
                let (spec, base) = if inherited {
                    (workspace_deps.get(name).unwrap_or(spec), root.to_path_buf())
                } else {
                    (spec, root.join(dir))
                };
                match spec
                    .get(PATH_KEY)
                    .and_then(|p| p.as_str())
                    .and_then(|p| rel(&base.join(p), root))
                {
                    Some(target) => {
                        direct.entry(dir.clone()).or_default().insert(target);
                    }
                    None => {
                        let package = spec
                            .get(PACKAGE_KEY)
                            .and_then(|p| p.as_str())
                            .unwrap_or(name);
                        external
                            .entry(dir.clone())
                            .or_default()
                            .insert(package.to_string());
                    }
                }
            }
        }
        let mut crates = Crates::default();
        for dir in crate_dirs {
            let mut seen = BTreeSet::new();
            let mut todo: Vec<&String> = direct.get(dir).into_iter().flatten().collect();
            while let Some(next) = todo.pop() {
                if next != dir && seen.insert(next.clone()) {
                    todo.extend(direct.get(next).into_iter().flatten());
                }
            }
            let mut packages: BTreeSet<String> = external.get(dir).cloned().unwrap_or_default();
            for d in &seen {
                packages.extend(external.get(d).into_iter().flatten().cloned());
            }
            crates.external.insert(dir.clone(), packages);
            crates.reach.insert(dir.clone(), seen);
        }
        crates
    }

    /// `from` depends on `to`, directly or not.
    pub fn reaches(&self, from: &str, to: &str) -> bool {
        self.reach.get(from).is_some_and(|r| r.contains(to))
    }

    /// `to` is upstream of `from`: `from` depends on it and it does not depend back.
    pub fn upstream(&self, from: &str, to: &str) -> bool {
        self.reaches(from, to) && !self.reaches(to, from)
    }

    /// How many crates depend on this one, directly or not.
    pub fn dependents(&self, dir: &str) -> usize {
        self.reach.values().filter(|r| r.contains(dir)).count()
    }

    /// `owner` owns a fact `copy` restates: `copy` depends on it, or, when neither depends on the
    /// other, more crates depend on `owner`.
    pub fn owns(&self, owner: &str, copy: &str) -> bool {
        self.upstream(copy, owner)
            || (!self.reaches(owner, copy)
                && !self.reaches(copy, owner)
                && self.dependents(owner) > self.dependents(copy))
    }

    /// How many crates this one depends on; fewer means more fundamental.
    pub fn depth(&self, dir: &str) -> usize {
        self.reach.get(dir).map_or(usize::default(), BTreeSet::len)
    }

    /// The crate depends, directly or not, on this package from outside the workspace.
    pub fn uses_package(&self, dir: &str, package: &str) -> bool {
        self.external.get(dir).is_some_and(|p| p.contains(package))
    }
}
