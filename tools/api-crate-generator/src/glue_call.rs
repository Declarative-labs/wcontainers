//! The calling glue: the C side's objects, each held by one counted reference, as an implementation of the traits.
//! Checked first, one call out per block, and an answer adopted only after a success.

use std::fmt::Write as _;

use super::{Glued, Part, Writer, direct_alias, snake, upper};
use crate::api::{Exception, Hold};
use crate::emit::{At, HEADER, Key, alias};
use crate::metadata::Method;

impl Writer<'_, '_> {
    fn system_path(&self, kind: usize, api_type: &str) -> String {
        format!("{}::<{api_type}>", self.system_type(kind, self.current.get()))
    }

    /// The trait signature of a method, as the interface writes it, rendered from a direct crate.
    pub(super) fn trait_signature(&self, method: &Method, api: usize, api_type: &str, place: &str) -> (Vec<(String, String)>, String) {
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
        // Default deny (src/kinds.rs): a part of a kind the system trusts, or memory lent until later, with no row.
        if let Some(crate::emit::kinds::Treatment::Refused(why)) = self.g.treatment(place) {
            for (name, _) in inputs {
                let _ = writeln!(text, "        let _ = {name};");
            }
            let answer = if result.starts_with("Result<") {
                "Err(E_NOTIMPL)"
            } else if result == "()" {
                ""
            } else if result.starts_with("Option<") {
                "None"
            } else {
                "Default::default()"
            };
            let why = why.join("; ").replace('\n', " ");
            let _ = writeln!(text, "        // Refused by its kinds (src/kinds.rs), the system never called: {why}.\n        {answer}");
            text.push_str("    }\n");
            return text;
        }
        let Some(glued) = glued else {
            for (name, _) in inputs {
                let _ = writeln!(text, "        let _ = {name};");
            }
            text.push_str(&not_glued("a parameter of a row the glue does not handle"));
            text.push_str("    }\n");
            return text;
        };
        let mut body = String::new();
        // The pins of the method (src/api.rs, checked by src/undo.rs): what could undo a fact a mark relies on.
        let holds = self.g.holds_of(place);
        let pinned_body = holds.iter().find_map(|hold| match hold {
            Hold::NotCalled { why } => {
                let answer = if result.starts_with("Result<") {
                    "Err(E_NOTIMPL)"
                } else if result == "()" {
                    ""
                } else if result.starts_with("Option<") {
                    "None"
                } else {
                    "Default::default()"
                };
                Some(format!("        // Pinned (src/api.rs): never called by the calling glue: {why}.\n        {answer}\n"))
            }
            Hold::Answers { value, why } => Some(format!("        // Pinned (src/api.rs): answered, the system never called: {why}.\n        {value}\n")),
            _ => None,
        });
        if let Some(pinned) = pinned_body {
            for (name, _) in inputs {
                let _ = writeln!(text, "        let _ = {name};");
            }
            text.push_str(&pinned);
            text.push_str("    }\n");
            return text;
        }
        for hold in &holds {
            match hold {
                Hold::Refuses { param, when, why } => {
                    let name = crate::names::value_name(param);
                    let _ = writeln!(body, "        // Pinned (src/api.rs): {why}.");
                    let _ = writeln!(body, "        if {} {{ {fail} }}", when.replace("{}", &name));
                }
                Hold::Clears { param, mask, why } => {
                    let name = crate::names::value_name(param);
                    let _ = writeln!(body, "        // Pinned (src/api.rs): {why}.");
                    let _ = writeln!(body, "        let {name} = {name} & !{mask:#x};");
                }
                Hold::NullOnly { param, why } => {
                    let name = crate::names::value_name(param);
                    let _ = writeln!(body, "        // Pinned (src/api.rs): `{param}` passes only as null: {why}.");
                    let _ = writeln!(body, "        if {name}.0 != 0 {{ {fail} }}");
                }
                _ => {}
            }
        }
        // Each enumeration's judgment on a parameter passed by value (src/kinds.rs): special values cleared or refused,
        // and only declared values passed where so judged.
        for rule in self.g.value_rules(place) {
            let name = crate::names::value_name(&rule.param);
            let newtype = glued.parts.iter().any(|(n, part)| *n == name && matches!(part, Part::Value { form: super::ValueForm::Newtype, .. }));
            body.push_str(&crate::emit::kinds::rule_checks(&rule, &name, newtype, &fail));
        }
        // The caller's bytes (src/trust.rs): each length is asked once, before anything of the call is under way, and
        // that one answer both sizes our buffer and is the count the system is told; the caller's `write` runs last.
        let mut writes_back = String::new();
        for (name, part) in &glued.parts {
            let Part::BytesOut { optional, .. } = part else { continue };
            let length = if *optional { format!("{name}.map_or(0, |bytes| bytes.len())") } else { format!("{name}.len()") };
            let _ = writeln!(body, "        let Ok({name}_length) = usize::try_from({length}) else {{ {fail} }};");
            let _ = writeln!(body, "        let mut {name}_buffer = Vec::new();\n        if {name}_buffer.try_reserve_exact({name}_length).is_err() {{ {} }}\n        {name}_buffer.resize({name}_length, 0_u8);", if status { "return Err(E_OUTOFMEMORY);" } else { &fail });
            if *optional {
                let _ = writeln!(writes_back, "        if let Some(bytes) = {name} {{ bytes.write(&{name}_buffer); }}");
            } else {
                let _ = writeln!(writes_back, "        {name}.write(&{name}_buffer);");
            }
        }
        let mut args: Vec<String> = Vec::new();
        let mut after = String::new();
        let mut outs: Vec<String> = Vec::new();
        for exception in self.g.exceptions() {
            if let Exception::Checked { method: m, param, rule } = exception
                && *m == place
            {
                let name = crate::names::value_name(param);
                let optional = glued.parts.iter().any(|(n, part)| *n == name && matches!(part, Part::StructIn { optional: true, .. }));
                let _ = writeln!(body, "        // The ranges the header states (an exception line of src/api.rs): refused before the system sees them.");
                if optional {
                    let _ = writeln!(body, "        if {name}.is_some_and(|value| !crate::lengths::{rule}(value)) {{ {fail} }}");
                } else {
                    let _ = writeln!(body, "        if !crate::lengths::{rule}({name}) {{ {fail} }}");
                }
            }
            if let Exception::Lent { until, resource, subresource, .. } = exception
                && *until == place
            {
                let resource = crate::names::value_name(resource);
                let subresource = crate::names::value_name(subresource);
                let enum_path = glued.parts.iter().find(|(n, _)| *n == resource).and_then(|(_, part)| match part {
                    Part::ObjectIn { enum_path: Some(path), .. } => Some(path.clone()),
                    _ => None,
                });
                if let Some(enum_path) = enum_path {
                    let (raw, _) = self.ref_raw_and_keep(&enum_path, &resource);
                    let _ = writeln!(body, "        // The system's memory lent by Map goes first: every view of it is emptied before its Unmap.");
                    let _ = writeln!(body, "        let lent_resource: Raw = {raw};");
                    let _ = writeln!(body, "        if let Some(lent) = end_foreign_lent((self.object.raw().addr(), lent_resource.addr(), {subresource})) {{ lent.revoke(crate::KEY); }}");
                }
            }
        }
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
                    let range = glued.ranges.iter().find(|(count, ..)| *count == position);
                    // The caller's bytes were asked their length once, above; a slice's length is the language's.
                    let length = |(user, part): &(&String, &Part)| match part {
                        Part::BytesOut { .. } => format!("{user}_length"),
                        _ => format!("{user}.len()"),
                    };
                    // An `_opt_` array of values may be empty for none, whatever the count.
                    let required: Vec<&(&String, &Part)> = users.iter().filter(|(_, part)| !matches!(part, Part::ValuesIn { optional: true, .. })).collect();
                    if let Some((_, most, start, Some(keep), _)) = range {
                        // The header's keep value when every array it counts is none (an exception line).
                        let first = required.first().copied().or(users.first()).map(|(user, _)| (*user).clone()).unwrap_or_default();
                        let _ = writeln!(body, "        let {name}: {c} = match {first} {{ None => {keep:#x}, Some(values) => {{ let Ok(count) = {c}::try_from(values.len()) else {{ {fail} }}; count }} }};");
                        for (user, part) in &users {
                            if matches!(part, Part::ValuesIn { optional: true, .. }) {
                                let _ = writeln!(body, "        if {user}.is_some_and(|values| !values.is_empty() && {c}::try_from(values.len()).ok() != Some({name})) {{ {fail} }}");
                            } else {
                                let _ = writeln!(body, "        if {user}.map(|values| {c}::try_from(values.len()).ok()) != {first}.map(|values| {c}::try_from(values.len()).ok()) {{ {fail} }}");
                            }
                        }
                        let span = match start {
                            Some(start) => format!("{}.checked_add({name}).is_none_or(|end| end > {most})", glued.parts[*start].0),
                            None => format!("{name} > {most}"),
                        };
                        let _ = writeln!(body, "        if {name} != {keep:#x} && {span} {{ {fail} }}");
                    } else if let Some(first) = required.first().copied().or(users.first()) {
                        let _ = writeln!(body, "        let Ok({name}) = {c}::try_from({}) else {{ {fail} }};", length(first));
                        for other in users.iter().filter(|other| other.0 != first.0) {
                            if matches!(other.1, Part::ValuesIn { optional: true, .. }) {
                                let _ = writeln!(body, "        if !{}.is_empty() && {c}::try_from({}).ok() != Some({name}) {{ {fail} }}", other.0, length(other));
                            } else {
                                let _ = writeln!(body, "        if {c}::try_from({}).ok() != Some({name}) {{ {fail} }}", length(other));
                            }
                        }
                        if let Some((_, most, start, None, _)) = range {
                            let span = match start {
                                Some(start) => format!("{}.checked_add({name}).is_none_or(|end| end > {most})", glued.parts[*start].0),
                                None => format!("{name} > {most}"),
                            };
                            let _ = writeln!(body, "        if {span} {{ {fail} }}");
                        }
                    } else {
                        let _ = writeln!(body, "        let {name}: {c} = 0;");
                    }
                    args.push(name.clone());
                }
                Part::StructIn { optional: true, coded } if coded.decode_only => {
                    // A structure holding a union, which the glue only reads: passed when absent, refused when given.
                    let not_glued = if result.starts_with("Result<") { "return Err(E_NOTIMPL);".to_string() } else { fail.clone() };
                    let _ = writeln!(body, "        if {name}.is_some() {{ {not_glued} }}");
                    args.push("null()".to_string());
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
                    } else if enum_path.is_some() {
                        args.push(format!("{{ let held = &{name}; {raw} }}"));
                    } else {
                        args.push(format!("{{ let held = {name}; {raw} }}"));
                    }
                }
                Part::ValuesIn { element, count, optional, .. } => {
                    let kept = glued.ranges.iter().any(|(c, _, _, keep, _)| c == count && keep.is_some());
                    let c = match element.as_ref() {
                        Part::Value { c, .. } => c.clone(),
                        _ => "u32".to_string(),
                    };
                    let mapped = match element.as_ref() {
                        Part::Value { form: super::ValueForm::Newtype, .. } => ".map(|value| value.0)".to_string(),
                        Part::Value { form: super::ValueForm::Same, .. } => ".copied()".to_string(),
                        other => format!(".map(|value| {})", Self::convert_out(other, "*value")),
                    };
                    let source = if kept { format!("{name}.unwrap_or_default()") } else { name.clone() };
                    if mapped == ".copied()" {
                        let _ = writeln!(body, "        let {name}_c: Vec<{c}> = {source}.to_vec();");
                    } else {
                        let _ = writeln!(body, "        let {name}_c: Vec<{c}> = {source}.iter(){mapped}.collect();");
                    }
                    if *optional || kept {
                        args.push(format!("if {name}_c.is_empty() {{ null() }} else {{ {name}_c.as_ptr().cast::<u8>() }}"));
                    } else {
                        args.push(format!("{name}_c.as_ptr().cast::<u8>()"));
                    }
                }
                Part::ObjectsIn { count, .. } => {
                    let kept = glued.ranges.iter().any(|(c, _, _, keep, _)| c == count && keep.is_some());
                    let source = if kept { format!("{name}.unwrap_or_default()") } else { name.clone() };
                    let _ = writeln!(body, "        let {name}_raw: Vec<Raw> = {source}.iter().map(|held| held.map_or(null_mut(), |held| held.raw())).collect();");
                    if kept {
                        args.push(format!("if {name}.is_none() {{ null() }} else {{ {name}_raw.as_ptr() }}"));
                    } else {
                        args.push(format!("{name}_raw.as_ptr()"));
                    }
                }
                Part::BytesOut { optional, .. } => {
                    // Our buffer, sized by the one length asked above; the caller's bytes are written from it last.
                    if *optional {
                        args.push(format!("if {name}.is_some() {{ {name}_buffer.as_mut_ptr() }} else {{ null_mut() }}"));
                    } else {
                        args.push(format!("{name}_buffer.as_mut_ptr()"));
                    }
                }
                Part::InitialData { desc, shape, per_subresource, size, bytes_at, pitch_at, slice_at, path, fields: [bytes_field, pitch_field, slice_field] } => {
                    let desc_name = &glued.parts[*desc].0;
                    let suffix = shape.suffix();
                    if *per_subresource {
                        let _ = writeln!(body, "        let {name}_list: &[{path}] = {name};");
                        let _ = writeln!(body, "        if !{name}_list.is_empty() && crate::lengths::subresources_{suffix}({desc_name}) != Some({name}_list.len()) {{ {fail} }}");
                    } else {
                        let _ = writeln!(body, "        let {name}_list: &[{path}] = {name}.map_or(&[], core::slice::from_ref);");
                    }
                    let _ = writeln!(body, "        let mut {name}_words: Vec<u64> = Vec::new();");
                    let _ = writeln!(body, "        for (index, record) in {name}_list.iter().enumerate() {{");
                    let _ = writeln!(body, "            // The system reads the length the shape's rule gives (an exception line of src/api.rs): no fewer bytes are passed.");
                    let _ = writeln!(body, "            let Some(needed) = crate::lengths::initial_length_{suffix}({desc_name}, index, record.{pitch_field}, record.{slice_field}) else {{ {fail} }};");
                    let _ = writeln!(body, "            if record.{bytes_field}.len() < needed {{ {fail} }}");
                    let _ = writeln!(body, "            let mut bytes = [0_u8; {size}];");
                    let _ = writeln!(body, "            put_usize(&mut bytes, {bytes_at}, record.{bytes_field}.as_ptr().expose_provenance());");
                    let _ = writeln!(body, "            put_u32(&mut bytes, {pitch_at}, record.{pitch_field});\n            put_u32(&mut bytes, {slice_at}, record.{slice_field});");
                    let _ = writeln!(body, "            {name}_words.extend(bytes.as_chunks::<8>().0.iter().map(|word| u64::from_ne_bytes(*word)));\n        }}");
                    args.push(format!("if {name}_list.is_empty() {{ null() }} else {{ {name}_words.as_ptr().cast::<u8>() }}"));
                }
                Part::UnknownIn { optional } => {
                    // The UnknownRef of the API whose interface holds the method (a face's is its own API's).
                    let owner = place.split_once("::").and_then(|(interface, _)| self.g.interface_api.iter().find(|(k, _)| k.1 == interface).map(|(_, &a)| a)).unwrap_or(self.current.get());
                    let kinds: Vec<usize> = (0..self.g.kinds.len()).filter(|&k| self.g.kinds[k].api == owner).collect();
                    let path = format!("{}::UnknownRef", alias(self.g.apis[owner]));
                    // An `_opt_` pointer reaches the trait as an `Option`; a required one as the reference itself.
                    let wrap = |pattern: String| if *optional { format!("Some({pattern})") } else { pattern };
                    let mut arms: Vec<String> = kinds.iter().map(|&k| format!("{} => object.raw()", wrap(format!("{path}::{}(object)", self.g.kinds[k].name)))).collect();
                    arms.push(format!("{} => match foreign.object().downcast_ref::<ForeignUnknown>() {{ Some(held) => held.raw(), None => {{ {fail} }} }}", wrap(format!("{path}::Unknown(foreign)"))));
                    if *optional {
                        arms.push("None => null_mut()".to_string());
                    }
                    let _ = writeln!(body, "        let {name}_raw: Raw = match {name} {{ {} }};", arms.join(", "));
                    args.push(format!("{name}_raw"));
                }
                Part::Riid => {
                    let bound = glued.parts.iter().find_map(|(_, part)| match part {
                        Part::BoundOut { name } => Some(name.clone()),
                        _ => None,
                    }).unwrap_or_default();
                    let top = self.g.top_path(self.current.get(), usize::MAX);
                    let _ = writeln!(body, "        let {name} = guid_bytes(&<<{api_type} as {top}>::{bound} as Adopt>::INTERFACE);");
                    args.push(format!("{name}.as_ptr()"));
                }
                Part::BoundOut { name: bound } => {
                    let top = self.g.top_path(self.current.get(), usize::MAX);
                    let _ = writeln!(body, "        let mut {name}: Raw = null_mut();");
                    args.push(format!("&raw mut {name}"));
                    let _ = writeln!(after, "        #[expect(unsafe_code, reason = \"{place}: {}, Out and ComOutPtr\")]", param.name);
                    let _ = writeln!(after, "        // SAFETY: the metadata's `{}` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` named, which is `Adopt::INTERFACE`.", param.name);
                    let _ = writeln!(after, "        let Some({name}) = (unsafe {{ <<{api_type} as {top}>::{bound} as Adopt>::adopt({name}) }}) else {{ return Err(E_FAIL); }};");
                    outs.push(name.clone());
                }
                Part::InterfaceOut { riid } => {
                    // The UnknownObject of the API whose interface holds the method (a face's is its own API's).
                    let owner = place.split_once("::").and_then(|(interface, _)| self.g.interface_api.iter().find(|(k, _)| k.1 == interface).map(|(_, &a)| a)).unwrap_or(self.current.get());
                    let adopt = if owner == self.current.get() { "unknown_object_of".to_string() } else { format!("{}::unknown_object_of", direct_alias(self.g.apis[owner].folder)) };
                    let riid_name = &glued.parts[*riid].0;
                    let _ = writeln!(body, "        let mut {name}: Raw = null_mut();");
                    args.push(format!("&raw mut {name}"));
                    let _ = writeln!(after, "        #[expect(unsafe_code, reason = \"{place}: {}, Out and ComOutPtr\")]", param.name);
                    let _ = writeln!(after, "        // SAFETY: the metadata's `{}` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `{riid_name}` names.", param.name);
                    let _ = writeln!(after, "        let Some({name}) = (unsafe {{ {adopt}::<{api_type}>({riid_name}, {name}) }}) else {{ return Err(E_FAIL); }};");
                    outs.push(name.clone());
                }
                Part::LentOut { coded, resource, subresource, fields: [data, row, depth] } => {
                    let resource_name = &glued.parts[*resource].0;
                    let subresource_name = &glued.parts[*subresource].0;
                    let Some(Part::ObjectIn { enum_path: Some(enum_path), .. }) = glued.parts.get(*resource).map(|(_, part)| part) else { continue };
                    let (raw, keep) = self.ref_raw_and_keep(enum_path, resource_name);
                    let until_slot = self.g.lent_of(place).and_then(|exception| match exception {
                        Exception::Lent { until, .. } => until.split_once("::"),
                        _ => None,
                    }).and_then(|(interface, method)| {
                        let trait_name = self.g.trait_names.iter().find(|(k, _)| k.1 == interface).map(|(_, name)| name.clone())?;
                        Some(format!("{}_{}", snake(&trait_name), snake(method)))
                    }).unwrap_or_default();
                    let _ = writeln!(body, "        let lent_resource: Raw = {raw};");
                    let _ = writeln!(body, "        let lent_key = (self.object.raw().addr(), lent_resource.addr(), {subresource_name});");
                    let _ = writeln!(body, "        // A subresource mapped through us is not mapped again: the first one's memory would die under its view.");
                    let _ = writeln!(body, "        let Some(pending) = PendingLent::reserve(lent_key) else {{ {fail} }};");
                    let _ = writeln!(body, "        let mut {name} = Aligned::<{}>::default();", coded.size);
                    args.push(format!("{name}.0.as_mut_ptr()"));
                    let _ = writeln!(after, "        let {name}_record = decode_{}(&{name}.0);", coded.ident);
                    let _ = writeln!(after, "        let lent_length = mapped_length(&{resource_name}, {subresource_name}, {name}_record.{row}, {name}_record.{depth});");
                    let _ = writeln!(after, "        let Some(lent_length) = lent_length.filter(|_| {name}_record.{data} != 0) else {{");
                    let _ = writeln!(after, "            #[expect(unsafe_code, reason = \"{place}: the system's Unmap of memory no view holds\")]");
                    let _ = writeln!(after, "            // SAFETY: a live object whose table holds Unmap where the metadata orders it; the resource lives while `{resource_name}` does.");
                    let _ = writeln!(after, "            unsafe {{ (self.object.entry_points().{until_slot})(self.object.raw(), lent_resource, {subresource_name}) }};");
                    let _ = writeln!(after, "            return Err(E_FAIL);\n        }};");
                    let _ = writeln!(after, "        let lent_keep: Vec<Box<dyn Send + Sync>> = vec![Box::new(self.clone()), {keep}];");
                    let _ = writeln!(after, "        #[expect(unsafe_code, reason = \"{place}: {}, memory the system lends until its Unmap\")]", param.name);
                    let _ = writeln!(after, "        // SAFETY: `{}`'s exception line (src/api.rs): after a success the system lends `lent_length` bytes at `{data}` (the shape's rule from the description and the pitches it returned) until its Unmap of this resource and subresource, which this glue calls only after revoking the view; `lent_keep` holds the context and the resource.", param.name);
                    let _ = writeln!(after, "        let lent = unsafe {{ ForeignLent::new({name}_record.{data}, lent_length, lent_keep) }};");
                    let _ = writeln!(after, "        pending.settle(lent.clone());");
                    let record_path = coded.path.trim_end_matches("<usize>").to_string();
                    let _ = writeln!(after, "        let {name} = {record_path} {{ {data}: lent, {row}: {name}_record.{row}, {depth}: {name}_record.{depth} }};");
                    outs.push(name.clone());
                }
                Part::ObjectOut { kinds, made, .. } => {
                    let _ = writeln!(body, "        let mut {name}: Raw = null_mut();");
                    args.push(format!("&raw mut {name}"));
                    let interface_key = self.out_interface(param);
                    let newest = self.g.kinds[kinds[0]].newest.first().cloned();
                    let adopt = if newest.as_ref() == interface_key.as_ref() { "from_owned" } else { "from_owned_older" };
                    let system = self.system_path(kinds[0], api_type);
                    let _ = writeln!(after, "        #[expect(unsafe_code, reason = \"{place}: {}, {}\")]", param.name, if *made { "Out and ComOutPtr" } else { "an object the metadata's Out hands over" });
                    let _ = writeln!(after, "        // SAFETY: the metadata's `{}` is `Out`{}: after a success, null or one reference to an object of `{}`.", param.name, if *made { " and `ComOutPtr`" } else { "" }, interface_key.as_ref().map_or("", |k| k.1.as_str()));
                    if *made {
                        let _ = writeln!(after, "        let Some({name}) = (unsafe {{ {system}::{adopt}({name}) }}) else {{ return Err(E_FAIL); }};");
                        let protected = self.g.exceptions().any(|exception| matches!(exception, Exception::ProtectThreads { kind, .. } if self.g.kinds[kinds[0]].root.1 == *kind));
                        if protected && !this {
                            let _ = writeln!(after, "        protect_threads(&{name})?;");
                        }
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
        let _ = writeln!(body, "        #[expect(unsafe_code, reason = \"{place}{}, called out\")]", if this { "" } else { ", the library's export" });
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
        body.push_str(&writes_back);
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

    /// The raw pointer of a `<Interface>Ref` of the calling glue's objects, and a clone of the object, boxed.
    fn ref_raw_and_keep(&self, enum_path: &str, value: &str) -> (String, String) {
        let kinds = self.g.kinds_deriving(&self.enum_interface(enum_path));
        let raw: Vec<String> = kinds.iter().map(|&k| format!("{enum_path}::{}(object) => object.raw()", self.g.kinds[k].name)).collect();
        let keep: Vec<String> = kinds.iter().map(|&k| format!("{enum_path}::{}(object) => Box::new((*object).clone())", self.g.kinds[k].name)).collect();
        (format!("match &{value} {{ {} }}", raw.join(", ")), format!("match &{value} {{ {} }}", keep.join(", ")))
    }

    /// The interface a `<Interface>Ref` enum is of, by its path: two APIs may each have a trait of that name (Direct3D 11's
    /// and DXGI's `Resource`), so the crate alias the path starts with picks the API, and the smallest key decides any
    /// tie, never a hash map's order.
    pub(super) fn enum_interface(&self, enum_path: &str) -> Key {
        let (prefix, last) = enum_path.rsplit_once("::").unwrap_or(("", enum_path));
        let name = last.trim_end_matches("Ref");
        let api = self.g.apis.iter().position(|spec| alias(spec) == prefix);
        self.g
            .trait_names
            .iter()
            .filter(|(k, trait_name)| *trait_name == name && api.is_none_or(|api| self.g.interface_api.get(*k) == Some(&api)))
            .map(|(k, _)| k.clone())
            .min()
            .unwrap_or_default()
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
        text.push_str("use wcontainers_component_object_model::structures::{Guid, HResult};\n");
        text.push_str("use wcontainers_component_object_model_direct::{Adopt, Aligned, ForeignLent, ForeignObject, ForeignUnknown, Raw, UnknownTable, guid_bytes, succeeded};\n");
        text.push_str("use wcontainers_volatile_bytes::VolatileBytes;\n\nuse crate::abi::*;\nuse crate::data::*;\nuse crate::serve::*;\n\n");
        // Memory the system lends from Map to Unmap, by context, resource and subresource.
        if spec.exceptions.iter().any(|e| matches!(e, Exception::Lent { .. })) {
            text.push_str(LENT_BOOKKEEPING);
        }
        // Multithread protection, turned on for each new object of a kind that needs it.
        for exception in spec.exceptions {
            let Exception::ProtectThreads { kind, interface, method } = exception else { continue };
            let Some(kind_index) = g.interface_api.keys().find(|k| k.1 == *kind).and_then(|k| g.kind_of.get(k)).copied() else { continue };
            let Some(interface_key) = g.interface_api.keys().find(|k| k.1 == *interface).cloned() else { continue };
            let trait_name = &g.trait_names[&interface_key];
            let table = format!("{trait_name}Table");
            let slot = format!("{}_{}", snake(trait_name), snake(method));
            let system = format!("System{}", g.kinds[kind_index].name);
            let iid = self.iid(&interface_key);
            let _ = writeln!(text, "/// Turns on the system's multithread protection of a new object (an exception line of src/api.rs), under which it and the");
            let _ = writeln!(text, "/// context made with it are free threaded, as their `Send` and `Sync` promise; refuses the object when it cannot.");
            let _ = writeln!(text, "fn protect_threads<A>(object: &{system}<A>) -> Result<(), HResult> {{");
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"QueryInterface for {interface}\")]\n    // SAFETY: `{iid}` names the interface `{table}` lays out.");
            let _ = writeln!(text, "    let Some(protection) = (unsafe {{ object.object.query::<{table}>(&{iid}) }}) else {{ return Err(E_NOINTERFACE); }};");
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"{interface}::{method}, called out\")]\n    // SAFETY: a live object whose table holds `{method}` where the metadata orders it; TRUE is a BOOL.");
            let _ = writeln!(text, "    let _ = unsafe {{ (protection.entry_points().{slot})(protection.raw(), 1) }};\n    Ok(())\n}}\n");
        }
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
            let _ = writeln!(text, "    /// Takes over a reference to an object of `{}`, or returns none for null.\n    ///\n    /// # Safety\n    /// `raw` is null or a live object of `{0}` whose reference the caller gives up, and which may be called from any\n    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).", newest.1);
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"the promise that a pointer is a live object of an interface\")]");
            let _ = writeln!(text, "    pub unsafe fn from_owned(raw: Raw) -> Option<{name}<A>> {{\n        #[deny(unsafe_code)]\n        let body = move || {{");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"the promise of the caller: a live object of {}\")]\n            // SAFETY: the caller's promise.", newest.1);
            let _ = writeln!(text, "            let object = unsafe {{ ForeignObject::<{table}>::from_owned(raw) }}?;\n            Some({name} {{ object, api: PhantomData }})\n        }};\n        body()\n    }}\n");
            let _ = writeln!(text, "    /// Takes over a reference to an object of an older interface of the kind, asking it for `{}`.\n    ///\n    /// # Safety\n    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread\n    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).", newest.1);
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"the promise that a pointer is a live COM object\")]");
            let _ = writeln!(text, "    pub unsafe fn from_owned_older(raw: Raw) -> Option<{name}<A>> {{\n        #[deny(unsafe_code)]\n        let body = move || {{");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"the promise of the caller: a live COM object, which is an IUnknown\")]\n            // SAFETY: the caller's promise: any COM object is an IUnknown.");
            let _ = writeln!(text, "            let unknown = unsafe {{ ForeignObject::<UnknownTable>::from_owned(raw) }}?;");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"QueryInterface for the newest interface of the kind\")]\n            // SAFETY: `{iid}` names the interface `{table}` lays out.");
            let _ = writeln!(text, "            let object = unsafe {{ unknown.query::<{table}>(&{iid}) }}?;\n            Some({name} {{ object, api: PhantomData }})\n        }};\n        body()\n    }}\n}}\n");
            let _ = writeln!(text, "impl<A> Adopt for {name}<A> {{\n    const INTERFACE: Guid = {iid};\n");
            let _ = writeln!(text, "    #[expect(unsafe_code, reason = \"the promise that a pointer is a live object of an interface\")]\n    unsafe fn adopt(raw: Raw) -> Option<{name}<A>> {{\n        #[deny(unsafe_code)]\n        let body = move || {{");
            let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"the promise of the caller: a live object of {}, the newest interface of the kind\")]\n            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.", newest.1);
            let _ = writeln!(text, "            unsafe {{ {name}::from_owned(raw) }}\n        }};\n        body()\n    }}\n}}\n");
            let _ = writeln!(text, "impl<A> Clone for {name}<A> {{\n    fn clone(&self) -> {name}<A> {{\n        {name} {{ object: self.object.clone(), api: PhantomData }}\n    }}\n}}\n");
            let _ = writeln!(text, "impl<A> PartialEq for {name}<A> {{\n    fn eq(&self, other: &{name}<A>) -> bool {{\n        self.object == other.object\n    }}\n}}\n\nimpl<A> Eq for {name}<A> {{}}\n");
            let _ = writeln!(text, "impl<A> Hash for {name}<A> {{\n    fn hash<H: Hasher>(&self, state: &mut H) {{\n        self.object.hash(state);\n    }}\n}}\n");
        }
        // An object handed out by interface id: as the kind that answers it, else as a foreign object.
        if g.needs_unknown.borrow().contains(&api) {
            let enum_path = format!("{}::UnknownObject", alias(spec));
            let _ = writeln!(text, "/// Takes over a reference to an object of the interface `riid` names: as the kind of this API that answers it (each");
            let _ = writeln!(text, "/// candidate asked by QueryInterface), else as a foreign object; none for null.\n///\n/// # Safety");
            let _ = writeln!(text, "/// `raw` is null or a live COM object of the interface `riid` names, whose reference the caller gives up, and which may be\n/// called from any thread as `Interface` states (a context only under the system's multithread protection).");
            let _ = writeln!(text, "#[expect(unsafe_code, reason = \"the promise that a pointer is a live COM object\")]");
            let _ = writeln!(text, "pub unsafe fn unknown_object_of<A: System>(riid: &Guid, raw: Raw) -> Option<{enum_path}<A>> {{\n    #[deny(unsafe_code)]\n    let body = move || {{");
            let _ = writeln!(text, "        #[expect(unsafe_code, reason = \"the promise of the caller: a live COM object, which is an IUnknown\")]\n        // SAFETY: the caller's promise: any COM object is an IUnknown; the reference it gave up is released when this drops.");
            let _ = writeln!(text, "        let unknown = unsafe {{ ForeignObject::<UnknownTable>::from_owned(raw) }}?;");
            for &kind in &(0..g.kinds.len()).filter(|&k| g.kinds[k].api == api).collect::<Vec<_>>() {
                let k = &g.kinds[kind];
                let newest = k.newest.first().cloned().unwrap_or_else(|| k.root.clone());
                let table = format!("{}Table", g.trait_names[&newest]);
                let iids: Vec<String> = k.ancestors.iter().filter(|a| g.interface_api.contains_key(*a)).map(|a| self.iid(a)).collect();
                let protected = self.g.exceptions().any(|exception| matches!(exception, Exception::ProtectThreads { kind: protected, .. } if k.root.1 == *protected));
                let _ = writeln!(text, "        if [{}].contains(riid) {{", iids.join(", "));
                let _ = writeln!(text, "            #[expect(unsafe_code, reason = \"QueryInterface for the newest interface of the kind\")]\n            // SAFETY: `{}` names the interface `{table}` lays out.", self.iid(&newest));
                let _ = writeln!(text, "            if let Some(object) = unsafe {{ unknown.query::<{table}>(&{}) }} {{", self.iid(&newest));
                let _ = writeln!(text, "                let object = System{} {{ object, api: PhantomData }};", k.name);
                if protected {
                    let _ = writeln!(text, "                protect_threads(&object).ok()?;");
                }
                let _ = writeln!(text, "                return Some({enum_path}::{}(object));\n            }}\n        }}", k.name);
            }
            let _ = writeln!(text, "        #[expect(unsafe_code, reason = \"the promise of the caller: a live COM object, which is an IUnknown\")]\n        // SAFETY: the caller's promise; a reference of our own is taken.");
            let _ = writeln!(text, "        let foreign = unsafe {{ ForeignUnknown::from_borrowed(unknown.raw()) }}?;");
            let _ = writeln!(text, "        Some({enum_path}::Unknown(wcontainers_component_object_model::structures::Foreign::new(unknown.raw().addr(), std::sync::Arc::new(foreign))))\n    }};\n    body()\n}}\n");
        }
        // The trait every API value of the calling glue meets: its kinds are this crate's object types.
        let mut bindings: Vec<String> = own_kinds.iter().map(|&k| format!("{} = System{}<Self>", g.kinds[k].name, g.kinds[k].name)).collect();
        for exception in spec.exceptions {
            match exception {
                Exception::BoundObject { name, .. } => bindings.push(format!("{name}: Adopt")),
                Exception::Lent { .. } if !bindings.iter().any(|b| b.starts_with("Mapped")) => bindings.push("Mapped = ForeignLent".to_string()),
                _ => {}
            }
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
            // Each trait once per type: a face's chain may share bases with the kind's or another face's.
            let mut implemented: Vec<Key> = traits.clone();
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
                let face_table = if face_api == api { format!("{}Table", g.trait_names[face]) } else { format!("{}::{}Table", direct_alias(g.apis[face_api].folder), g.trait_names[face]) };
                let face_iid = self.iid(face);
                for interface_key in self.chain(face) {
                    if implemented.contains(&interface_key) {
                        continue;
                    }
                    implemented.push(interface_key.clone());
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
                        // The face's trait names its own API's types (its UnknownRef, its Object enums).
                        let (inputs, result) = self.trait_signature(slot.method, face_api, "Self::Api", &place);
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
            for exception in spec.exceptions {
                match exception {
                    Exception::BoundObject { name, kind, .. } => {
                        if let Some(&kind_index) = g.interface_api.keys().find(|k| k.1 == *kind).and_then(|k| g.kind_of.get(k)) {
                            top_bindings.push(format!("    type {name} = System{}<Self>;", g.kinds[kind_index].name));
                        }
                    }
                    Exception::Lent { .. } if !top_bindings.iter().any(|b| b.contains("type Mapped")) => top_bindings.push("    type Mapped = ForeignLent;".to_string()),
                    _ => {}
                }
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
                for exception in spec.exceptions {
                    if let Exception::Binds { api: bound_api, name, kind } = exception
                        && bound_api == folder
                        && let Some(&kind_index) = g.interface_api.keys().find(|k| k.1 == *kind).and_then(|k| g.kind_of.get(k))
                    {
                        lines.push(format!("    type {name} = System{}<Self>;", g.kinds[kind_index].name));
                    }
                }
                let mut other_functions = String::new();
                let other_spec = g.apis[other];
                let mut listed: Vec<&Method> = g
                    .read
                    .functions
                    .iter()
                    .filter(|((namespace, _), _)| other_spec.namespaces.contains(&namespace.as_str()))
                    .map(|(_, function)| function)
                    .filter(|function| other_spec.library.is_some_and(|library| function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library))))
                    .filter(|function| other_spec.only.is_empty() || other_spec.only.contains(&function.name.as_str()))
                    .collect();
                listed.sort_by(|a, b| a.name.cmp(&b.name));
                for function in listed {
                    let rust_name = names::escaped(snake(crate::names::strip_first(&function.name, other_spec.function_prefixes)));
                    let mut renamed = function.clone();
                    renamed.name = crate::names::strip_first(&function.name, other_spec.function_prefixes).to_string();
                    let (inputs, result) = self.trait_signature(&renamed, other, "Self", &function.name);
                    let parameters: String = inputs.iter().map(|(n, t)| format!(", {n}: {t}")).collect();
                    let returns = if result == "()" { String::new() } else { format!(" -> {result}") };
                    let _ = writeln!(other_functions, "    fn {rust_name}(&self{parameters}){returns} {{");
                    for (n, _) in &inputs {
                        let _ = writeln!(other_functions, "        let _ = {n};");
                    }
                    let answer = if result.starts_with("Result<") { "Err(E_NOTIMPL)" } else if result.starts_with("Option<") { "None" } else if result == "()" { "" } else { "Default::default()" };
                    let _ = writeln!(other_functions, "        // Another library's export ({}): its own API's System calls it; this one never loads that library.\n        {answer}\n    }}", other_spec.library.unwrap_or(""));
                }
                let _ = writeln!(text, "impl {other_top} for {name} {{\n{}\n{other_functions}}}\n", lines.join("\n"));
                let _ = writeln!(text, "impl {other_direct}::System for {name} {{}}\n");
            }
        }
        let _ = upper;
        let _ = alias;
        text
    }
}

