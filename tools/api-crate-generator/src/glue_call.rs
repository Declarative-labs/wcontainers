//! The calling glue: the C side's objects, each held by one counted reference, as an implementation of the traits.
//! Checked first, one call out per block, and an answer adopted only after a success.

use std::fmt::Write as _;

use super::{Glued, Part, Writer, direct_alias, snake, upper};
use crate::emit::{At, HEADER, Key, alias};
use crate::metadata::Method;

impl Writer<'_, '_> {
    fn system_path(&self, kind: usize, api_type: &str) -> String {
        format!("{}::<{api_type}>", self.system_type(kind, self.current.get()))
    }

    /// The trait signature of a method, as the interface writes it, rendered from a direct crate.
    fn trait_signature(&self, method: &Method, api: usize, api_type: &str, place: &str) -> (Vec<(String, String)>, String) {
        let rows = self.g.rows.borrow().clone();
        let unmapped = self.g.unmapped.borrow().len();
        let at = At { api, api_type };
        let (inputs, result) = self.g.signature(method, at, place);
        *self.g.rows.borrow_mut() = rows;
        self.g.unmapped.borrow_mut().truncate(unmapped);
        let fix = |text: String| super::super::rewrite_direct_paths(&text, self.g, api);
        (inputs.into_iter().map(|(name, ty)| (name, fix(ty))).collect(), fix(result))
    }

