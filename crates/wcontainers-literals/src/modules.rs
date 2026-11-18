//! Files that are modules declared under a test-only attribute (`#[cfg(test)] mod tests;`), so that
//! the code in them is test code although it sits under src/.

use std::path::{Path, PathBuf};

use syn::{Attribute, Expr, Item, Lit, Meta};

use crate::RUST_EXTENSION;

/// Files whose child modules live next to them rather than in a folder of their own name.
const MOD_RS_FILES: &[&str] = &["mod.rs", "lib.rs", "main.rs", "build.rs"];
/// The folder of a crate's extra binaries, each of which is a crate root.
const BIN_FOLDER: &str = "bin";
/// `#[path = "..."]` on a module declaration.
const PATH_ATTRIBUTE: &str = "path";

/// A module's file and the folder of its own child modules.
pub struct Module {
    pub file: PathBuf,
    pub folder: PathBuf,
}

/// The modules a file declares: those under a test-only attribute (everything at or under them is
/// test), and those of production code placed with `#[path]` (code wherever they are, even in tests/).
#[derive(Default)]
pub struct Declared {
    pub test: Vec<Module>,
    pub placed: Vec<PathBuf>,
}

fn module_folder(file: &Path) -> PathBuf {
    let parent = file.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let crate_root = MOD_RS_FILES.contains(&name.as_str())
        || parent.file_name().is_some_and(|p| p == BIN_FOLDER);
    if crate_root {
        parent
    } else {
        file.with_extension("")
    }
}

fn path_attribute(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|a| match &a.meta {
        Meta::NameValue(nv) if nv.path.is_ident(PATH_ATTRIBUTE) => match &nv.value {
            Expr::Lit(l) => match &l.lit {
                Lit::Str(s) => Some(s.value()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    })
}

fn collect(
    items: &[Item],
    file: &Path,
    folder: &Path,
    inherited: bool,
    is_test: &dyn Fn(&[Attribute]) -> bool,
    out: &mut Declared,
) {
    for item in items {
        let Item::Mod(m) = item else { continue };
        let test = inherited || is_test(&m.attrs);
        let name = m.ident.to_string();
        if let Some((_, inner)) = &m.content {
            collect(inner, file, &folder.join(&name), test, is_test, out);
            continue;
        }
        let module = match path_attribute(&m.attrs) {
            Some(p) => {
                let file = file
                    .parent()
                    .map(|d| d.join(&p))
                    .unwrap_or_else(|| PathBuf::from(&p));
                if !test {
                    out.placed.push(file.clone());
                }
                Module {
                    folder: file.with_extension(""),
                    file,
                }
            }
            None => Module {
                file: folder.join(&name).with_extension(RUST_EXTENSION),
                folder: folder.join(&name),
            },
        };
        if test {
            out.test.push(module);
        }
    }
}

/// The modules a file declares, as absolute paths.
pub fn declared(file: &Path, ast: &syn::File, is_test: &dyn Fn(&[Attribute]) -> bool) -> Declared {
    let mut out = Declared::default();
    collect(
        &ast.items,
        file,
        &module_folder(file),
        is_test(&ast.attrs),
        is_test,
        &mut out,
    );
    out
}
