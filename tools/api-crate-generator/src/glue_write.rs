//! The text of one API's `implementation/direct` crate.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use super::{BYTES_HELPERS, Glued, MOST_ELEMENTS, NOWHERE, Part, ValueForm, Writer, direct_alias, snake, upper};
use crate::api::Exception;
use crate::emit::{HEADER, Key, alias, package};
use crate::metadata::Method;
use crate::names;

/// The unit of the C side's failures the glue answers with.
const STATUS_IMPORTS: &str = "use wcontainers_component_object_model::constants::{E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER};";

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
                    match self.glue_of(method, &format!("{}::{}", member.1, method.name)) {
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
                Part::StructIn { .. } | Part::ValuesIn { .. } | Part::InitialData { .. } | Part::Riid => "*const u8".to_string(),
                Part::ObjectIn { .. } | Part::UnknownIn { .. } => "Raw".to_string(),
                Part::ObjectsIn { .. } => "*const Raw".to_string(),
                Part::ObjectOut { .. } | Part::BoundOut { .. } => "*mut Raw".to_string(),
                Part::ValueOut { .. } | Part::StructOut { .. } | Part::BytesOut { .. } | Part::LentOut { .. } => "*mut u8".to_string(),
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
        let top_of_api = self.g.top_path(self.current.get(), NOWHERE);
        // The role `this` plays for an object it relates to: its kind's name (none for an abstract base, a child).
        let this_role = place.split("::").next().and_then(|interface| self.g.kind_of.iter().find(|(k, _)| k.1 == interface)).map(|(_, &kind)| self.g.kinds[kind].name.clone());
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
            if matches!(part, Part::ObjectOut { .. } | Part::ValueOut { .. } | Part::StructOut { .. } | Part::BoundOut { .. }) && !param.is_optional && status {
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
                Part::InitialData { desc, shape, per_subresource, size, bytes_at, pitch_at, slice_at, path, fields } => {
                    let desc_name = &glued.parts[*desc].0;
                    let suffix = shape.suffix();
                    let [bytes_field, pitch_field, slice_field] = fields;
                    let _ = writeln!(inner, "            let {name}_records: Vec<{path}> = if {name}.is_null() {{ Vec::new() }} else {{");
                    if *per_subresource {
                        let _ = writeln!(inner, "                let count = {};", fail(&format!("crate::lengths::subresources_{suffix}(&{desc_name}).filter(|count| *count <= {MOST_ELEMENTS})"), "E_INVALIDARG"));
                    } else {
                        let _ = writeln!(inner, "                let count = 1;");
                    }
                    let _ = writeln!(inner, "                #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "                // SAFETY: `{metadata}`'s exception line (src/api.rs): {}, each of {size} bytes, readable for the call.", if *per_subresource { "one record per subresource of the description" } else { "one record" });
                    let _ = writeln!(inner, "                let records = unsafe {{ read_only_at({name}, count * {size}) }};");
                    let _ = writeln!(inner, "                let records = {};", fail("records", "E_INVALIDARG"));
                    let _ = writeln!(inner, "                let mut read = Vec::new();");
                    let _ = writeln!(inner, "                for (index, record) in records.as_chunks::<{size}>().0.iter().enumerate() {{");
                    let _ = writeln!(inner, "                    let {pitch_field} = get_u32(record, {pitch_at});\n                    let {slice_field} = get_u32(record, {slice_at});");
                    let _ = writeln!(inner, "                    let length = {};", fail(&format!("crate::lengths::initial_length_{suffix}(&{desc_name}, index, {pitch_field}, {slice_field})"), "E_INVALIDARG"));
                    let _ = writeln!(inner, "                    let address = core::ptr::with_exposed_provenance::<u8>(get_usize(record, {bytes_at}));");
                    let _ = writeln!(inner, "                    #[expect(unsafe_code, reason = \"{place}: {metadata}'s bytes\")]");
                    let _ = writeln!(inner, "                    // SAFETY: `{metadata}`'s exception line (src/api.rs): each record's bytes hold the length its shape's rule gives from the description and the pitches, readable for the call.");
                    let _ = writeln!(inner, "                    let {bytes_field} = unsafe {{ read_only_at(address, length) }};");
                    let _ = writeln!(inner, "                    let {bytes_field} = {};", fail(bytes_field, "E_INVALIDARG"));
                    let _ = writeln!(inner, "                    read.push({path} {{ {bytes_field}, {pitch_field}, {slice_field} }});\n                }}\n                read\n            }};");
                    if *per_subresource {
                        args.push(format!("&{name}_records"));
                    } else {
                        args.push(format!("{name}_records.first()"));
                    }
                }
                Part::UnknownIn { optional } => {
                    let found = format!("UnknownFound::<{api_type}>");
                    let _ = writeln!(inner, "            #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "            // SAFETY: the metadata's `{metadata}` is an `IUnknown*`: null or a live COM object, for the call.");
                    let _ = writeln!(inner, "            let {name}_found = unsafe {{ {found}::find({name}) }};");
                    if *optional {
                        let _ = writeln!(inner, "            if {name}_found.is_none() && !{name}.is_null() {{ return {}; }}", if status { "Err(E_INVALIDARG)" } else { "None" });
                        args.push(format!("{name}_found.as_ref().map(|held| held.get())"));
                    } else {
                        let _ = writeln!(inner, "            let {name}_found = {};", fail(&format!("{name}_found"), "E_INVALIDARG"));
                        args.push(format!("{name}_found.get()"));
                    }
                }
                Part::Riid => {
                    let _ = writeln!(inner, "            let mut {name}_bytes = [0_u8; 16];");
                    let _ = writeln!(inner, "            #[expect(unsafe_code, reason = \"{place}: {metadata}\")]");
                    let _ = writeln!(inner, "            // SAFETY: the metadata's `{metadata}` is `In` and `Const`: one GUID, readable for the call.");
                    let _ = writeln!(inner, "            let {name}_read = unsafe {{ read_bytes({name}, &mut {name}_bytes) }};");
                    let _ = writeln!(inner, "            if !{name}_read {{ return {}; }}", if status { "Err(E_INVALIDARG)" } else { "None" });
                    let _ = writeln!(inner, "            let {name} = decode_guid(&{name}_bytes);");
                }
                Part::ObjectOut { .. } | Part::ValueOut { .. } | Part::StructOut { .. } | Part::LentOut { .. } | Part::BoundOut { .. } => {
                    outs.push((name.clone(), part, metadata.clone()));
                }
            }
        }
        // Memory lent until a later call: the subresource is held busy before the implementation is asked, so that no
        // second Map of it comes between; a call that ends the lending writes our copy back first.
        let lent_out = outs.iter().find_map(|(_, part, _)| match part {
            Part::LentOut { resource, subresource, .. } => Some((glued.parts[*resource].0.clone(), glued.parts[*subresource].0.clone())),
            _ => None,
        });
        if let Some((resource, subresource)) = &lent_out {
            let _ = writeln!(inner, "            let lendings = {};", fail(&format!("{resource}_found.part::<Lendings>()"), "E_INVALIDARG"));
            let _ = writeln!(inner, "            let reservation = {};", fail(&format!("lendings.reserve({subresource})"), "E_INVALIDARG"));
        }
        let ending = self.g.exceptions().find_map(|exception| match exception {
            Exception::Lent { until, resource, subresource, .. } if *until == place => Some((names::value_name(resource), names::value_name(subresource))),
            _ => None,
        });
        if let Some((resource, subresource)) = &ending {
            let _ = writeln!(inner, "            let ending = {resource}_found.part::<Lendings>().and_then(|lendings| lendings.begin_end({subresource}));");
            let _ = writeln!(inner, "            let Some((lent, busy)) = ending else {{ return {}; }};", if status { "Ok(())" } else { "Some(())" });
            let _ = writeln!(inner, "            if let Some((shadow, data)) = lent.downcast_ref::<(Shadow, <{api_type} as {top_of_api}>::Mapped)>()");
            let _ = writeln!(inner, "                && let Some(contents) = shadow.contents()\n            {{\n                let _ = wcontainers_volatile_bytes::VolatileBytes::write(data, &contents);\n            }}");
        }
        // An object handed out again: from what the glue recorded, before asking the implementation.
        let again = !status && outs.len() == 1 && glued.returned.is_none() && matches!(outs[0].1, Part::ObjectOut { made: false, .. }) && bound.is_some();
        let receiver = if bound.is_some() { "found.value()" } else { "found" };
        let call = format!("{receiver}.{}({})", Self::method_name_of(method, bound.is_none(), self), args.join(", "));
        if again {
            if let (name, Part::ObjectOut { kinds, .. }, _) = &outs[0] {
                let kind_type = self.kind_type(kinds[0], api_type);
                let role = &self.g.kinds[kinds[0]].name;
                let _ = writeln!(inner, "            if let Some(recorded) = related::<V>(KEY, this.addr(), \"{role}\") {{");
                let _ = writeln!(inner, "                return Some((recorded.into_address().get().to_ne_bytes(),));\n            }}");
                let _ = writeln!(inner, "            let made = {call};");
                let _ = writeln!(inner, "            let Some(made) = made else {{ return Some((0_usize.to_ne_bytes(),)); }};");
                let _ = writeln!(inner, "            drop(found);");
                let _ = writeln!(inner, "            let {name} = serve(KEY, made, record(), {}::<{kind_type}>)?;", self.interfaces_fn(kinds[0], self.current.get()));
                let _ = writeln!(inner, "            if let Some(this_counted) = counted::<V>(KEY, this.addr()) {{");
                if let Some(this_role) = &this_role {
                    let _ = writeln!(inner, "                relate::<{kind_type}>(KEY, {name}.address(), \"{this_role}\", this_counted.weak());");
                }
                let _ = writeln!(inner, "                relate::<V>(KEY, this.addr(), \"{role}\", {name}.weak());\n            }}");
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
            // A bound object found again: the one served for this index while it lives, asked for the interface.
            let bound_index = glued.parts.iter().find(|(_, part)| matches!(part, Part::Value { .. })).map(|(name, _)| name.clone());
            let riid = glued.parts.iter().find(|(_, part)| matches!(part, Part::Riid)).map(|(name, _)| name.clone());
            for (_, part, _) in &outs {
                if let (Part::BoundOut { name: bound_name }, Some(index), Some(riid)) = (part, &bound_index, &riid) {
                    let bound_type = format!("<{api_type} as {top_of_api}>::{bound_name}");
                    let _ = writeln!(inner, "            if let Some(recorded) = related_at::<V>(KEY, this.addr(), \"{bound_name}\", {index}) {{");
                    let _ = writeln!(inner, "                let answered = {};", fail(&format!("query::<{bound_type}>(KEY, recorded.address(), &{riid})"), "E_NOINTERFACE"));
                    let _ = writeln!(inner, "                return Ok((answered.into_address().get().to_ne_bytes(),));\n            }}");
                }
            }
            let _ = writeln!(inner, "            let {pattern} = {call}{};", if status { "?" } else { "" });
            let mut written = Vec::new();
            let mut served = Vec::new();
            for (name, part, _) in &outs {
                if let Part::LentOut { coded, fields: [data, row, depth], .. } = part
                    && let Some((resource, subresource)) = &lent_out
                {
                    let until = self.g.lent_of(place).and_then(|exception| match exception {
                        Exception::Lent { until, .. } => until.split("::").nth(1).map(|method| names::escaped(snake(method))),
                        _ => None,
                    }).unwrap_or_default();
                    let expression = coded.path.replace('<', "::<");
                    let _ = writeln!(inner, "            let lent_length = mapped_length(&{resource}_found.get(), {subresource}, {name}_value.{row}, {name}_value.{depth});");
                    let _ = writeln!(inner, "            let lent_shadow = lent_length.and_then(Shadow::allocate);");
                    let _ = writeln!(inner, "            let lent_copy = lent_length.and_then(|length| {{ let mut copy = Vec::new(); copy.try_reserve_exact(length).ok()?; copy.resize(length, 0_u8); Some(copy) }});");
                    let _ = writeln!(inner, "            let (Some(shadow), Some(mut copy)) = (lent_shadow, lent_copy) else {{");
                    let _ = writeln!(inner, "                found.value().{until}({resource}_found.get(), {subresource});\n                return Err(E_FAIL);\n            }};");
                    let _ = writeln!(inner, "            let _ = wcontainers_volatile_bytes::VolatileBytes::read(&{name}_value.{data}, &mut copy);");
                    let _ = writeln!(inner, "            let _ = shadow.fill(&copy);");
                    let _ = writeln!(inner, "            let {name}_record = {expression} {{ {data}: shadow.address(), {row}: {name}_value.{row}, {depth}: {name}_value.{depth} }};");
                    let _ = writeln!(inner, "            reservation.settle(Box::new((shadow, {name}_value.{data})));");
                    let _ = writeln!(inner, "            let mut {name}_bytes = [0_u8; {}];", coded.size);
                    let _ = writeln!(inner, "            encode_{}(&{name}_record, &mut {name}_bytes);", coded.ident);
                    written.push(format!("{name}_bytes"));
                }
            }
            let _ = writeln!(inner, "            drop(found);");
            for (name, part, _) in &outs {
                match part {
                    Part::LentOut { .. } => {}
                    Part::BoundOut { name: bound_name } => {
                        let bound_type = format!("<{api_type} as {top_of_api}>::{bound_name}");
                        let index = bound_index.clone().unwrap_or_default();
                        let riid = riid.clone().unwrap_or_default();
                        let _ = writeln!(inner, "            let {name}_interfaces = {};", fail(&format!("bound_{}::<{bound_type}>()", snake(bound_name)), "E_NOINTERFACE"));
                        let _ = writeln!(inner, "            let {name}_served = {};", fail(&format!("serve(KEY, {name}_value, record(), {name}_interfaces)"), "E_OUTOFMEMORY"));
                        let _ = writeln!(inner, "            relate_at::<V>(KEY, this.addr(), \"{bound_name}\", {index}, {name}_served.weak());");
                        let _ = writeln!(inner, "            let {name}_answer = {};", fail(&format!("query::<{bound_type}>(KEY, {name}_served.address(), &{riid})"), "E_NOINTERFACE"));
                        written.push(format!("{name}_answer"));
                    }
                    Part::ObjectOut { kinds, made, .. } => {
                        let kind_type = self.kind_type(kinds[0], api_type);
                        let interfaces = self.interfaces_fn(kinds[0], self.current.get());
                        if *made {
                            let _ = writeln!(inner, "            let {name}_served = {};", fail(&format!("serve(KEY, {name}_value, record(), {interfaces}::<{kind_type}>)"), "E_OUTOFMEMORY"));
                            if bound.is_some()
                                && let Some(this_role) = &this_role
                            {
                                let _ = writeln!(inner, "            if let Some(this_counted) = counted::<V>(KEY, this.addr()) {{ relate::<{kind_type}>(KEY, {name}_served.address(), \"{this_role}\", this_counted.weak()); }}");
                            }
                            served.push((name.clone(), kind_type, self.g.kinds[kinds[0]].name.clone()));
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
            for (left, left_type, _) in &served {
                for (right, _, right_role) in &served {
                    if left != right {
                        let _ = writeln!(inner, "            relate::<{left_type}>(KEY, {left}_served.address(), \"{right_role}\", {right}_served.weak());");
                    }
                }
            }
            let mut finals = Vec::new();
            for ((name, part, _), value) in outs.iter().zip(&written) {
                match part {
                    Part::ObjectOut { made: true, .. } | Part::BoundOut { .. } => finals.push(format!("{value}.into_address().get().to_ne_bytes()")),
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
                Part::ObjectOut { .. } | Part::BoundOut { .. } => "size_of::<usize>()".to_string(),
                Part::LentOut { coded, .. } => coded.size.to_string(),
                Part::ValueOut { size, .. } => size.to_string(),
                Part::StructOut { coded } => coded.size.to_string(),
                _ => "0".to_string(),
            };
            let index = position + offset;
            let _ = writeln!(text, "        let {name}_written = answer.as_ref().{}.map_or([0_u8; {size}], |written| written.{index});", if status { "ok()" } else { "as_ref()" }.replace(".as_ref().as_ref()", ".as_ref()"));
            let promise = match part {
                Part::ObjectOut { made: true, .. } | Part::BoundOut { .. } => "`Out` and `ComOutPtr`: one interface pointer to write",
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
            let table_const = if face_api == api { format!("Tables::<V>::{}", upper(&g.trait_names[face])) } else { format!("{}::Tables::<V>::{}", direct_alias(g.apis[face_api].folder), upper(&g.trait_names[face])) };
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
        match writer.glue_of(function, &function.name) {
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
        let mut wheres = Vec::new();
        let mut binds = Vec::new();
        for exception in spec.exceptions {
            let Exception::Binds { api: other_folder, name, kind } = exception else { continue };
            let Some(other) = g.apis.iter().position(|a| a.folder == *other_folder) else { continue };
            let Some(kind_index) = g.interface_api.keys().find(|k| k.1 == *kind).and_then(|k| g.kind_of.get(k)).copied() else { continue };
            let kind_type = writer.kind_type(kind_index, "M");
            wheres.push(format!("M: {}<{name} = {kind_type}>", g.top_path(other, NOWHERE)));
            binds.push(format!("    {}::bind_{}::<{kind_type}>({}::<{kind_type}>);", direct_alias(other_folder), snake(name), writer.interfaces_fn(kind_index, api)));
        }
        let signature = if wheres.is_empty() { format!("pub fn exports<M: {top}>(module: M) -> Library") } else { format!("pub fn exports<M>(module: M) -> Library\nwhere\n    M: {top},\n    {},", wheres.join(",\n    ")) };
        let _ = writeln!(exports_fn, "{signature} {{");
        for bind in &binds {
            let _ = writeln!(exports_fn, "{bind}");
        }
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
        let _ = writeln!(found_enums, "        }}\n    }}\n\n    /// The part of what the glue recorded about the object that is of type `R`, if any.\n    pub fn part<R: Any>(&self) -> Option<&R> {{\n        match self {{");
        for &kind in &kinds {
            let _ = writeln!(found_enums, "            {held}::{}(found) => found.part::<R>(),", g.kinds[kind].name);
        }
        let _ = writeln!(found_enums, "        }}\n    }}\n}}\n");
    }
    // An `IUnknown*` the C side passed: one of our objects of the API's kinds, or any other COM object.
    if g.needs_unknown.borrow().contains(&api) {
        let path = format!("{}::UnknownRef", alias(spec));
        let _ = writeln!(found_enums, "/// An `IUnknown*` the C side passed: one of our objects of a kind of this API, held for the call, or any other.");
        let _ = writeln!(found_enums, "pub enum UnknownFound<A: {top}> {{");
        let own: Vec<usize> = (0..g.kinds.len()).filter(|&k| g.kinds[k].api == api).collect();
        for &kind in &own {
            let _ = writeln!(found_enums, "    {}(Found<{}>),", g.kinds[kind].name, writer.kind_type(kind, "A"));
        }
        let _ = writeln!(found_enums, "    Unknown(Foreign),\n}}\n\nimpl<A: {top}> UnknownFound<A> {{");
        let _ = writeln!(found_enums, "    /// Finds the object at this address: one of ours, else the C side's, of which it takes a reference; none for null.\n    ///\n    /// # Safety\n    /// `raw` is null or a live COM object, for the call.");
        let _ = writeln!(found_enums, "    #[expect(unsafe_code, reason = \"an IUnknown* the C side passes in\")]\n    pub unsafe fn find(raw: Raw) -> Option<UnknownFound<A>> {{\n        #[deny(unsafe_code)]\n        let body = move || {{\n            let address = raw.addr();");
        for &kind in &own {
            let _ = writeln!(found_enums, "            if let Some(found) = object::<{}>(KEY, address) {{ return Some(UnknownFound::{}(found)); }}", writer.kind_type(kind, "A"), g.kinds[kind].name);
        }
        let _ = writeln!(found_enums, "            #[expect(unsafe_code, reason = \"an IUnknown* the C side passes in\")]\n            // SAFETY: the caller's promise.\n            let foreign = unsafe {{ ForeignUnknown::from_borrowed(raw) }}?;");
        let _ = writeln!(found_enums, "            Some(UnknownFound::Unknown(Foreign::new(address, Arc::new(foreign))))\n        }};\n        body()\n    }}\n");
        let _ = writeln!(found_enums, "    /// The object as the trait takes it.\n    pub fn get(&self) -> {path}<'_, A> {{\n        match self {{");
        for &kind in &own {
            let _ = writeln!(found_enums, "            UnknownFound::{0}(found) => {path}::{0}(found.value()),", g.kinds[kind].name);
        }
        let _ = writeln!(found_enums, "            UnknownFound::Unknown(foreign) => {path}::Unknown(foreign),\n        }}\n    }}\n}}\n");
    }
    // The length of memory lent by Map, by the resource's shape: the same rule in both glues.
    for exception in spec.exceptions {
        let Exception::Lent { method, resource, .. } = exception else { continue };
        let Some((interface, method_name)) = method.split_once("::") else { continue };
        let Some(method) = g.read.interfaces.iter().find(|(k, _)| k.1 == interface).and_then(|(_, i)| i.methods.iter().find(|m| m.name == method_name)) else { continue };
        let Some(resource_key) = method.params.iter().find(|p| p.name == *resource).and_then(|p| crate::emit::named(&p.ty)) else { continue };
        let path = format!("{}::{}Ref", alias(spec), g.trait_names[&resource_key]);
        let _ = writeln!(found_enums, "/// The bytes the C side may touch in memory Map lends for a subresource of `resource` whose rows start `row_pitch`\n/// apart and slices `depth_pitch` apart, by the shape's rule from its description; none for a shape no rule covers.");
        let _ = writeln!(found_enums, "pub fn mapped_length<A: {top}>(resource: &{path}<'_, A>, subresource: u32, row_pitch: u32, depth_pitch: u32) -> Option<usize> {{\n    match resource {{");
        for kind in g.kinds_deriving(&resource_key) {
            let kind_name = &g.kinds[kind].name;
            let shape = [crate::api::Shape::Buffer, crate::api::Shape::Texture1D, crate::api::Shape::Texture2D, crate::api::Shape::Texture3D].into_iter().find(|shape| shape.kind() == kind_name);
            match shape {
                Some(shape) => {
                    let trait_path = writer.trait_path(&g.kinds[kind].root);
                    let _ = writeln!(found_enums, "        {path}::{kind_name}(object) => crate::lengths::mapped_length_{}(&{trait_path}::get_desc(*object), subresource, row_pitch, depth_pitch),", shape.suffix());
                }
                None => {
                    let _ = writeln!(found_enums, "        {path}::{kind_name}(_) => None,");
                }
            }
        }
        let _ = writeln!(found_enums, "    }}\n}}\n");
    }
    // A bound object: the tables of the type an API built on this one binds, which that API's glue registers.
    for exception in spec.exceptions {
        let Exception::BoundObject { name, .. } = exception else { continue };
        let snake_name = snake(name);
        let upper_name = snake_name.to_ascii_uppercase();
        let _ = writeln!(found_enums, "/// The tables of each type served as a `{name}`, by the glue of the API that binds it.\nstatic BOUND_{upper_name}: Mutex<Vec<(core::any::TypeId, Interfaces)>> = Mutex::new(Vec::new());\n");
        let _ = writeln!(found_enums, "/// Registers the tables of `B`, the type an API built on this one binds as its `{name}`.\npub fn bind_{snake_name}<B: 'static>(interfaces: Interfaces) {{");
        let _ = writeln!(found_enums, "    let mut bound = BOUND_{upper_name}.lock().unwrap_or_else(PoisonError::into_inner);\n    if !bound.iter().any(|(kind, _)| *kind == core::any::TypeId::of::<B>()) {{ bound.push((core::any::TypeId::of::<B>(), interfaces)); }}\n}}\n");
        let _ = writeln!(found_enums, "/// Returns the tables registered for `B`, if any.\npub fn bound_{snake_name}<B: 'static>() -> Option<Interfaces> {{");
        let _ = writeln!(found_enums, "    let bound = BOUND_{upper_name}.lock().unwrap_or_else(PoisonError::into_inner);\n    bound.iter().find(|(kind, _)| *kind == core::any::TypeId::of::<B>()).map(|(_, interfaces)| *interfaces)\n}}\n");
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
    let _ = writeln!(serving, "use wcontainers_component_object_model::structures::Foreign;");
    let _ = writeln!(serving, "use wcontainers_component_object_model_direct::{{IID_IUNKNOWN, ForeignUnknown, Found, Interfaces, Lendings, Raw, Shadow, Table, counted, guarded, object, query, read_bytes, read_only_at, record, relate, relate_at, related, related_at, serve, status, unknown_table, writable_at, write_bytes}};\n");
    let _ = writeln!(serving, "use crate::KEY;\nuse crate::abi::*;\nuse crate::data::*;\n");
    let _ = writeln!(serving, "/// The tables of this API's interfaces, for objects of type `V`.\npub struct Tables<V>(PhantomData<V>);\n");
    serving.push_str(&tables);
    serving.push_str(&kinds_text);
    serving.push_str(&found_enums);
    serving.push_str(&exports_fn);
    serving.push_str(&serve);
    let abi_text = format!("//! {HEADER}\n//!\n//! The C tables of this API's interfaces, and its library's exports.\n\nuse wcontainers_component_object_model_direct::{{Absent, Interface, Raw, UnknownTable}};\n\n{abi}{library}");
    let has_lengths = spec.exceptions.iter().any(|e| matches!(e, Exception::InitialData { .. } | Exception::Lent { .. }));
    let lengths_module = if has_lengths { "#[path = \"lengths.rs\"]\nmod lengths;\n" } else { "" };
    let lib = format!(
        "//! {HEADER}\n//!\n//! {} through its C ABI: the serving glue (raw C calls in, any implementation of the traits out) and the\n//! calling glue (the C side's objects as an implementation of the traits). Every `unsafe` of the API is here, each marked.\n\n#[path = \"abi.rs\"]\npub mod abi;\n#[path = \"call.rs\"]\nmod call;\n#[path = \"data.rs\"]\nmod data;\n{lengths_module}#[path = \"serve.rs\"]\nmod serve;\n\npub use abi::*;\npub use call::*;\npub use serve::*;\n\nuse wcontainers_component_object_model_direct::GlueKey;\n\n#[expect(unsafe_code, reason = \"the glue's key, which only this crate's glue holds\")]\n// SAFETY: kept in this private static and passed only to the COM crate's glue functions.\nstatic KEY: GlueKey = unsafe {{ GlueKey::new() }};\n",
        spec.summary.trim_end_matches('.')
    );
    let mut files = vec![("implementation.rs", lib), ("abi.rs", abi_text), ("serve.rs", serving), ("call.rs", calling), ("data.rs", data)];
    if has_lengths {
        // Only the rules of the shapes this set has: a slice's interface crate holds only the descriptions it names.
        let mut kept = String::new();
        let mut keep = true;
        for line in include_str!("../templates/direct3d-11-direct/lengths.rs").lines() {
            if let Some(shape) = line.strip_prefix("// shape: ") {
                let kind = [crate::api::Shape::Buffer, crate::api::Shape::Texture1D, crate::api::Shape::Texture2D, crate::api::Shape::Texture3D].into_iter().find(|s| s.suffix() == shape).map(|s| s.kind());
                keep = kind.is_some_and(|kind| g.kinds.iter().any(|k| k.api == api && k.name == kind));
                continue;
            }
            if line == "// end of shape" {
                keep = true;
                continue;
            }
            if keep {
                kept.push_str(line);
                kept.push('\n');
            }
        }
        files.push(("lengths.rs", format!("//! {HEADER}\n//!\n{kept}")));
    }
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