    /// One method of the calling glue, called through `object` (an expression of a `ForeignObject` whose table holds
    /// `slot`), or through a library's export when `slot` is the export's field.
    #[allow(clippy::too_many_arguments, reason = "the method's own parts")]
    fn calling_method(&self, method: &Method, glued: Option<&Glued>, rust_name: &str, inputs: &[(String, String)], result: &str, call_target: &str, slot: &str, this: bool, api_type: &str, place: &str, face: Option<(&str, &str)>) -> String {
        let mut text = String::new();
        let parameters: String = inputs.iter().map(|(name, ty)| format!(", {name}: {ty}")).collect();
        let returns = if result == "()" { String::new() } else { format!(" -> {result}") };
        let _ = writeln!(text, "    fn {rust_name}(&self{parameters}){returns} {{");
        let status = glued.is_some_and(|g| g.status);
        let fail = if result.starts_with("Result<") {
            "return Err(E_INVALIDARG);".to_string()
        } else if result == "()" {
            "return;".to_string()
        } else if result.starts_with("Option<") {
            "return None;".to_string()
        } else {
            "return Default::default();".to_string()
        };
        let not_glued = |why: &str| -> String {
            if result.starts_with("Result<") {
                format!("        // Not glued yet: {why}.\n        Err(E_NOTIMPL)\n")
            } else if result == "()" {
                format!("        // Not glued yet: {why}.\n")
            } else if result.starts_with("Option<") {
                format!("        // Not glued yet: {why}.\n        None\n")
            } else {
                format!("        unimplemented!(\"{place}: not glued yet: {why}\")\n")
            }
        };
        let Some(glued) = glued else {
            for (name, _) in inputs {
                let _ = writeln!(text, "        let _ = {name};");
            }
            text.push_str(&not_glued("a parameter of a row the glue does not handle"));
            text.push_str("    }\n");
            return text;
        };
        let mut body = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut after = String::new();
        let mut outs: Vec<String> = Vec::new();
        if let Some((face_table, face_iid)) = face {
            let _ = writeln!(body, "        #[expect(unsafe_code, reason = \"QueryInterface for another interface the object answers\")]");
            let _ = writeln!(body, "        // SAFETY: `{face_iid}` names the interface `{face_table}` lays out.");
            let _ = writeln!(body, "        let Some(object) = (unsafe {{ self.object.query::<{face_table}>(&{face_iid}) }}) else {{ {} }};", if result.starts_with("Result<") { "return Err(E_NOINTERFACE);" } else { &fail });
        }
        for ((name, part), param) in glued.parts.iter().zip(&method.params) {
            match part {
                Part::Value { .. } => args.push(Self::convert_out(part, name)),
                Part::Count => {
                    let position = glued.parts.iter().position(|(n, _)| n == name).unwrap_or(0);
                    let users: Vec<(&String, &Part)> = glued
                        .parts
                        .iter()
                        .filter(|(_, p)| matches!(p, Part::ValuesIn { count, .. } | Part::ObjectsIn { count, .. } | Part::BytesOut { count, .. } if *count == position))
                        .map(|(n, p)| (n, p))
                        .collect();
                    let c = self.count_c_type(&param.ty);
                    let length = |(user, part): &(&String, &Part)| match part {
                        Part::BytesOut { optional: true, .. } => format!("{user}.map_or(0, |bytes| bytes.len())"),
                        Part::BytesOut { .. } => format!("{user}.len()"),
                        _ => format!("{user}.len()"),
                    };
                    if let Some(first) = users.first() {
                        let _ = writeln!(body, "        let Ok({name}) = {c}::try_from({}) else {{ {fail} }};", length(first));
                        for other in users.iter().skip(1) {
                            let _ = writeln!(body, "        if {c}::try_from({}).ok() != Some({name}) {{ {fail} }}", length(other));
                        }
                    } else {
                        let _ = writeln!(body, "        let {name}: {c} = 0;");
                    }
                    args.push(name.clone());
                }
                Part::StructIn { optional, coded } => {
                    if coded.decode_only {
                        for (input, _) in inputs {
                            let _ = writeln!(text, "        let _ = {input};");
                        }
                        text.push_str(&not_glued("a structure holding a union, which the glue only reads"));
                        text.push_str("    }\n");
                        return text;
                    }
                    let size = coded.size;
                    if *optional {
                        let _ = writeln!(body, "        let {name}_bytes = {name}.map(|value| {{ let mut bytes = Aligned::<{size}>::default(); encode_{}(value, &mut bytes.0); bytes }});", coded.ident);
                        args.push(format!("{name}_bytes.as_ref().map_or(null(), |bytes| bytes.0.as_ptr())"));
                    } else {
                        let _ = writeln!(body, "        let mut {name}_bytes = Aligned::<{size}>::default();\n        encode_{}({name}, &mut {name}_bytes.0);", coded.ident);
                        args.push(format!("{name}_bytes.0.as_ptr()"));
                    }
                }
                Part::ObjectIn { optional, enum_path, .. } => {
                    let raw = match enum_path {
                        Some(path) => {
                            let variants: Vec<String> = self.g.kinds_deriving(&self.enum_interface(path)).iter().map(|&k| format!("{path}::{}(object) => object.raw()", self.g.kinds[k].name)).collect();
                            format!("match held {{ {} }}", variants.join(", "))
                        }
                        None => "held.raw()".to_string(),
                    };
                    if *optional {
                        args.push(format!("{name}.map_or(null_mut(), |held| {raw})"));
                    } else {
                        args.push(format!("{{ let held = {name}; {raw} }}"));
                    }
                }
                Part::ValuesIn { element, .. } => {
                    let c = match element.as_ref() {
                        Part::Value { c, .. } => c.clone(),
                        _ => "u32".to_string(),
                    };
                    let mapped = match element.as_ref() {
                        Part::Value { form: super::ValueForm::Newtype, .. } => ".map(|value| value.0)".to_string(),
                        Part::Value { form: super::ValueForm::Same, .. } => ".copied()".to_string(),
                        other => format!(".map(|value| {})", Self::convert_out(other, "*value")),
                    };
                    if mapped == ".copied()" {
                        let _ = writeln!(body, "        let {name}_c: Vec<{c}> = {name}.to_vec();");
                    } else {
                        let _ = writeln!(body, "        let {name}_c: Vec<{c}> = {name}.iter(){mapped}.collect();");
                    }
                    args.push(format!("{name}_c.as_ptr().cast::<u8>()"));
                }
                Part::ObjectsIn { .. } => {
                    let _ = writeln!(body, "        let {name}_raw: Vec<Raw> = {name}.iter().map(|held| held.map_or(null_mut(), |held| held.raw())).collect();");
                    args.push(format!("{name}_raw.as_ptr()"));
                }
                Part::BytesOut { optional, .. } => {
                    let length = if *optional { format!("{name}.map_or(0, |bytes| bytes.len())") } else { format!("{name}.len()") };
                    let _ = writeln!(body, "        let Ok({name}_length) = usize::try_from({length}) else {{ {fail} }};");
                    let _ = writeln!(body, "        let mut {name}_buffer = Vec::new();\n        if {name}_buffer.try_reserve_exact({name}_length).is_err() {{ {} }}\n        {name}_buffer.resize({name}_length, 0_u8);", if status { "return Err(E_OUTOFMEMORY);" } else { &fail });
                    if *optional {
                        args.push(format!("if {name}.is_some() {{ {name}_buffer.as_mut_ptr() }} else {{ null_mut() }}"));
                        let _ = writeln!(after, "        if let Some(bytes) = {name} {{ bytes.write(&{name}_buffer); }}");
                    } else {
                        args.push(format!("{name}_buffer.as_mut_ptr()"));
                        let _ = writeln!(after, "        {name}.write(&{name}_buffer);");
                    }
                }
                Part::ObjectOut { kinds, made, .. } => {
                    let _ = writeln!(body, "        let mut {name}: Raw = null_mut();");
                    args.push(format!("&raw mut {name}"));
                    let interface_key = self.out_interface(param);
                    let newest = self.g.kinds[kinds[0]].newest.first().cloned();
                    let adopt = if newest.as_ref() == interface_key.as_ref() { "from_owned" } else { "from_owned_older" };
                    let system = self.system_path(kinds[0], api_type);
                    let _ = writeln!(after, "        #[expect(unsafe_code, reason = \"{place}: {}\")]", param.name);
                    let _ = writeln!(after, "        // SAFETY: the metadata's `{}` is `Out`{}: after a success, null or one reference to an object of `{}`.", param.name, if *made { " and `ComOutPtr`" } else { "" }, interface_key.as_ref().map_or("", |k| k.1.as_str()));
                    if *made {
                        let _ = writeln!(after, "        let Some({name}) = (unsafe {{ {system}::{adopt}({name}) }}) else {{ return Err(E_FAIL); }};");
                    } else {
                        let _ = writeln!(after, "        let {name} = unsafe {{ {system}::{adopt}({name}) }};");
                    }
                    outs.push(name.clone());
                }
                Part::ValueOut { size, element } => {
                    let c = match element.as_ref() {
                        Part::Value { c, .. } => c.clone(),
                        _ => "u32".to_string(),
                    };
                    let _ = writeln!(body, "        let mut {name} = Aligned::<{size}>::default();");
                    args.push(format!("{name}.0.as_mut_ptr()"));
                    let _ = writeln!(after, "        let {name} = {};", Self::convert_in(element, &format!("get_{c}(&{name}.0, 0)")));
                    outs.push(name.clone());
                }
                Part::StructOut { coded } => {
                    let _ = writeln!(body, "        let mut {name} = Aligned::<{}>::default();", coded.size);
                    args.push(format!("{name}.0.as_mut_ptr()"));
                    let _ = writeln!(after, "        let {name} = decode_{}(&{name}.0);", coded.ident);
                    outs.push(name.clone());
                }
            }
        }
        let target = if face.is_some() { "object".to_string() } else { call_target.to_string() };
        let callee = if this { format!("({target}.entry_points().{slot})({target}.raw(){})", args.iter().map(|a| format!(", {a}")).collect::<String>()) } else { format!("({target}.{slot})({})", args.join(", ")) };
        let _ = writeln!(body, "        #[expect(unsafe_code, reason = \"{place}, called out\")]");
        if this {
            let _ = writeln!(body, "        // SAFETY: a live object whose table holds `{place}` where the metadata orders it; every pointer passed is ours for the call.");
        } else {
            let _ = writeln!(body, "        // SAFETY: the library's export `{place}`, as `new`'s caller promised; every pointer passed is ours for the call.");
        }
        let _ = writeln!(body, "        let returned = unsafe {{ {callee} }};");
        if status {
            let _ = writeln!(body, "        succeeded(returned)?;");
        }
        body.push_str(&after);
        let mut values: Vec<String> = Vec::new();
        if let Some(returned) = &glued.returned {
            values.push(Self::convert_in(returned, "returned"));
        } else if !status {
            let _ = writeln!(body, "        let () = returned;");
        }
        values.extend(outs);
        let value = match values.len() {
            0 => "()".to_string(),
            1 => values.remove(0),
            _ => format!("({})", values.join(", ")),
        };
        if status {
            let _ = writeln!(body, "        Ok({value})");
        } else if value != "()" {
            let _ = writeln!(body, "        {value}");
        }
        text.push_str(&body);
        text.push_str("    }\n");
        text
    }

