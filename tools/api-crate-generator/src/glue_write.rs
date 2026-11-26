//! The text of one API's `implementation/direct` crate.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use super::{BYTES_HELPERS, Glued, MOST_ELEMENTS, NOWHERE, Part, ValueForm, Writer, direct_alias, snake, upper};
use crate::emit::{HEADER, Key, alias, package};
use crate::metadata::Method;
use crate::names;

/// The unit of the C side's failures the glue answers with.
const STATUS_IMPORTS: &str = "use wcontainers_component_object_model::constants::{E_FAIL, E_INVALIDARG, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER};";

/// One entry of a C table: the slot's name and its glue, if glued.
pub(super) struct Slot<'m> {
    pub(super) interface: Key,
    pub(super) method: &'m Method,
    pub(super) name: String,
    pub(super) glued: Option<Glued>,
}

impl Writer<'_, '_> {
    pub(super) fn trait_path(&self, interface_key: &Key) -> String {
        format!("{}::{}", alias(self.g.apis[self.g.interface_api[interface_key]]), self.g.trait_names[interface_key])
    }

    pub(super) fn method_name(method: &Method) -> String {
        names::escaped(snake(&method.name))
    }

    /// The interfaces of a chain from its first in the set to `interface_key`.
    pub(super) fn chain(&self, interface_key: &Key) -> Vec<Key> {
        let mut chain: Vec<Key> = self.g.bases_of(interface_key).into_iter().filter(|k| self.g.interface_api.contains_key(k)).collect();
        chain.reverse();
        chain
    }