use crate::names;

/// The calling glue's record of the system's memory lent by Map, until its Unmap: by context, resource and subresource.
const LENT_BOOKKEEPING: &str = r#"/// The system's memory each Map lent and no Unmap has ended, by context, resource and subresource; none while a Map of
/// it is under way.
static FOREIGN_LENT: std::sync::Mutex<Vec<((usize, usize, u32), Option<ForeignLent>)>> = std::sync::Mutex::new(Vec::new());

/// A subresource held busy while the system's Map runs, freed when this drops unless settled.
struct PendingLent((usize, usize, u32));

impl PendingLent {
    /// Marks the subresource busy, or returns none when it is lent or busy.
    fn reserve(key: (usize, usize, u32)) -> Option<PendingLent> {
        let mut held = FOREIGN_LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.iter().any(|(other, _)| *other == key) {
            return None;
        }
        held.push((key, None));
        Some(PendingLent(key))
    }

    /// Records the memory lent, until the Unmap that ends it.
    fn settle(self, lent: ForeignLent) {
        let mut held = FOREIGN_LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = held.iter_mut().find(|(other, _)| *other == self.0) {
            entry.1 = Some(lent);
        }
        drop(held);
        core::mem::forget(self);
    }
}

impl Drop for PendingLent {
    fn drop(&mut self) {
        let removed = {
            let mut held = FOREIGN_LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            held.iter().position(|(other, _)| *other == self.0).map(|at| held.swap_remove(at))
        };
        drop(removed);
    }
}

/// Returns how many subresources the system has lent through this glue and no Unmap has ended, which a test reads.
pub fn foreign_lent_outstanding() -> usize {
    FOREIGN_LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()
}

/// Takes the memory lent for the subresource, which the caller revokes before the system's Unmap.
fn end_foreign_lent(key: (usize, usize, u32)) -> Option<ForeignLent> {
    let mut held = FOREIGN_LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let at = held.iter().position(|(other, lent)| *other == key && lent.is_some())?;
    held.swap_remove(at).1
}

"#;