    fn enum_interface(&self, enum_path: &str) -> Key {
        let name = enum_path.rsplit("::").next().unwrap_or("").trim_end_matches("Ref");
        self.g.trait_names.iter().find(|(_, trait_name)| *trait_name == name).map(|(k, _)| k.clone()).unwrap_or_default()
    }

    fn out_interface(&self, param: &crate::metadata::Param) -> Option<Key> {
        let (inner, _, _) = crate::emit::pointee(&param.ty)?;
        crate::emit::named(inner)
    }

    pub(super) fn calling(&self, api: usize, interfaces: &[Key], exports: &[(String, &Glued)], functions: &[&Method]) -> String {
        let g = self.g;
        let spec = g.apis[api];
        let top = g.top_path(api, usize::MAX);
        let mut text = String::new();
        let _ = writeln!(text, "//! {HEADER}\n//!\n//! The calling glue: the C side's objects as an implementation of the traits.\n");
        text.push_str("#![allow(unused_variables, unused_imports, non_snake_case, clippy::let_unit_value, clippy::unit_arg, clippy::needless_return, reason = \"generated: every method has the same shape\")]\n\n");
        text.push_str("use core::hash::{Hash, Hasher};\nuse core::marker::PhantomData;\nuse core::ptr::{null, null_mut};\n\n");
        text.push_str("use wcontainers_component_object_model::constants::{E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_OUTOFMEMORY};\n");
        text.push_str("use wcontainers_component_object_model_direct::{Aligned, ForeignObject, ForeignUnknown, Raw, UnknownTable, succeeded};\n");
        text.push_str("use wcontainers_volatile_bytes::VolatileBytes;\n\nuse crate::abi::*;\nuse crate::data::*;\n\n");
        let _ = interfaces;
        // The object types, one per kind.
        let own_kinds: Vec<usize> = (0..g.kinds.len()).filter(|&k| g.kinds[k].api == api).collect();
        for &kind in &own_kinds {
            let k = &g.kinds[kind];
            let newest = k.newest.first().cloned().unwrap_or_else(|| k.root.clone());
            let table = format!("{}Table", g.trait_names[&newest]);
            let name = format!("System{}", k.name);
            let iid = self.iid(&newest);
            let _ = writeln!(text, "/// An object of the C side's of the kind `{}`, held by one counted reference.", k.name);
            let _ = writeln!(text, "pub struct {name}<A> {{\n    object: ForeignObject<{table}>,\n    api: PhantomData<fn() -> A>,\n}}\n");
            let _ = writeln!(text, "impl<A> {name}<A> {{");
            let _ = writeln!(text, "    /// The object's pointer, live while this lives.\n    pub fn raw(&self) -> Raw {{\n        self.object.raw()\n    }}\n");
            let _ = writeln!(text, "    /// Takes over a reference to an object of `{}`, or returns none for null.\n    ///\n    /// # Safety\n    /// `raw` is null or a live object of `{0}` whose reference the caller gives up.", newest.1);
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"the promise that a pointer is a live object of an interface\")]");
            let _ = writeln!(text, "    pub unsafe fn from_owned(raw: Raw) -> Option<{name}<A>> {{\n        #[deny(unsafe_code)]\n        let body = move || {{");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"the caller's live object\")]\n            // SAFETY: the caller's promise.");
            let _ = writeln!(text, "            let object = unsafe {{ ForeignObject::<{table}>::from_owned(raw) }}?;\n            Some({name} {{ object, api: PhantomData }})\n        }};\n        body()\n    }}\n");
            let _ = writeln!(text, "    /// Takes over a reference to an object of an older interface of the kind, asking it for `{}`.\n    ///\n    /// # Safety\n    /// `raw` is null or a live COM object whose reference the caller gives up.", newest.1);
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"the promise that a pointer is a live COM object\")]");
            let _ = writeln!(text, "    pub unsafe fn from_owned_older(raw: Raw) -> Option<{name}<A>> {{\n        #[deny(unsafe_code)]\n        let body = move || {{");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"the caller's live object\")]\n            // SAFETY: the caller's promise: any COM object is an IUnknown.");
            let _ = writeln!(text, "            let unknown = unsafe {{ ForeignObject::<UnknownTable>::from_owned(raw) }}?;");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"QueryInterface for the newest interface of the kind\")]\n            // SAFETY: `{iid}` names the interface `{table}` lays out.");
            let _ = writeln!(text, "            let object = unsafe {{ unknown.query::<{table}>(&{iid}) }}?;\n            Some({name} {{ object, api: PhantomData }})\n        }};\n        body()\n    }}\n}}\n");
            let _ = writeln!(text, "impl<A> Clone for {name}<A> {{\n    fn clone(&self) -> {name}<A> {{\n        {name} {{ object: self.object.clone(), api: PhantomData }}\n    }}\n}}\n");
            let _ = writeln!(text, "impl<A> PartialEq for {name}<A> {{\n    fn eq(&self, other: &{name}<A>) -> bool {{\n        self.object == other.object\n    }}\n}}\n\nimpl<A> Eq for {name}<A> {{}}\n");
            let _ = writeln!(text, "impl<A> Hash for {name}<A> {{\n    fn hash<H: Hasher>(&self, state: &mut H) {{\n        self.object.hash(state);\n    }}\n}}\n");
        }
        // The trait every API value of the calling glue meets: its kinds are this crate's object types.
        let mut bindings: Vec<String> = own_kinds.iter().map(|&k| format!("{} = System{}<Self>", g.kinds[k].name, g.kinds[k].name)).collect();
        if g.needs_unknown.borrow().contains(&api) {
            bindings.push("Unknown = ForeignUnknown".to_string());
        }
        let supers: Vec<String> = spec
            .supertraits
            .iter()
            .filter_map(|folder| g.apis.iter().position(|a| a.folder == *folder))
            .map(|other| format!("{}::System", direct_alias(g.apis[other].folder)))
            .collect();
        if spec.top_trait.is_some() {
            let mut bound = format!("{top}<{}>", bindings.join(", "));
            for s in &supers {
                bound.push_str(&format!(" + {s}"));
            }
            let _ = writeln!(text, "/// An API value whose objects are the C side's: every kind is this crate's object type.\npub trait System: {bound} {{}}\n");
        }
        // Each kind's traits.
        for &kind in &own_kinds {
            let k = &g.kinds[kind];
            let name = format!("System{}", k.name);
            let mut traits: Vec<Key> = Vec::new();
            for newest in &k.newest {
                for base in g.bases_of(newest) {
                    if g.interface_api.contains_key(&base) && !traits.contains(&base) {
                        traits.push(base);
                    }
                }
            }
            for interface_key in &traits {
                let root_has_api = !g.apiless.borrow().contains(&g.chain_root(interface_key));
                let is_root = g.chain_root(interface_key) == *interface_key;
                let _ = writeln!(text, "impl<A: System> {} for {name}<A> {{", self.trait_path(interface_key));
                if is_root && root_has_api {
                    let _ = writeln!(text, "    type Api = A;\n");
                }
                let slots = self.slots(interface_key);
                for slot in slots.iter().filter(|slot| slot.interface == *interface_key && g.read.interfaces[interface_key].methods.iter().any(|m| m.name == slot.method.name)) {
                    let place = format!("{}::{}", interface_key.1, slot.method.name);
                    let (inputs, result) = self.trait_signature(slot.method, api, "Self::Api", &place);
                    text.push_str(&self.calling_method(slot.method, slot.glued.as_ref(), &Self::method_name(slot.method), &inputs, &result, "self.object", &slot.name, true, "A", &place, None));
                }
                let _ = writeln!(text, "}}\n");
            }
            // The interfaces of another API the kind's objects also answer.
            let faces: Vec<Key> = spec
                .faces
                .iter()
                .filter(|(root, _)| *root == k.root.1)
                .flat_map(|(_, faces)| faces.iter())
                .filter_map(|face| g.interface_api.keys().find(|key| key.1 == *face).cloned())
                .collect();
            for face in &faces {
                let face_api = g.interface_api[face];
                let face_table = format!("{}::{}Table", direct_alias(g.apis[face_api].folder), g.trait_names[face]);
                let face_iid = self.iid(face);
                for interface_key in self.chain(face) {
                    let root_has_api = !g.apiless.borrow().contains(&g.chain_root(&interface_key));
                    let is_root = g.chain_root(&interface_key) == interface_key;
                    let _ = writeln!(text, "impl<A: System> {} for {name}<A> {{", self.trait_path(&interface_key));
                    if is_root && root_has_api {
                        let _ = writeln!(text, "    type Api = A;\n");
                    }
                    let saved = self.current.replace(face_api);
                    let slots = self.slots(face);
                    for slot in slots.iter().filter(|slot| slot.interface == interface_key && g.read.interfaces[&interface_key].methods.iter().any(|m| m.name == slot.method.name)) {
                        let place = format!("{}::{}", interface_key.1, slot.method.name);
                        let (inputs, result) = self.trait_signature(slot.method, api, "Self::Api", &place);
                        self.current.set(saved);
                        text.push_str(&self.calling_method(slot.method, slot.glued.as_ref(), &Self::method_name(slot.method), &inputs, &result, "self.object", &slot.name, true, "A", &place, Some((&face_table, &face_iid))));
                        self.current.set(face_api);
                    }
                    self.current.set(saved);
                    let _ = writeln!(text, "}}\n");
                }
            }
        }
        // The API value: the library's exports, and the libraries of the APIs it extends.
        if let Some(top_name) = spec.top_trait {
            let name = format!("System{top_name}");
            let mut fields = Vec::new();
            if !exports.is_empty() {
                fields.push(("library".to_string(), "Library".to_string()));
            }
            let _ = writeln!(text, "/// The C side's library, as an implementation of the API's top-level trait and of those it extends.");
            if fields.is_empty() {
                let _ = writeln!(text, "pub struct {name};\n");
            } else {
                let _ = writeln!(text, "pub struct {name} {{");
                for (field, ty) in &fields {
                    let _ = writeln!(text, "    {field}: {ty},");
                }
                let _ = writeln!(text, "}}\n\nimpl {name} {{");
                let parameters: Vec<String> = fields.iter().map(|(f, t)| format!("{f}: {t}")).collect();
                let names_list: Vec<String> = fields.iter().map(|(f, _)| f.clone()).collect();
                let _ = writeln!(text, "    /// Returns the API over the C side's library.\n    ///\n    /// # Safety\n    /// Each entry is the library's own export of that name, which takes the parameters its metadata gives.");
                let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"the promise that each entry is the library's export\")]");
                let _ = writeln!(text, "    pub unsafe fn new({}) -> {name} {{\n        {name} {{ {} }}\n    }}\n}}\n", parameters.join(", "), names_list.join(", "));
            }
            let mut top_bindings: Vec<String> = own_kinds.iter().map(|&k| format!("    type {} = System{}<Self>;", g.kinds[k].name, g.kinds[k].name)).collect();
            if g.needs_unknown.borrow().contains(&api) {
                top_bindings.push("    type Unknown = ForeignUnknown;".to_string());
            }
            let _ = writeln!(text, "impl {top} for {name} {{\n{}\n", top_bindings.join("\n"));
            for function in functions {
                let rust_name = names::escaped(snake(crate::names::strip_first(&function.name, spec.function_prefixes)));
                let mut renamed = (*function).clone();
                renamed.name = crate::names::strip_first(&function.name, spec.function_prefixes).to_string();
                let (inputs, result) = self.trait_signature(&renamed, api, "Self", &function.name);
                let glued = exports.iter().find(|(n, _)| *n == snake(&function.name)).map(|(_, g)| *g);
                text.push_str(&self.calling_method(function, glued, &rust_name, &inputs, &result, "self.library", &snake(&function.name), false, "Self", &function.name, None));
            }
            let _ = writeln!(text, "}}\n\nimpl System for {name} {{}}\n");
            // The APIs this one extends, with their kinds from their own direct crates.
            for folder in spec.supertraits {
                let Some(other) = g.apis.iter().position(|a| a.folder == *folder) else { continue };
                let other_top = g.top_path(other, usize::MAX);
                let other_direct = direct_alias(folder);
                let mut lines: Vec<String> = (0..g.kinds.len()).filter(|&k| g.kinds[k].api == other).map(|k| format!("    type {} = {other_direct}::System{}<Self>;", g.kinds[k].name, g.kinds[k].name)).collect();
                if g.needs_unknown.borrow().contains(&other) {
                    lines.push("    type Unknown = ForeignUnknown;".to_string());
                }
                let _ = writeln!(text, "impl {other_top} for {name} {{\n{}\n}}\n", lines.join("\n"));
                let _ = writeln!(text, "impl {other_direct}::System for {name} {{}}\n");
            }
        }
        let _ = upper;
        let _ = alias;
        text
    }
}

use crate::names;