    pub(super) fn slots(&self, interface_key: &Key) -> Vec<Slot<'_>> {
        let mut slots = Vec::new();
        for member in self.chain(interface_key) {
            let trait_name = &self.g.trait_names[&member];
            let kept: BTreeSet<&str> = self.g.read.interfaces[&member].methods.iter().map(|m| m.name.as_str()).collect();
            let mut seen = BTreeSet::new();
            for method in self.g.all_methods.get(&member).into_iter().flatten() {
                let mut name = format!("{}_{}", snake(trait_name), snake(&method.name));
                while !seen.insert(name.clone()) {
                    name.push('_');
                }
                let glued = if kept.contains(method.name.as_str()) {
                    match self.glue_of(method) {
                        Ok(glued) => Some(glued),
                        Err(why) => {
                            self.not_glued.borrow_mut().push(format!("{}::{}: {why}", member.1, method.name));
                            None
                        }
                    }
                } else {
                    None
                };
                slots.push(Slot { interface: member.clone(), method, name, glued });
            }
        }
        slots
    }

    /// C's parameter list after `this`, and C's return type.
    pub(super) fn c_signature(&self, glued: &Glued, method: &Method) -> (Vec<(String, String)>, String) {
        let mut parameters = Vec::new();
        for ((name, part), param) in glued.parts.iter().zip(&method.params) {
            let c = match part {
                Part::Value { c, .. } => c.clone(),
                Part::Count => self.count_c_type(&param.ty),
                Part::StructIn { .. } | Part::ValuesIn { .. } => "*const u8".to_string(),
                Part::ObjectIn { .. } => "Raw".to_string(),
                Part::ObjectsIn { .. } => "*const Raw".to_string(),
                Part::ObjectOut { .. } => "*mut Raw".to_string(),
                Part::ValueOut { .. } | Part::StructOut { .. } | Part::BytesOut { .. } => "*mut u8".to_string(),
            };
            parameters.push((name.clone(), c));
        }
        let returned = if glued.status {
            "i32".to_string()
        } else {
            match &glued.returned {
                Some(Part::Value { c, .. }) => c.clone(),
                _ => "()".to_string(),
            }
        };
        (parameters, returned)
    }

    pub(super) fn count_c_type(&self, ty: &windows_metadata::Type) -> String {
        match self.value_part(ty) {
            Ok(Part::Value { c, .. }) => c,
            _ => "u32".to_string(),
        }
    }

    pub(super) fn fn_type(&self, glued: &Glued, method: &Method, this: bool) -> String {
        let (parameters, returned) = self.c_signature(glued, method);
        let mut list: Vec<String> = if this { vec!["Raw".to_string()] } else { Vec::new() };
        list.extend(parameters.into_iter().map(|(_, c)| c));
        if returned == "()" {
            format!("unsafe extern \"system\" fn({})", list.join(", "))
        } else {
            format!("unsafe extern \"system\" fn({}) -> {returned}", list.join(", "))
        }
    }

    pub(super) fn convert_in(part: &Part, raw: &str) -> String {
        match part {
            Part::Value { form: ValueForm::Same, .. } => raw.to_string(),
            Part::Value { form: ValueForm::Bool, .. } => format!("{raw} != 0"),
            Part::Value { form: ValueForm::Newtype, rust, .. } => format!("{rust}({raw})"),
            _ => raw.to_string(),
        }
    }

    pub(super) fn convert_out(part: &Part, value: &str) -> String {
        match part {
            Part::Value { form: ValueForm::Same, .. } => value.to_string(),
            Part::Value { form: ValueForm::Bool, .. } => format!("i32::from({value})"),
            Part::Value { form: ValueForm::Newtype, .. } => format!("{value}.0"),
            _ => value.to_string(),
        }
    }

    // ---- The serving glue.

    /// One serving entry: `this` is the bound on the object's type, or none for an export (whose module is `M`).
    fn serving_entry(&self, fn_name: &str, method: &Method, glued: &Glued, bound: Option<&str>, top: &str, place: &str) -> String {
        let (parameters, returned) = self.c_signature(glued, method);
        let api_type = if bound.is_some() { "V::Api" } else { "M" };
        let status = glued.status;
        let fail = |expression: &str, code: &str| if status { format!("{expression}.ok_or({code})?") } else { format!("{expression}?") };
        let mut c_list: Vec<String> = Vec::new();
        if bound.is_some() {
            c_list.push("this: Raw".to_string());
        }
        c_list.extend(parameters.iter().map(|(name, c)| format!("{name}: {c}")));
        let mut text = String::new();
        let generic = match bound {
            Some(bound) => format!("V: {bound}"),
            None => format!("M: {top}"),
        };
        let returns = if returned == "()" { String::new() } else { format!(" -> {returned}") };
        let _ = writeln!(text, "/// `{place}` as the C side calls it.\n///\n/// # Safety\n/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.");
        let _ = writeln!(text, "#[expect(unsafe_code, reason = \"{place}, which the C side calls\")]");
        let _ = writeln!(text, "pub unsafe extern \"system\" fn {fn_name}<{generic}>({}){returns} {{", c_list.join(", "));
        let _ = writeln!(text, "    #[deny(unsafe_code)]\n    let body = move || {{");
        let mut inner = String::new();
        // Required out-parameters must be writable before anything is made.
        for ((name, part), param) in glued.parts.iter().zip(&method.params) {
            if matches!(part, Part::ObjectOut { .. } | Part::ValueOut { .. } | Part::StructOut { .. }) && !param.is_optional && status {
                let _ = writeln!(inner, "            if {name}.is_null() {{ return Err(E_POINTER); }}");
            }
        }
        let this_lookup = match bound {
            Some(_) => format!("let found = {};", fail("object::<V>(KEY, this.addr())", "E_POINTER")),
            None => format!("let found = {};", fail("module::<M>()", "E_FAIL")),
        };
        let _ = writeln!(inner, "            {this_lookup}");
        let mut args = Vec::new();
        let mut outs: Vec<(String, &Part, String)> = Vec::new();
        let counts: Vec<&String> = Vec::new();
        let _ = counts;
        for ((name, part), param) in glued.parts.iter().zip(&method.params) {
            let metadata = &param.name;
            match part {
                Part::Value { .. } => {
                    let _ = writeln!(inner, "            let {name} = {};", Self::convert_in(part, name));
                    args.push(name.clone());
                }
                Part::Count => {}
                Part::StructIn { optional, coded } => {
                    let _ = writeln!(inner, "            let mut {name}_bytes = [0_u8; {}];", coded.size);
                    let _ = writeln!(inner, "            let {name}_null = {name}.is_null();");
                    let _ = writeln!(inner, "            #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "            // SAFETY: the metadata's `{metadata}` is `In` and `Const`: one `{}` of {} bytes, readable for the call.", coded.path.rsplit("::").next().unwrap_or(""), coded.size);
                    let _ = writeln!(inner, "            let {name}_read = unsafe {{ read_bytes({name}, &mut {name}_bytes) }};");
                    let _ = writeln!(inner, "            let {name} = {name}_read.then(|| decode_{}(&{name}_bytes));", coded.ident);
                    if *optional {
                        let _ = writeln!(inner, "            if !{name}_read && !{name}_null {{ return {}; }}", if status { "Err(E_INVALIDARG)" } else { "None" });
                        args.push(format!("{name}.as_ref()"));
                    } else {
                        let _ = writeln!(inner, "            let {name} = {};", fail(name, "E_INVALIDARG"));
                        args.push(format!("&{name}"));
                    }
                }
                Part::ObjectIn { optional, kinds, enum_path } => {
                    let lookup = match enum_path {
                        Some(path) => {
                            let held = format!("{}Found", path.rsplit("::").next().unwrap_or(""));
                            format!("{held}::<{api_type}>::find({name}.addr())")
                        }
                        None => format!("object::<{}>(KEY, {name}.addr())", self.kind_type(kinds[0], api_type)),
                    };
                    let get = if enum_path.is_some() { "get()" } else { "value()" };
                    if *optional {
                        let _ = writeln!(inner, "            let {name}_found = if {name}.is_null() {{ None }} else {{ Some({}) }};", fail(&lookup, "E_INVALIDARG"));
                        args.push(format!("{name}_found.as_ref().map(|held| held.{get})"));
                    } else {
                        let _ = writeln!(inner, "            let {name}_found = {};", fail(&lookup, "E_INVALIDARG"));
                        args.push(format!("{name}_found.{get}"));
                    }
                }
                Part::ValuesIn { count, element, size } => {
                    let count_name = &glued.parts[*count].0;
                    let _ = writeln!(inner, "            let {name}_count = {};", fail(&format!("usize::try_from({count_name}).ok().filter(|count| *count <= {MOST_ELEMENTS})"), "E_INVALIDARG"));
                    let _ = writeln!(inner, "            let {name}_bytes = if {name}_count == 0 {{ Some(Vec::new()) }} else {{");
                    let _ = writeln!(inner, "                #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "                // SAFETY: the metadata's `NativeArrayInfo` names `{}` as the count of `{metadata}`: that many elements of {size} bytes, readable for the call.", method.params[*count].name);
                    let _ = writeln!(inner, "                unsafe {{ read_only_at({name}, {name}_count * {size}) }}\n            }};");
                    let c = match element.as_ref() {
                        Part::Value { c, .. } => c.clone(),
                        _ => "u32".to_string(),
                    };
                    let read = Self::convert_in(element, &format!("get_{c}(element, 0)"));
                    let _ = writeln!(inner, "            let {name}: Vec<_> = {}.as_chunks::<{size}>().0.iter().map(|element| {read}).collect();", fail(&format!("{name}_bytes"), "E_INVALIDARG"));
                    args.push(format!("&{name}"));
                }
                Part::ObjectsIn { count, kind } => {
                    let count_name = &glued.parts[*count].0;
                    let kind_type = self.kind_type(*kind, api_type);
                    let _ = writeln!(inner, "            let {name}_count = {};", fail(&format!("usize::try_from({count_name}).ok().filter(|count| *count <= {MOST_ELEMENTS})"), "E_INVALIDARG"));
                    let _ = writeln!(inner, "            let {name}_bytes = if {name}_count == 0 {{ Some(Vec::new()) }} else {{");
                    let _ = writeln!(inner, "                #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "                // SAFETY: the metadata's `NativeArrayInfo` names `{}` as the count of `{metadata}`: that many pointers, readable for the call.", method.params[*count].name);
                    let _ = writeln!(inner, "                unsafe {{ read_only_at({name}.cast::<u8>(), {name}_count * size_of::<usize>()) }}\n            }};");
                    let _ = writeln!(inner, "            let mut {name}_found = Vec::new();");
                    let _ = writeln!(inner, "            for element in {}.as_chunks::<{{ size_of::<usize>() }}>().0 {{", fail(&format!("{name}_bytes"), "E_INVALIDARG"));
                    let _ = writeln!(inner, "                let address = get_usize(element, 0);");
                    let _ = writeln!(inner, "                {name}_found.push(if address == 0 {{ None }} else {{ Some({}) }});", fail(&format!("object::<{kind_type}>(KEY, address)"), "E_INVALIDARG"));
                    let _ = writeln!(inner, "            }}");
                    let _ = writeln!(inner, "            let {name}: Vec<Option<&{kind_type}>> = {name}_found.iter().map(|held| held.as_ref().map(|held| held.value())).collect();");
                    args.push(format!("&{name}"));
                }
                Part::BytesOut { count, optional } => {
                    let count_name = &glued.parts[*count].0;
                    let _ = writeln!(inner, "            let {name}_length = {};", fail(&format!("usize::try_from({count_name}).ok()"), "E_INVALIDARG"));
                    let _ = writeln!(inner, "            #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "            // SAFETY: the metadata's `MemorySize` names `{}` as the byte count of `{metadata}`, which is `Out`: that many bytes, writable for the call.", method.params[*count].name);
                    let _ = writeln!(inner, "            let {name} = unsafe {{ writable_at({name}, {name}_length) }};");
                    if *optional {
                        args.push(format!("{name}.as_ref()"));
                    } else {
                        let _ = writeln!(inner, "            let {name} = {};", fail(name, "E_INVALIDARG"));
                        args.push(format!("&{name}"));
                    }
                }
                Part::ObjectOut { .. } | Part::ValueOut { .. } | Part::StructOut { .. } => {
                    outs.push((name.clone(), part, metadata.clone()));
                }
            }
        }
        // An object handed out again: from what the glue recorded, before asking the implementation.
        let again = !status && outs.len() == 1 && glued.returned.is_none() && matches!(outs[0].1, Part::ObjectOut { made: false, .. }) && bound.is_some();
        let receiver = if bound.is_some() { "found.value()" } else { "found" };
        let call = format!("{receiver}.{}({})", Self::method_name_of(method, bound.is_none(), self), args.join(", "));
        if again {
            if let (name, Part::ObjectOut { kinds, .. }, _) = &outs[0] {
                let kind_type = self.kind_type(kinds[0], api_type);
                let _ = writeln!(inner, "            if let Some(recorded) = related::<V, {kind_type}>(KEY, this.addr()) {{");
                let _ = writeln!(inner, "                return Some((recorded.into_address().get().to_ne_bytes(),));\n            }}");
                let _ = writeln!(inner, "            let made = {call};");
                let _ = writeln!(inner, "            let Some(made) = made else {{ return Some((0_usize.to_ne_bytes(),)); }};");
                let _ = writeln!(inner, "            drop(found);");
                let _ = writeln!(inner, "            let {name} = serve(KEY, made, record(), {}::<{kind_type}>)?;", self.interfaces_fn(kinds[0], self.current.get()));
                let _ = writeln!(inner, "            if let Some(this_counted) = counted::<V>(KEY, this.addr()) {{");
                let _ = writeln!(inner, "                relate::<{kind_type}, V>(KEY, {name}.address(), this_counted.weak());");
                let _ = writeln!(inner, "                relate::<V, {kind_type}>(KEY, this.addr(), {name}.weak());\n            }}");
                let _ = writeln!(inner, "            Some(({name}.into_address().get().to_ne_bytes(),))");
            }
        } else {
            let mut names_out: Vec<String> = Vec::new();
            if glued.returned.is_some() {
                names_out.push("returned".to_string());
            }
            names_out.extend(outs.iter().map(|(name, _, _)| format!("{name}_value")));
            let pattern = match names_out.len() {
                0 => "()".to_string(),
                1 => names_out[0].clone(),
                _ => format!("({})", names_out.join(", ")),
            };
            let _ = writeln!(inner, "            let {pattern} = {call}{};", if status { "?" } else { "" });
            let _ = writeln!(inner, "            drop(found);");
            let mut written = Vec::new();
            let mut served = Vec::new();
            for (name, part, _) in &outs {
                match part {
                    Part::ObjectOut { kinds, made, .. } => {
                        let kind_type = self.kind_type(kinds[0], api_type);
                        let interfaces = self.interfaces_fn(kinds[0], self.current.get());
                        if *made {
                            let _ = writeln!(inner, "            let {name}_served = {};", fail(&format!("serve(KEY, {name}_value, record(), {interfaces}::<{kind_type}>)"), "E_OUTOFMEMORY"));
                            if bound.is_some() {
                                let _ = writeln!(inner, "            if let Some(this_counted) = counted::<V>(KEY, this.addr()) {{ relate::<{kind_type}, V>(KEY, {name}_served.address(), this_counted.weak()); }}");
                            }
                            served.push((name.clone(), kind_type));
                            written.push(format!("{name}_served"));
                        } else {
                            let _ = writeln!(inner, "            let {name}_served = match {name}_value {{ Some(value) => Some({}), None => None }};", fail(&format!("serve(KEY, value, record(), {interfaces}::<{kind_type}>)"), "E_OUTOFMEMORY"));
                            written.push(format!("{name}_served"));
                        }
                    }
                    Part::ValueOut { size, element } => {
                        let c = match element.as_ref() {
                            Part::Value { c, .. } => c.clone(),
                            _ => "u32".to_string(),
                        };
                        let _ = writeln!(inner, "            let mut {name}_bytes = [0_u8; {size}];");
                        let _ = writeln!(inner, "            put_{c}(&mut {name}_bytes, 0, {});", Self::convert_out(element, &format!("{name}_value")));
                        written.push(format!("{name}_bytes"));
                    }
                    Part::StructOut { coded } => {
                        let _ = writeln!(inner, "            let mut {name}_bytes = [0_u8; {}];", coded.size);
                        let _ = writeln!(inner, "            encode_{}(&{name}_value, &mut {name}_bytes);", coded.ident);
                        written.push(format!("{name}_bytes"));
                    }
                    _ => {}
                }
            }
            // Objects made together know each other: the immediate context is the device's, and the device the context's.
            for (left, left_type) in &served {
                for (right, right_type) in &served {
                    if left != right {
                        let _ = writeln!(inner, "            relate::<{left_type}, {right_type}>(KEY, {left}_served.address(), {right}_served.weak());");
                    }
                }
            }
            let mut finals = Vec::new();
            for ((name, part, _), value) in outs.iter().zip(&written) {
                match part {
                    Part::ObjectOut { made: true, .. } => finals.push(format!("{value}.into_address().get().to_ne_bytes()")),
                    Part::ObjectOut { made: false, .. } => finals.push(format!("{value}.map_or(0, |served| served.into_address().get()).to_ne_bytes()")),
                    _ => finals.push(value.clone()),
                }
                let _ = name;
            }
            let tuple = if glued.returned.is_some() {
                let mut all = vec![Self::convert_out(glued.returned.as_ref().unwrap_or(&Part::Count), "returned")];
                all.extend(finals);
                format!("({},)", all.join(", "))
            } else {
                format!("({},)", finals.join(", "))
            };
            let tuple = if tuple == "(,)" { "()".to_string() } else { tuple };
            let _ = writeln!(inner, "            {}", if status { format!("Ok({tuple})") } else { format!("Some({tuple})") });
        }
        let failed = if status { "Err(E_FAIL)" } else { "None" };
        let _ = writeln!(text, "        let answer = guarded({failed}, move || {{\n{inner}        }});");
        // Every out-parameter is written on every path: the answer, or zeros.
        let offset = usize::from(glued.returned.is_some());
        for (position, (name, part, metadata)) in outs.iter().enumerate() {
            let size = match part {
                Part::ObjectOut { .. } => "size_of::<usize>()".to_string(),
                Part::ValueOut { size, .. } => size.to_string(),
                Part::StructOut { coded } => coded.size.to_string(),
                _ => "0".to_string(),
            };
            let index = position + offset;
            let _ = writeln!(text, "        let {name}_written = answer.as_ref().{}.map_or([0_u8; {size}], |written| written.{index});", if status { "ok()" } else { "as_ref()" }.replace(".as_ref().as_ref()", ".as_ref()"));
            let promise = match part {
                Part::ObjectOut { made: true, .. } => "`Out` and `ComOutPtr`: one interface pointer to write",
                Part::ObjectOut { .. } => "`Out`: one interface pointer to write",
                _ => "`Out`: one value of this size to write",
            };
            let _ = writeln!(text, "        #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
            let _ = writeln!(text, "        // SAFETY: the metadata's `{metadata}` is {promise}, or null, for the call.");
            let _ = writeln!(text, "        let _ = unsafe {{ write_bytes({name}.cast::<u8>(), &{name}_written) }};");
        }
        if status {
            let _ = writeln!(text, "        status(&answer)");
        } else if glued.returned.is_some() {
            let _ = writeln!(text, "        answer.map_or(Default::default(), |written| written.0)");
        } else {
            let _ = writeln!(text, "        let _ = answer;");
        }
        let _ = writeln!(text, "    }};");
        let failed_code = if status { "E_FAIL.0".to_string() } else if returned == "()" { "()".to_string() } else { "Default::default()".to_string() };
        let _ = writeln!(text, "    guarded({failed_code}, body)\n}}\n");
        text
    }

    fn method_name_of(method: &Method, export: bool, writer: &Writer<'_, '_>) -> String {
        if export {
            let prefixes = writer.g.apis[writer.current.get()].function_prefixes;
            names::escaped(snake(names::strip_first(&method.name, prefixes)))
        } else {
            Self::method_name(method)
        }
    }
}

pub(in crate::emit) fn write_direct(writer: &Writer<'_, '_>, api: usize, crate_folder: &Path) -> Result<String, String> {
    let g = writer.g;
    writer.current.set(api);
    writer.coded.borrow_mut().clear();
    writer.codecs.borrow_mut().clear();
    let spec = g.apis[api];
    let mut abi = String::new();
    let mut serve = String::new();
    let mut tables = String::new();
    let mut report = String::new();
    let interfaces: Vec<Key> = g.interface_api.iter().filter(|(_, a)| **a == api).map(|(k, _)| k.clone()).collect();
    // The C tables and their serving entries.
    let mut entries_written: BTreeSet<String> = BTreeSet::new();
    for interface_key in &interfaces {
        let trait_name = &g.trait_names[interface_key];
        let table = format!("{trait_name}Table");
        let slots = writer.slots(interface_key);
        let _ = writeln!(abi, "/// `{}`'s table of entry points, in the metadata's order; an entry the glue leaves out is null.", interface_key.1);
        let _ = writeln!(abi, "#[repr(C)]\npub struct {table} {{\n    pub unknown: UnknownTable,");
        for slot in &slots {
            match &slot.glued {
                Some(glued) => {
                    let _ = writeln!(abi, "    pub {}: {},", slot.name, writer.fn_type(glued, slot.method, true));
                }
                None => {
                    let _ = writeln!(abi, "    pub {}: Absent,", slot.name);
                }
            }
        }
        let _ = writeln!(abi, "}}\n");
        let _ = writeln!(abi, "// SAFETY: `{table}` lays out `{}` as the metadata orders its methods, after IUnknown's three; its objects", interface_key.1);
        let _ = writeln!(abi, "// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.");
        let _ = writeln!(abi, "#[expect(unsafe_code, reason = \"the layout and threads of the table of {}\")]", interface_key.1);
        let _ = writeln!(abi, "unsafe impl Interface for {table} {{\n    fn unknown(&self) -> &UnknownTable {{\n        &self.unknown\n    }}\n}}\n");
        // Serving entries for this interface's own methods.
        for slot in slots.iter().filter(|slot| slot.interface == *interface_key) {
            if let Some(glued) = &slot.glued
                && entries_written.insert(slot.name.clone())
            {
                let bound = writer.trait_path(&slot.interface);
                let place = format!("{}::{}", slot.interface.1, slot.method.name);
                serve.push_str(&writer.serving_entry(&slot.name, slot.method, glued, Some(&bound), "", &place));
                writer.glued.borrow_mut().push(place);
            }
        }
        // The table, filled with the entries, for any object type that is this interface.
        let _ = writeln!(tables, "impl<V: {}> Tables<V> {{", writer.trait_path(interface_key));
        let _ = writeln!(tables, "    /// `{}`'s table for objects of type `V`.", interface_key.1);
        let _ = writeln!(tables, "    pub const {}: &'static {table} = &{table} {{\n        unknown: unknown_table::<V>(),", upper(trait_name));
        for slot in &slots {
            if slot.glued.is_some() {
                let _ = writeln!(tables, "        {}: {}::<V>,", slot.name, slot.name);
            } else {
                let _ = writeln!(tables, "        {}: None,", slot.name);
            }
        }
        let _ = writeln!(tables, "    }};\n}}\n");
    }
    // Interfaces each kind answers.
    let mut kinds_text = String::new();
    for (kind_index, kind) in g.kinds.iter().enumerate().filter(|(_, kind)| kind.api == api) {
        let fn_name = format!("{}_interfaces", snake(&kind.name));
        let mut bounds: Vec<String> = kind.newest.iter().map(|n| writer.trait_path(n)).collect();
        let faces: Vec<Key> = spec
            .faces
            .iter()
            .filter(|(root, _)| *root == kind.root.1)
            .flat_map(|(_, faces)| faces.iter())
            .filter_map(|face| g.interface_api.keys().find(|k| k.1 == *face).cloned())
            .collect();
        bounds.extend(faces.iter().map(|face| writer.trait_path(face)));
        let _ = writeln!(kinds_text, "/// The tables an object of the kind `{}` answers, by interface; IUnknown's is the first.", kind.name);
        let _ = writeln!(kinds_text, "pub fn {fn_name}<V: {}>(interface: &Guid) -> Option<Table> {{", bounds.join(" + "));
        for newest in &kind.newest {
            let ancestors: Vec<String> = g.bases_of(newest).iter().filter(|k| g.interface_api.contains_key(*k)).map(|k| writer.iid(k)).collect();
            let table_const = format!("Tables::<V>::{}", upper(&g.trait_names[newest]));
            let _ = writeln!(kinds_text, "    if *interface == IID_IUNKNOWN || [{}].contains(interface) {{", ancestors.join(", "));
            let _ = writeln!(kinds_text, "        #[expect(unsafe_code, reason = \"a table of entry points the C side calls\")]");
            let _ = writeln!(kinds_text, "        // SAFETY: `{}Table` lays out `{}`, and each of its entries checks the type of the object it finds.", g.trait_names[newest], newest.1);
            let _ = writeln!(kinds_text, "        return Some(unsafe {{ Table::new({table_const}) }});\n    }}");
        }
        for face in &faces {
            let face_api = g.interface_api[face];
            let ancestors: Vec<String> = g.bases_of(face).iter().filter(|k| g.interface_api.contains_key(*k)).map(|k| writer.iid(k)).collect();
            let table_const = format!("{}::Tables::<V>::{}", direct_alias(g.apis[face_api].folder), upper(&g.trait_names[face]));
            let _ = writeln!(kinds_text, "    if [{}].contains(interface) {{", ancestors.join(", "));
            let _ = writeln!(kinds_text, "        #[expect(unsafe_code, reason = \"a table of entry points the C side calls\")]");
            let _ = writeln!(kinds_text, "        // SAFETY: `{}Table` lays out `{}`, and each of its entries checks the type of the object it finds.", g.trait_names[face], face.1);
            let _ = writeln!(kinds_text, "        return Some(unsafe {{ Table::new({table_const}) }});\n    }}");
        }
        let _ = writeln!(kinds_text, "    None\n}}\n");
        let _ = kind_index;
    }
    // Exports.
    let functions: Vec<&Method> = if spec.top_trait.is_some() {
        g.read
            .functions
            .iter()
            .filter(|((namespace, _), _)| spec.namespaces.contains(&namespace.as_str()))
            .map(|(_, function)| function)
            .filter(|function| spec.library.is_some_and(|library| function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library))))
            .filter(|function| spec.only.is_empty() || spec.only.contains(&function.name.as_str()))
            .collect()
    } else {
        Vec::new()
    };
    let top = g.top_path(api, NOWHERE);
    let mut library = String::new();
    let mut exports_fn = String::new();
    let mut export_fields = Vec::new();
    for function in &functions {
        match writer.glue_of(function) {
            Ok(glued) => {
                let name = snake(&function.name);
                let entry = format!("export_{name}");
                serve.push_str(&writer.serving_entry(&entry, function, &glued, None, &top, &function.name));
                export_fields.push((name, writer.fn_type(&glued, function, false), entry, glued));
                writer.glued.borrow_mut().push(function.name.clone());
            }
            Err(why) => writer.not_glued.borrow_mut().push(format!("{}: {why}", function.name)),
        }
    }
    if !export_fields.is_empty() {
        let _ = writeln!(library, "/// The library's exports, as C calls them.\n#[derive(Clone, Copy)]\npub struct Library {{");
        for (name, ty, _, _) in &export_fields {
            let _ = writeln!(library, "    pub {name}: {ty},");
        }
        let _ = writeln!(library, "}}\n");
        let _ = writeln!(exports_fn, "/// Every module served, for the life of the program: the exports find theirs by type.");
        let _ = writeln!(exports_fn, "static MODULES: Mutex<Vec<Arc<dyn Any + Send + Sync>>> = Mutex::new(Vec::new());\n");
        let _ = writeln!(exports_fn, "fn module<M: Send + Sync + 'static>() -> Option<Arc<M>> {{");
        let _ = writeln!(exports_fn, "    let held = MODULES.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|module| module.is::<M>()).cloned();");
        let _ = writeln!(exports_fn, "    held.and_then(|module| module.downcast::<M>().ok())\n}}\n");
        let _ = writeln!(exports_fn, "/// Serves `module` as the library: the exports C calls, which call it. A module of a type already served");
        let _ = writeln!(exports_fn, "/// is not replaced.");
        let _ = writeln!(exports_fn, "pub fn exports<M: {top}>(module: M) -> Library {{");
        let _ = writeln!(exports_fn, "    {{\n        let mut modules = MODULES.lock().unwrap_or_else(PoisonError::into_inner);");
        let _ = writeln!(exports_fn, "        if !modules.iter().any(|held| held.is::<M>()) {{ modules.push(Arc::new(module)); }}\n    }}");
        let fields: Vec<String> = export_fields.iter().map(|(name, _, entry, _)| format!("{name}: {entry}::<M>")).collect();
        let _ = writeln!(exports_fn, "    Library {{ {} }}\n}}\n", fields.join(", "));
    }
    // Held lookups of base-interface pointers (`ResourceRef` and the like) for the serving glue.
    let mut found_enums = String::new();
    for interface_key in g.needs_ref.borrow().iter().filter(|k| g.interface_api[*k] == api) {
        let name = format!("{}Ref", g.trait_names[interface_key]);
        let path = format!("{}::{name}", alias(spec));
        let held = format!("{name}Found");
        let _ = writeln!(found_enums, "/// A `{}*` the C side passed, found as one of our objects and held for the call.", interface_key.1);
        let _ = writeln!(found_enums, "pub enum {held}<A: {top}> {{");
        let kinds = g.kinds_deriving(interface_key);
        for &kind in &kinds {
            let _ = writeln!(found_enums, "    {}(Found<{}>),", g.kinds[kind].name, writer.kind_type(kind, "A"));
        }
        let _ = writeln!(found_enums, "}}\n\nimpl<A: {top}> {held}<A> {{");
        let _ = writeln!(found_enums, "    /// Finds the object at this address, of any of the kinds, or none.\n    pub fn find(address: usize) -> Option<{held}<A>> {{");
        for &kind in &kinds {
            let _ = writeln!(found_enums, "        if let Some(found) = object::<{}>(KEY, address) {{ return Some({held}::{}(found)); }}", writer.kind_type(kind, "A"), g.kinds[kind].name);
        }
        let _ = writeln!(found_enums, "        None\n    }}\n\n    /// The object as the trait takes it.\n    pub fn get(&self) -> {path}<'_, A> {{\n        match self {{");
        for &kind in &kinds {
            let _ = writeln!(found_enums, "            {held}::{0}(found) => {path}::{0}(found.value()),", g.kinds[kind].name);
        }
        let _ = writeln!(found_enums, "        }}\n    }}\n}}\n");
    }
    let calling = writer.calling(api, &interfaces, &export_fields.iter().map(|(n, _, _, gl)| (n.clone(), gl)).collect::<Vec<_>>(), &functions);
    // The crate's files.
    let folder = crate_folder.join("implementation").join("direct");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let codecs = writer.codecs.replace(String::new());
    let data = format!("//! {HEADER}\n//!\n//! Every structure this crate passes, as the bytes C lays it out in.\n\n#![allow(dead_code, unused_imports, reason = \"each structure's both forms are written; a method may use one\")]\n{BYTES_HELPERS}\n{codecs}");
    let mut serving = String::new();
    let _ = writeln!(serving, "//! {HEADER}\n//!\n//! The serving glue: raw C calls in, any implementation of the traits out.\n");
    serving.push_str("#![allow(unused_variables, unused_imports, non_snake_case, clippy::let_unit_value, clippy::unit_arg, reason = \"generated: every method has the same shape\")]\n\n");
    let _ = writeln!(serving, "use core::marker::PhantomData;\nuse std::any::Any;\nuse std::sync::{{Arc, Mutex, PoisonError}};\n");
    let _ = writeln!(serving, "use wcontainers_component_object_model::structures::Guid;");
    serving.push_str(STATUS_IMPORTS);
    serving.push('\n');
    let _ = writeln!(serving, "use wcontainers_component_object_model_direct::{{IID_IUNKNOWN, Found, Raw, Table, counted, guarded, object, read_bytes, read_only_at, record, relate, related, serve, status, unknown_table, writable_at, write_bytes}};\n");
    let _ = writeln!(serving, "use crate::KEY;\nuse crate::abi::*;\nuse crate::data::*;\n");
    let _ = writeln!(serving, "/// The tables of this API's interfaces, for objects of type `V`.\npub struct Tables<V>(PhantomData<V>);\n");
    serving.push_str(&tables);
    serving.push_str(&kinds_text);
    serving.push_str(&found_enums);
    serving.push_str(&exports_fn);
    serving.push_str(&serve);
    let abi_text = format!("//! {HEADER}\n//!\n//! The C tables of this API's interfaces, and its library's exports.\n\nuse wcontainers_component_object_model_direct::{{Absent, Interface, Raw, UnknownTable}};\n\n{abi}{library}");
    let lib = format!(
        "//! {HEADER}\n//!\n//! {} through its C ABI: the serving glue (raw C calls in, any implementation of the traits out) and the\n//! calling glue (the C side's objects as an implementation of the traits). Every `unsafe` of the API is here, each marked.\n\n#[path = \"abi.rs\"]\npub mod abi;\n#[path = \"call.rs\"]\nmod call;\n#[path = \"data.rs\"]\nmod data;\n#[path = \"serve.rs\"]\nmod serve;\n\npub use abi::*;\npub use call::*;\npub use serve::*;\n\nuse wcontainers_component_object_model_direct::GlueKey;\n\n#[expect(unsafe_code, reason = \"the glue's key, which only this crate's glue holds\")]\n// SAFETY: kept in this private static and passed only to the COM crate's glue functions.\nstatic KEY: GlueKey = unsafe {{ GlueKey::new() }};\n",
        spec.summary.trim_end_matches('.')
    );
    let files = [("implementation.rs", lib), ("abi.rs", abi_text), ("serve.rs", serving), ("call.rs", calling), ("data.rs", data)];
    let mut all = String::new();
    for (name, text) in &files {
        let text = with_direct_imports(g, api, text);
        all.push_str(&text);
        std::fs::write(folder.join(name), text).map_err(|error| format!("{name}: {error}"))?;
    }
    let mut manifest = format!(
        "[package]\nname = \"{}-direct\"\ndescription = \"{} through its C ABI: both glues, and every unsafe of the API, each marked.\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"implementation.rs\"\n\n[dependencies]\n",
        package(spec),
        spec.summary.trim_end_matches('.')
    );
    let _ = writeln!(manifest, "{} = {{ path = \"../..\" }}", package(spec));
    for (other, other_spec) in g.apis.iter().enumerate() {
        if other == api {
            continue;
        }
        let interface_alias = format!("{}::", alias(other_spec));
        if all.contains(&interface_alias) || other == 0 {
            let _ = writeln!(manifest, "{} = {{ path = \"../../../{}\" }}", package(other_spec), other_spec.folder);
        }
        let direct = format!("{}::", direct_alias(other_spec.folder));
        if all.contains(&direct) || other == 0 {
            let _ = writeln!(manifest, "{}-direct = {{ path = \"../../../{}/implementation/direct\" }}", package(other_spec), other_spec.folder);
        }
    }
    if all.contains("VolatileBytes") {
        let _ = writeln!(manifest, "wcontainers-volatile-bytes = {{ path = \"../../../volatile-bytes\" }}");
    }
    manifest.push_str("\n# A C boundary: it denies unsafe code, so that each use is marked with its reason.\n[lints.rust]\nunsafe_code = \"deny\"\nunsafe_op_in_unsafe_fn = \"deny\"\n\n[lints.clippy]\ntype_complexity = \"allow\"\n");
    std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
    let _ = writeln!(report, "DIRECT {}: {} interfaces with tables", spec.folder, interfaces.len());
    Ok(report)
}

/// Adds `use` lines for every crate of the set a direct crate's file names by alias.
fn with_direct_imports(g: &crate::emit::Generator<'_>, own: usize, text: &str) -> String {
    let mut imports = Vec::new();
    let named = |needle: &str| text.lines().any(|line| !line.starts_with("use ") && !line.starts_with("//") && line.contains(needle));
    for (position, api) in g.apis.iter().enumerate() {
        if named(&format!("{}::", alias(api))) {
            imports.push(format!("use {} as {};", package(api).replace('-', "_"), alias(api)));
        }
        if position != own && named(&format!("{}::", direct_alias(api.folder))) {
            imports.push(format!("use {}_direct as {};", package(api).replace('-', "_"), direct_alias(api.folder)));
        }
    }
    if imports.is_empty() {
        return text.to_string();
    }
    // After the file's doc comment and inner attributes.
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|line| !(line.starts_with("//!") || line.starts_with("#![") || line.is_empty())).unwrap_or(lines.len());
    let mut out = lines[..at].join("\n");
    out.push_str("\n");
    out.push_str(&imports.join("\n"));
    out.push_str("\n\n");
    out.push_str(&lines[at..].join("\n"));
    out.push('\n');
    out
}
