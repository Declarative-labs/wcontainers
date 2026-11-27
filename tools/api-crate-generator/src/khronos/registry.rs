//! Reads a Khronos registry (gl.xml, egl.xml): its commands with their parameters' C types and attributes, its
//! enumerants with the groups they belong to, its features and its extensions.

use std::collections::{BTreeMap, BTreeSet};

/// One parameter, or a command's return (named `return`).
#[derive(Clone, Debug, Default)]
pub struct Param {
    pub name: String,
    /// The registry's `<ptype>`, or the C type written in the text when there is none (`void`, `char`).
    pub ptype: String,
    /// Stars after the type: 0 a value, 1 a pointer, 2 a pointer to pointers.
    pub pointers: usize,
    /// `const` before the type.
    pub is_const: bool,
    /// `const` between the stars (`const GLchar *const*`).
    pub inner_const: bool,
    pub len: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Command {
    pub name: String,
    pub ret: Param,
    pub params: Vec<Param>,
}

#[derive(Clone, Debug)]
pub struct Enumerant {
    /// The value as written (`0x8B31`, `EGL_CAST(EGLint,-1)`).
    pub value: String,
    /// `u` or `ull` when the registry marks the value unsigned or 64-bit.
    pub suffix: Option<String>,
    /// The API the value is for, when the registry gives one value per API.
    pub api: Option<String>,
}

/// What a feature or an extension requires.
#[derive(Clone, Debug, Default)]
pub struct Requires {
    pub commands: BTreeSet<String>,
    pub enums: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub api: String,
    pub number: String,
    pub requires: Requires,
}

#[derive(Clone, Debug)]
pub struct Extension {
    pub name: String,
    /// The APIs it is defined for (`gles2|gl`).
    pub supported: BTreeSet<String>,
    pub requires: Requires,
}

#[derive(Debug, Default)]
pub struct Registry {
    pub commands: BTreeMap<String, Command>,
    pub enumerants: BTreeMap<String, Vec<Enumerant>>,
    /// Group name to the enumerants that name it in their `group` attribute.
    pub groups: BTreeMap<String, BTreeSet<String>>,
    pub features: Vec<Feature>,
    pub extensions: Vec<Extension>,
}

fn param_of(node: roxmltree::Node<'_, '_>) -> Param {
    let mut param = Param {
        len: node.attribute("len").map(str::to_string),
        ..Param::default()
    };
    let mut before_type = String::new();
    let mut after_type = String::new();
    let mut seen_type = false;
    for child in node.children() {
        match child.tag_name().name() {
            "ptype" => {
                param.ptype = child.text().unwrap_or_default().to_string();
                seen_type = true;
            }
            "name" => param.name = child.text().unwrap_or_default().to_string(),
            "" => {
                let text = child.text().unwrap_or_default();
                if seen_type || !param.name.is_empty() {
                    after_type.push_str(text);
                } else {
                    before_type.push_str(text);
                }
            }
            _ => {}
        }
    }
    if !seen_type {
        // `const void *pixels`, `const char *procname`: the C type is in the text.
        let words: Vec<&str> = before_type.split(|c: char| c.is_whitespace() || c == '*').filter(|w| !w.is_empty()).collect();
        param.ptype = words.iter().rev().find(|w| **w != "const").map(|w| w.to_string()).unwrap_or_default();
        param.is_const = words.first() == Some(&"const");
        param.pointers = before_type.matches('*').count();
    } else {
        param.is_const = before_type.trim_start().starts_with("const");
        param.pointers = after_type.matches('*').count();
        param.inner_const = after_type.contains("const");
    }
    param
}

fn requires_of(node: roxmltree::Node<'_, '_>, api: &str) -> Requires {
    let mut requires = Requires::default();
    for require in node.children().filter(|c| c.tag_name().name() == "require") {
        // A require block for one API only applies to that API.
        if require.attribute("api").is_some_and(|a| a != api) {
            continue;
        }
        for item in require.children() {
            let Some(name) = item.attribute("name") else { continue };
            match item.tag_name().name() {
                "command" => {
                    requires.commands.insert(name.to_string());
                }
                "enum" => {
                    requires.enums.insert(name.to_string());
                }
                _ => {}
            }
        }
    }
    requires
}

/// Reads a registry file; `api` is the API whose per-API requires and values are taken (`gles2`, `egl`).
pub fn read(text: &str, api: &str) -> Result<Registry, String> {
    let document = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
    let mut registry = Registry::default();
    for top in document.root_element().children() {
        match top.tag_name().name() {
            "commands" => {
                for command in top.children().filter(|c| c.tag_name().name() == "command") {
                    let Some(proto) = command.children().find(|c| c.tag_name().name() == "proto") else { continue };
                    let mut ret = param_of(proto);
                    let name = std::mem::replace(&mut ret.name, "return".to_string());
                    let params = command.children().filter(|c| c.tag_name().name() == "param").map(param_of).collect();
                    registry.commands.insert(name.clone(), Command { name, ret, params });
                }
            }
            "enums" => {
                for item in top.children().filter(|c| c.tag_name().name() == "enum") {
                    let (Some(name), Some(value)) = (item.attribute("name"), item.attribute("value")) else { continue };
                    registry.enumerants.entry(name.to_string()).or_default().push(Enumerant {
                        value: value.to_string(),
                        suffix: item.attribute("type").map(str::to_string),
                        api: item.attribute("api").map(str::to_string),
                    });
                    for group in item.attribute("group").unwrap_or_default().split(',').filter(|g| !g.is_empty()) {
                        registry.groups.entry(group.to_string()).or_default().insert(name.to_string());
                    }
                }
            }
            "feature" => {
                let feature_api = top.attribute("api").unwrap_or_default().to_string();
                registry.features.push(Feature {
                    requires: requires_of(top, &feature_api),
                    api: feature_api,
                    number: top.attribute("number").unwrap_or_default().to_string(),
                });
            }
            "extensions" => {
                for extension in top.children().filter(|c| c.tag_name().name() == "extension") {
                    let supported: BTreeSet<String> =
                        extension.attribute("supported").unwrap_or_default().split('|').map(str::to_string).collect();
                    registry.extensions.push(Extension {
                        name: extension.attribute("name").unwrap_or_default().to_string(),
                        requires: requires_of(extension, api),
                        supported,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(registry)
}

impl Registry {
    /// Adds another registry's commands, values, groups and extensions (ANGLE's own extensions beside gl.xml); a
    /// command or value both name fails.
    pub fn merge(&mut self, other: Registry) -> Result<(), String> {
        for (name, command) in other.commands {
            if self.commands.insert(name.clone(), command).is_some() {
                return Err(format!("{name} is in both registries"));
            }
        }
        for (name, values) in other.enumerants {
            if self.enumerants.contains_key(&name) {
                return Err(format!("{name} is in both registries"));
            }
            self.enumerants.insert(name, values);
        }
        for (group, members) in other.groups {
            self.groups.entry(group).or_default().extend(members);
        }
        self.extensions.extend(other.extensions);
        Ok(())
    }

    /// The value of an enumerant for this API: the one marked for it, else the one marked for none.
    pub fn value(&self, name: &str, api: &str) -> Option<&Enumerant> {
        let all = self.enumerants.get(name)?;
        all.iter().find(|e| e.api.as_deref() == Some(api)).or_else(|| all.iter().find(|e| e.api.is_none()))
    }

    /// The extensions of this API that require a command, by name.
    pub fn extensions_of(&self, command: &str, api: &str) -> Vec<String> {
        self.extensions
            .iter()
            .filter(|e| e.supported.contains(api) && e.requires.commands.contains(command))
            .map(|e| e.name.clone())
            .collect()
    }

    /// Whether a feature of this API up to a version requires a command.
    pub fn in_core(&self, command: &str, api: &str, up_to: &str) -> bool {
        self.features.iter().any(|f| f.api == api && version_at_most(&f.number, up_to) && f.requires.commands.contains(command))
    }
}

/// `3.0` at most `3.2`.
pub fn version_at_most(number: &str, up_to: &str) -> bool {
    let parse = |text: &str| -> (u32, u32) {
        let mut parts = text.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
    };
    parse(number) <= parse(up_to)
}
