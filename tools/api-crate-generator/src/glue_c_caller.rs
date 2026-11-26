//! The hostile C caller (AC3): a C program, written from the same tables the serving glue fills, that calls every
//! entry of every table and every export with raw pointers and counts of its own choosing, as any C program may.
//!
//! What it passes stays inside the C contract the serving glue's marks rest on (each pointer null or covering what
//! the metadata says it covers, for the count C chose), and is hostile everywhere else: `this` null, released
//! (stale), an object of another kind, or an address that is no object; object parameters the same; counts of 0,
//! small, past the header's ranges, at and past the glue's limit (`MOST_ELEMENTS`, then with one element's memory,
//! which the glue must refuse before reading) and all ones; every buffer allocated at exactly the size the count gives,
//! so that AddressSanitizer reports a read or write one byte past it; structures of random bytes; interface ids known
//! and random; out-pointers null or garbage-filled. Objects the calls hand out are kept per interface and used again,
//! released at random, and every reference given back at the end of a round. Initial data is passed as null (C would
//! have to know the length the shape's rule gives; the slice's harness tests it). Written into
//! `crates/direct3d-11/implementation/direct/c-caller`, with a Rust test that serves the hostile implementation's
//! exports to it under a lie picked per round.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::{Glued, Part, Writer, direct_alias, snake};
use crate::emit::{HEADER, Key, package};
use crate::metadata::Method;

/// The glue's limit on array elements, as a number for C (src/glue.rs `MOST_ELEMENTS`).
const MOST: u64 = 1 << 16;

/// What one run counts, in the order the C writes them.
const COUNTED: [&str; 19] = [
    "calls", "absent", "skipped", "succeeded", "failed", "adopted", "released", "stale_this", "wrong_this", "null_this", "garbage_this", "big_counts", "past_limit", "null_pointers", "exports_called", "query_interfaces", "query_left_pointer", "refused", "refused_wrong",
];

/// The C type of a Rust C-side type the glue's `c_signature` names.
fn c_type(rust: &str) -> &'static str {
    match rust {
        "Raw" => "void *",
        "*const u8" => "const uint8_t *",
        "*mut u8" => "uint8_t *",
        "*const Raw" => "void *const *",
        "*mut Raw" => "void **",
        "i8" => "int8_t",
        "u8" => "uint8_t",
        "i16" => "int16_t",
        "u16" => "uint16_t",
        "i32" => "int32_t",
        "u32" => "uint32_t",
        "i64" => "int64_t",
        "u64" => "uint64_t",
        "isize" => "intptr_t",
        "usize" => "uintptr_t",
        "f32" => "float",
        "f64" => "double",
        "()" => "void",
        super::GUID_ABI => "GuidAbi",
        _ => "",
    }
}

/// The C caller's view of the set: every interface, its id, base and table, and the exports.
struct Plan {
    interfaces: Vec<Key>,
    index: BTreeMap<Key, usize>,
}

impl Writer<'_, '_> {
    /// One call of a table entry or an export, as C code inside a `case`: the arguments chosen, the call through the
    /// function read from the table (or the export's address), and what it handed out kept.
    fn c_call(&self, plan: &Plan, glued: &Glued, method: &Method, function: &str, this: Option<&str>) -> Result<String, String> {
        let (parameters, returned) = self.c_signature(glued, method);
        let mut types: Vec<String> = Vec::new();
        if this.is_some() {
            types.push("void *".to_string());
        }
        for (name, rust) in &parameters {
            let c = c_type(rust);
            if c.is_empty() {
                return Err(format!("{}: parameter {name} of C type {rust} the C caller has no form for", method.name));
            }
            types.push(c.to_string());
        }
        let ret = c_type(&returned);
        if ret.is_empty() {
            return Err(format!("{}: a return of C type {returned} the C caller has no form for", method.name));
        }
        let mut text = String::new();
        let _ = writeln!(text, "        typedef {ret} (*F)({});", if types.is_empty() { "void".to_string() } else { types.join(", ") });
        let _ = writeln!(text, "        F f = (F)({function});\n        if (!f) {{ fprintf(stderr, \"C CALLER: a null entry\\n\"); abort(); }}");
        // Counts first: each array it counts is allocated by it.
        let mut counts: BTreeMap<usize, String> = BTreeMap::new();
        // A count passed by value of an array out, and a count by pointer, are counts too: C allocates by them.
        let out_counts: Vec<usize> = glued.parts.iter().filter_map(|(_, part)| match part {
            Part::ValuesOut { count, .. } | Part::ObjectsOut { count, .. } => Some(*count),
            _ => None,
        }).collect();
        for (position, (_, part)) in glued.parts.iter().enumerate() {
            if matches!(part, Part::Count | Part::CountByPointer) || out_counts.contains(&position) {
                let _ = writeln!(text, "        uint64_t n{position} = pick_count(s);");
                counts.insert(position, format!("n{position}"));
            }
        }
        // A parameter whose GUID names the interface an object out is of: an interface id C chooses, known or not.
        let riids: Vec<usize> = glued.parts.iter().filter_map(|(_, part)| match part {
            Part::InterfaceOut { riid } => Some(*riid),
            _ => None,
        }).collect();
        let mut args: Vec<String> = Vec::new();
        if let Some(this) = this {
            args.push(this.to_string());
        }
        let mut after = String::new();
        let mut frees: Vec<String> = Vec::new();
        for (position, ((_, part), (_, rust))) in glued.parts.iter().zip(&parameters).enumerate() {
            let c = c_type(rust);
            let a = format!("a{position}");
            if riids.contains(&position) || matches!(part, Part::Riid) {
                let _ = writeln!(text, "        int {a}_interface = -1;\n        uint8_t *{a} = pick_riid(s, &{a}_interface);");
                frees.push(a.clone());
                args.push(a);
                continue;
            }
            match part {
                Part::Value { .. } if out_counts.contains(&position) => {
                    let _ = writeln!(text, "        {c} {a} = ({c})n{position};");
                }
                Part::Value { c: value_type, .. } => {
                    let value = if value_type == super::GUID_ABI {
                        "pick_guid(s)".to_string()
                    } else if value_type.starts_with('f') {
                        format!("({c})pick_float(s)")
                    } else {
                        format!("({c})pick_value(s)")
                    };
                    let _ = writeln!(text, "        {c} {a} = {value};");
                }
                Part::Count => {
                    let _ = writeln!(text, "        {c} {a} = ({c})n{position};");
                }
                Part::StructIn { coded, .. } => {
                    let _ = writeln!(text, "        uint8_t *{a} = maybe_bytes(s, {});", coded.size);
                    frees.push(a.clone());
                }
                Part::ValuesIn { count, size, .. } => {
                    let n = counts.get(count).cloned().unwrap_or_else(|| "0".to_string());
                    let _ = writeln!(text, "        uint8_t *{a} = elements(s, {n}, {size});");
                    frees.push(a.clone());
                }
                Part::ObjectsIn { count, .. } => {
                    let n = counts.get(count).cloned().unwrap_or_else(|| "0".to_string());
                    let _ = writeln!(text, "        void **{a} = (void **)elements(s, {n}, sizeof(void *));");
                    let _ = writeln!(text, "        if ({a}) for (uint64_t i = 0; i < ({n} > MOST ? 1 : {n}); i++) {a}[i] = pick_object(s, 1);");
                    frees.push(a.clone());
                }
                Part::BytesOut { count, .. } => {
                    let n = counts.get(count).cloned().unwrap_or_else(|| "0".to_string());
                    let _ = writeln!(text, "        uint8_t *{a} = out_bytes(s, {n});");
                    frees.push(a.clone());
                }
                Part::FixedIn { length, size, .. } => {
                    let _ = writeln!(text, "        uint8_t *{a} = maybe_bytes(s, {});", length * size);
                    frees.push(a.clone());
                }
                Part::FixedOut { length, size, .. } => {
                    let _ = writeln!(text, "        uint8_t *{a} = out_bytes_fixed(s, {});", length * size);
                    frees.push(a.clone());
                }
                Part::CountByPointer => {
                    let _ = writeln!(text, "        uint8_t *{a} = (uint8_t *)count_cell(s, n{position});");
                    frees.push(a.clone());
                }
                Part::ValuesOut { count, size, .. } => {
                    let n = counts.get(count).cloned().unwrap_or_else(|| "0".to_string());
                    let _ = writeln!(text, "        uint8_t *{a} = out_elements(s, {n}, {size});");
                    frees.push(a.clone());
                }
                Part::ObjectsOut { count, .. } => {
                    let n = counts.get(count).cloned().unwrap_or_else(|| "0".to_string());
                    let _ = writeln!(text, "        void **{a} = (void **)out_elements(s, {n}, sizeof(void *));");
                    let interface = self.out_interface(&method.params[position]).and_then(|key| plan.index.get(&key).copied()).map_or("-1".to_string(), |index| index.to_string());
                    // Within the glue's limit the glue writes every element (a reference or null); past it, none.
                    let _ = writeln!(after, "        if ({a} && {n} <= MOST) for (uint64_t i = 0; i < {n}; i++) adopt(s, ok ? {a}[i] : NULL, {interface});");
                    frees.push(a.clone());
                }
                Part::InitialData { .. } => {
                    let _ = writeln!(text, "        const uint8_t *{a} = NULL;");
                }
                Part::ObjectIn { .. } => {
                    let _ = writeln!(text, "        void *{a} = pick_object(s, 1);");
                }
                Part::UnknownIn { .. } => {
                    // A foreign IUnknown is called by the glue: only a live COM object, ours, a released one of ours, or
                    // the caller's own foreign object.
                    let _ = writeln!(text, "        void *{a} = pick_object(s, 0);");
                }
                Part::Riid => {}
                Part::ObjectOut { .. } | Part::BoundOut { .. } | Part::InterfaceOut { .. } => {
                    let _ = writeln!(text, "        void **{a} = (void **)out_bytes_fixed(s, sizeof(void *));");
                    let interface = match part {
                        Part::ObjectOut { .. } => {
                            let param = &method.params[position];
                            self.out_interface(param).and_then(|key| plan.index.get(&key).copied()).map_or("-1".to_string(), |index| index.to_string())
                        }
                        Part::BoundOut { .. } => glued.parts.iter().position(|(_, p)| matches!(p, Part::Riid)).map_or("-1".to_string(), |riid| format!("a{riid}_interface")),
                        Part::InterfaceOut { riid } => format!("a{riid}_interface"),
                        _ => "-1".to_string(),
                    };
                    let _ = writeln!(after, "        if ({a}) adopt(s, ok ? *{a} : NULL, {interface});");
                    frees.push(a.clone());
                }
                Part::ValueOut { size, .. } => {
                    let _ = writeln!(text, "        uint8_t *{a} = out_bytes_fixed(s, {size});");
                    frees.push(a.clone());
                }
                Part::StructOut { coded } | Part::LentOut { coded, .. } => {
                    let _ = writeln!(text, "        uint8_t *{a} = out_bytes_fixed(s, {});", coded.size);
                    frees.push(a.clone());
                }
            }
            args.push(a);
        }
        let call = format!("f({})", args.join(", "));
        if glued.status {
            let _ = writeln!(text, "        int ok = {call} >= 0;");
        } else if ret == "void" {
            let _ = writeln!(text, "        {call};\n        int ok = 1;");
        } else {
            let _ = writeln!(text, "        (void){call};\n        int ok = 1;");
        }
        let _ = writeln!(text, "        s->counted[ok ? SUCCEEDED : FAILED]++;");
        text.push_str(&after);
        for a in frees {
            let _ = writeln!(text, "        free((void *){a});");
        }
        Ok(text)
    }

    /// The C caller's source, the exports' fields in the order C takes them, and how many entries it calls.
    fn c_caller(&self, plan: &Plan) -> Result<(String, Vec<(usize, String)>, usize), String> {
        let g = self.g;
        let mut c = String::new();
        let _ = writeln!(c, "/* {HEADER}\n   The hostile C caller (tools/api-crate-generator/src/glue_c_caller.rs). */\n");
        c.push_str(C_PRELUDE);
        let count = plan.interfaces.len();
        let _ = writeln!(c, "#define INTERFACES {count}\n#define MOST {MOST}u\n");
        // Interface ids, bases and the interfaces each one's slots may be called through.
        let _ = writeln!(c, "static const uint8_t IIDS[INTERFACES][16] = {{");
        for key in &plan.interfaces {
            let guid = g.read.interfaces[key].guid.ok_or_else(|| format!("{}: no interface id", key.1))?;
            let mut bytes: Vec<u8> = guid.data1.to_le_bytes().to_vec();
            bytes.extend(guid.data2.to_le_bytes());
            bytes.extend(guid.data3.to_le_bytes());
            bytes.extend(guid.data4);
            let listed: Vec<String> = bytes.iter().map(|b| format!("{b:#04x}")).collect();
            let _ = writeln!(c, "    {{{}}}, /* {} */", listed.join(", "), key.1);
        }
        let _ = writeln!(c, "}};\n\nstatic const int BASE[INTERFACES] = {{");
        for key in &plan.interfaces {
            let base = g.read.interfaces[key].base.as_ref().and_then(|base| plan.index.get(base)).map_or(-1, |&index| i64::try_from(index).unwrap_or(-1));
            let _ = writeln!(c, "    {base}, /* {} */", key.1);
        }
        let _ = writeln!(c, "}};\n");
        // The cases: each interface's own glued slots, at its place in the table.
        let mut cases = String::new();
        let mut case = 0_usize;
        let mut case_interface: Vec<usize> = Vec::new();
        let mut called = 0_usize;
        let saved = self.not_glued.borrow().len();
        for (interface_index, key) in plan.interfaces.iter().enumerate() {
            self.current.set(g.interface_api[key]);
            let slots = self.slots(key);
            for (position, slot) in slots.iter().enumerate() {
                if slot.interface != *key {
                    continue;
                }
                let place = 3 + position;
                let _ = writeln!(cases, "    case {case}: {{ /* {}::{} */", key.1, slot.method.name);
                let _ = writeln!(cases, "        void *this = pick_this(s, right);");
                case_interface.push(interface_index);
                match &slot.glued {
                    Some(glued) => {
                        cases.push_str(&self.c_call(plan, glued, slot.method, &format!("(*(void ***)right)[{place}]"), Some("this"))?);
                        called += 1;
                    }
                    None => {
                        let refusal = self.refusal(slot.method).map_err(|why| format!("{}::{}: {why}", key.1, slot.method.name))?;
                        cases.push_str(&refused_call(&refusal, crate::emit::named(&slot.method.return_type).is_some_and(|k| k.1 == "HRESULT"), &format!("(*(void ***)right)[{place}]"))?);
                        called += 1;
                    }
                }
                let _ = writeln!(cases, "        break;\n    }}");
                case += 1;
            }
        }
        // The exports, each from the address Rust gave, in the order of `exports`.
        let mut exports: Vec<(usize, String)> = Vec::new();
        let mut export_cases = String::new();
        for (api, spec) in g.apis.iter().enumerate().skip(1) {
            if spec.top_trait.is_none() {
                continue;
            }
            self.current.set(api);
            for function in self.export_functions(api) {
                let Ok(glued) = self.glue_of(function, &function.name) else { continue };
                let at = exports.len();
                let _ = writeln!(export_cases, "    case {at}: {{ /* {} */", function.name);
                export_cases.push_str(&self.c_call(plan, &glued, function, &format!("exports[{at}]"), None)?);
                let _ = writeln!(export_cases, "        break;\n    }}");
                exports.push((api, snake(&function.name)));
                called += 1;
            }
        }
        self.not_glued.borrow_mut().truncate(saved);
        let _ = writeln!(c, "#define CASES {case}\n#define EXPORTS {}\n", exports.len());
        // For each interface, the cases an object obtained as it may be called with: its own slots and its bases'.
        for (interface_index, _) in plan.interfaces.iter().enumerate() {
            let mut ancestors = vec![interface_index];
            let mut at = interface_index;
            while let Some(base) = g.read.interfaces[&plan.interfaces[at]].base.as_ref().and_then(|base| plan.index.get(base)) {
                ancestors.push(*base);
                at = *base;
            }
            let listed: Vec<String> = case_interface.iter().enumerate().filter(|(_, owner)| ancestors.contains(owner)).map(|(which, _)| which.to_string()).collect();
            if listed.is_empty() {
                let _ = writeln!(c, "static const uint16_t CASES_OF_{interface_index}[1] = {{0}};\n#define COUNT_OF_{interface_index} 0");
            } else {
                let _ = writeln!(c, "static const uint16_t CASES_OF_{interface_index}[] = {{{}}};\n#define COUNT_OF_{interface_index} {}", listed.join(", "), listed.len());
            }
        }
        let of: Vec<String> = (0..plan.interfaces.len()).map(|at| format!("CASES_OF_{at}")).collect();
        let counts: Vec<String> = (0..plan.interfaces.len()).map(|at| format!("COUNT_OF_{at}")).collect();
        let _ = writeln!(c, "static const uint16_t *const CASES_OF[INTERFACES] = {{{}}};\nstatic const uint32_t CASE_COUNT[INTERFACES] = {{{}}};\n", of.join(", "), counts.join(", "));
        c.push_str(C_RUN);
        let _ = writeln!(c, "static void call_slot(State *s, uint64_t which, void *right) {{\n    switch (which) {{\n{cases}    default: break;\n    }}\n}}\n");
        let _ = writeln!(c, "static void call_export(State *s, const uintptr_t *exports, uint64_t which) {{\n    switch (which) {{\n{export_cases}    default: (void)exports; break;\n    }}\n}}\n");
        c.push_str(C_RUN_ENTRY);
        Ok((c, exports, called))
    }

    /// The exports of `api` the serving glue writes, as write_direct selects them.
    fn export_functions(&self, api: usize) -> Vec<&Method> {
        let g = self.g;
        let spec = g.apis[api];
        g.read
            .functions
            .iter()
            .filter(|((namespace, _), _)| spec.namespaces.contains(&namespace.as_str()))
            .map(|(_, function)| function)
            .filter(|function| spec.library.is_some_and(|library| function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library))))
            .filter(|function| spec.only.is_empty() || spec.only.contains(&function.name.as_str()))
            .collect()
    }
}

/// A call of an entry that refuses its method: arguments of the right types and any values where it reads and writes
/// nothing, an out-buffer of exactly the size it writes where it writes (null now and then), its answer checked and
/// what it wrote checked to be COM's null or zeros.
fn refused_call(refusal: &super::Refusal, status: bool, function: &str) -> Result<String, String> {
    let mut c_types = vec!["void *".to_string()];
    let mut args = vec!["this".to_string()];
    let mut text = String::new();
    let mut checks = String::new();
    for (at, (_, ty, writes)) in refusal.params.iter().enumerate() {
        let c = c_type(ty);
        if c.is_empty() {
            return Err(format!("a refused entry's parameter of C type {ty} the C caller has no form for"));
        }
        c_types.push(c.to_string());
        if let Some(size) = writes.c_size() {
            let _ = writeln!(text, "        uint8_t *w{at} = out_bytes_fixed(s, {size});");
            let _ = writeln!(checks, "        if (w{at}) {{ for (uint64_t i = 0; i < {size}; i++) if (w{at}[i]) {{ s->counted[REFUSED_WRONG]++; break; }} free(w{at}); }}");
            args.push(format!("w{at}"));
            continue;
        }
        args.push(match c {
            "GuidAbi" => "pick_guid(s)".to_string(),
            "float" | "double" => format!("({c})pick_float(s)"),
            _ if c.ends_with('*') => format!("({c})pick_object(s, 1)"),
            _ => format!("({c})pick_value(s)"),
        });
    }
    let returned = refusal.returned.as_str();
    let ret = c_type(returned);
    if ret.is_empty() {
        return Err(format!("a refused entry's return of C type {returned} the C caller has no form for"));
    }
    let _ = writeln!(text, "        typedef {ret} (*F)({});", c_types.join(", "));
    let _ = writeln!(text, "        F f = (F)({function});\n        if (!f) {{ fprintf(stderr, \"C CALLER: a null entry\\n\"); abort(); }}");
    if status {
        let _ = writeln!(text, "        if (f({}) != (int32_t)0x80004001u) s->counted[REFUSED_WRONG]++;", args.join(", "));
    } else if ret == "void" {
        let _ = writeln!(text, "        f({});", args.join(", "));
    } else {
        let _ = writeln!(text, "        (void)f({});", args.join(", "));
    }
    text.push_str(&checks);
    let _ = writeln!(text, "        s->counted[REFUSED]++;");
    Ok(text)
}

/// Writes the C caller crate beside Direct3D 11's direct crate: caller.c, build.rs, lib.rs, the manifest and the test.
pub(in crate::emit) fn write_c_caller(writer: &Writer<'_, '_>, crates: &Path) -> Result<String, String> {
    let g = writer.g;
    let Some(d3d11) = g.apis.iter().position(|api| api.folder == "direct3d-11") else { return Ok(String::new()) };
    let interfaces: Vec<Key> = g.interface_api.keys().filter(|key| g.read.interfaces.contains_key(*key)).cloned().collect();
    let index: BTreeMap<Key, usize> = interfaces.iter().enumerate().map(|(at, key)| (key.clone(), at)).collect();
    let plan = Plan { interfaces, index };
    let (c, exports, called) = writer.c_caller(&plan)?;
    let folder = crates.join(g.apis[d3d11].folder).join("implementation").join("direct").join("c-caller");
    std::fs::create_dir_all(folder.join("tests")).map_err(|error| format!("{}: {error}", folder.display()))?;
    std::fs::write(folder.join("caller.c"), c).map_err(|error| error.to_string())?;
    std::fs::write(folder.join("build.rs"), include_str!("../harness/c-caller/build.rs")).map_err(|error| error.to_string())?;
    std::fs::write(folder.join("tests").join("c_caller.rs"), include_str!("../harness/c-caller/c_caller.rs")).map_err(|error| error.to_string())?;
    // lib.rs: the one entry into C, and the exports of the hostile implementation in C's order.
    let mut lib = String::new();
    let _ = writeln!(lib, "//! {HEADER}\n//!\n//! The hostile C caller (src/glue_c_caller.rs): caller.c calls every entry of every table and every export with raw");
    let _ = writeln!(lib, "//! pointers and counts of its own choosing; `run` hands it the exports and returns what it counted.\n");
    let mut uses: Vec<String> = Vec::new();
    for (api, spec) in g.apis.iter().enumerate() {
        if exports.iter().any(|(a, _)| *a == api) {
            uses.push(format!("use {}_direct as {};", package(spec).replace('-', "_"), direct_alias(spec.folder)));
        }
    }
    uses.push(format!("use {}_hostile::HostileLibrary;", package(g.apis[d3d11]).replace('-', "_")));
    let _ = writeln!(lib, "{}\n", uses.join("\n"));
    let names: Vec<String> = COUNTED.iter().map(|name| format!("\"{name}\"")).collect();
    let _ = writeln!(lib, "/// What one run counts, in the order the C writes them.\npub const COUNTED: [&str; {}] = [{}];\n", COUNTED.len(), names.join(", "));
    let _ = writeln!(lib, "/// How many exports the C takes.\npub const EXPORTS: usize = {};\n", exports.len());
    let _ = writeln!(lib, "/// How many table entries and exports the C calls.\npub const CALLED: usize = {called};\n");
    let _ = writeln!(lib, "#[expect(unsafe_code, reason = \"the hostile C caller's one entry, which build.rs compiles\")]\nunsafe extern \"C\" {{\n    fn hostile_c_run(seed: u64, calls: u64, threads: u32, exports: *const usize, count: usize, report: *mut u64) -> i32;\n}}\n");
    let _ = writeln!(lib, "/// The hostile implementation served as the libraries' exports, each as the address C calls, in C's order.\npub fn hostile_exports() -> Vec<usize> {{");
    let mut served: Vec<usize> = exports.iter().map(|(api, _)| *api).collect();
    served.dedup();
    for api in &served {
        let _ = writeln!(lib, "    let library_{api} = {}::exports(HostileLibrary);", direct_alias(g.apis[*api].folder));
    }
    let fields: Vec<String> = exports.iter().map(|(api, name)| format!("library_{api}.{name} as usize")).collect();
    let _ = writeln!(lib, "    vec![{}]\n}}\n", fields.join(", "));
    let _ = writeln!(lib, "/// Runs `calls` calls on each of `threads` threads of C, from `seed`, over `exports` (C's order); returns what C counted,\n/// or none when C refused the arguments.\npub fn run(seed: u64, calls: u64, threads: u32, exports: &[usize]) -> Option<[u64; {}]> {{", COUNTED.len());
    let _ = writeln!(lib, "    if exports.len() != EXPORTS {{\n        return None;\n    }}\n    let mut report = [0_u64; {}];", COUNTED.len());
    let _ = writeln!(lib, "    #[expect(unsafe_code, reason = \"the hostile C caller's entry, given our own serving glue's exports and room for its counts\")]\n    // SAFETY: `hostile_c_run` reads `count` words at `exports`, each the address of an export of the serving glue in the\n    // order caller.c's EXPORTS names them (`hostile_exports`), and writes COUNTED's length of words at `report`.");
    let _ = writeln!(lib, "    let status = unsafe {{ hostile_c_run(seed, calls, threads, exports.as_ptr(), exports.len(), report.as_mut_ptr()) }};\n    (status == 0).then_some(report)\n}}");
    std::fs::write(folder.join("lib.rs"), lib).map_err(|error| error.to_string())?;
    let mut manifest = format!(
        "[package]\nname = \"{}-direct-c-caller\"\ndescription = \"The generated hostile C caller over every table of the serving glue, for tests only.\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n",
        package(g.apis[d3d11])
    );
    for (api, spec) in g.apis.iter().enumerate() {
        if served.contains(&api) {
            let _ = writeln!(manifest, "{}-direct = {{ path = \"../../../../{}/implementation/direct\" }}", package(spec), spec.folder);
        }
    }
    let _ = writeln!(manifest, "{}-hostile = {{ path = \"../../hostile\" }}", package(g.apis[d3d11]));
    let _ = writeln!(manifest, "\n[dev-dependencies]\n{}-hostile = {{ path = \"../../hostile\" }}\n{} = {{ path = \"../../..\" }}", package(g.apis[d3d11]), package(g.apis[d3d11]));
    for spec in g.apis.iter() {
        if spec.folder != "direct3d-11" {
            let _ = writeln!(manifest, "{} = {{ path = \"../../../../{}\" }}", package(spec), spec.folder);
        }
    }
    manifest.push_str("\n[build-dependencies]\ncc = \"1\"\n\n# Test scaffolding that calls C: it denies unsafe code and marks each use.\n[lints.rust]\nunsafe_code = \"deny\"\nunsafe_op_in_unsafe_fn = \"deny\"\n\n[lints.clippy]\ntype_complexity = \"allow\"\n");
    std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
    Ok(format!("C CALLER: {} interfaces, {called} entries and exports called ({} exports)\n", plan.interfaces.len(), exports.len()))
}

/// The C caller's fixed part: its random numbers, pools of objects, and the helpers each call uses.
const C_PRELUDE: &str = r#"#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { CALLS, ABSENT, SKIPPED, SUCCEEDED, FAILED, ADOPTED, RELEASED, STALE_THIS, WRONG_THIS, NULL_THIS, GARBAGE_THIS, BIG_COUNTS, PAST_LIMIT, NULL_POINTERS, EXPORTS_CALLED, QUERY_INTERFACES, QUERY_LEFT_POINTER, REFUSED, REFUSED_WRONG, COUNTED };

typedef struct GuidAbi { uint32_t data1; uint16_t data2, data3; uint8_t data4[8]; } GuidAbi;

#define POOL 24
#define STALE 64

typedef struct State State;
static int is_a(int have, int want);

/* The caller's own COM object, foreign to the glue: answers no interface, counts its references. */
typedef struct Foreign { void **table; atomic_long references; } Foreign;
static int32_t foreign_query(void *self, const uint8_t *riid, void **out) { (void)self; (void)riid; if (out) *out = NULL; return (int32_t)0x80004002u; }
static uint32_t foreign_add_ref(void *self) { return (uint32_t)atomic_fetch_add(&((Foreign *)self)->references, 1) + 1; }
static uint32_t foreign_release(void *self) { return (uint32_t)atomic_fetch_sub(&((Foreign *)self)->references, 1) - 1; }
static void *FOREIGN_TABLE[3] = {(void *)foreign_query, (void *)foreign_add_ref, (void *)foreign_release};
static Foreign FOREIGN = {FOREIGN_TABLE, 1};
"#;

/// The C caller's run: per thread a state, rounds of calls, every reference given back at the end.
const C_RUN: &str = r#"
struct State {
    uint64_t rng;
    void *held[INTERFACES][POOL];
    uint32_t count[INTERFACES];
    void *stale[STALE];
    uint32_t stale_at;
    uint64_t counted[COUNTED];
};

static uint64_t next(State *s) {
    uint64_t z = (s->rng += 0x9e3779b97f4a7c15ull);
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
    return z ^ (z >> 31);
}

static uint64_t below(State *s, uint64_t bound) { return bound ? next(s) % bound : 0; }

static int is_a(int have, int want) {
    for (int at = have; at >= 0; at = BASE[at]) if (at == want) return 1;
    return 0;
}

static uint64_t pick_value(State *s) {
    static const uint64_t EDGES[] = {0, 1, 2, 3, 4, 7, 8, 16, 0x7f, 0x80, 0xff, 0x100, 0xffff, 0x10000, 0x7fffffff, 0x80000000u, 0xffffffffu, 0x100000000ull, 0x7fffffffffffffffull, 0xffffffffffffffffull};
    switch (below(s, 4)) {
    case 0: return EDGES[below(s, sizeof EDGES / sizeof EDGES[0])];
    case 1: return below(s, 16);
    case 2: return below(s, 1024);
    default: return next(s);
    }
}

static GuidAbi pick_guid(State *s) {
    GuidAbi guid;
    uint64_t words[2] = {next(s), next(s)};
    memcpy(&guid, words, sizeof guid);
    return guid;
}

static double pick_float(State *s) {
    static const double EDGES[] = {0.0, -0.0, 1.0, -1.0, 0.5, 1e30, -1e30, 1e-30};
    if (below(s, 2)) return EDGES[below(s, sizeof EDGES / sizeof EDGES[0])];
    uint32_t bits = (uint32_t)next(s);
    float value;
    memcpy(&value, &bits, sizeof value);
    return value;
}

/* A count of C's choosing: mostly small, sometimes at the glue's limit, past it, or all ones. */
static uint64_t pick_count(State *s) {
    uint64_t roll = below(s, 1024);
    if (roll == 0) { s->counted[BIG_COUNTS]++; return MOST; }
    if (roll <= 8) { s->counted[PAST_LIMIT]++; return MOST + 1 + below(s, 1000); }
    if (roll <= 16) { s->counted[PAST_LIMIT]++; return 0xffffffffu; }
    if (roll < 64) return 0;
    if (roll < 256) return below(s, 300);
    return below(s, 9);
}

static void fill(State *s, uint8_t *bytes, uint64_t length) {
    if (!bytes) return;
    if (length > 4096) {
        memset(bytes, (int)below(s, 256), length);
        return;
    }
    for (uint64_t at = 0; at < length; at += 8) {
        uint64_t word = next(s);
        memcpy(bytes + at, &word, length - at < 8 ? length - at : 8);
    }
}

/* `count` elements of `size` bytes at exactly that size, or one element's memory past the glue's limit (which the glue
   must refuse before reading), or null now and then. */
static uint8_t *elements(State *s, uint64_t count, uint64_t size) {
    if (below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint64_t length = (count > MOST ? 1 : count) * size;
    uint8_t *bytes = malloc(length ? length : 1);
    if (bytes) fill(s, bytes, length);
    return bytes;
}

/* Bytes the callee writes: exactly `count` of them, or null now and then and past a mebibyte. */
static uint8_t *out_bytes(State *s, uint64_t count) {
    if (count > (1u << 20) || below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(count ? count : 1);
    if (bytes) memset(bytes, 0xa5, count);
    return bytes;
}

/* Elements the callee writes: exactly `count` of them, or one element's memory past the glue's limit (which the glue
   must refuse before writing), or null now and then. */
static uint8_t *out_elements(State *s, uint64_t count, uint64_t size) {
    if (below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint64_t length = (count > MOST ? 1 : count) * size;
    uint8_t *bytes = malloc(length ? length : 1);
    if (bytes) memset(bytes, 0xa5, length);
    return bytes;
}

/* A count by pointer: the room `count` (cut to 32 bits), or null now and then. */
static uint32_t *count_cell(State *s, uint64_t count) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint32_t *cell = malloc(sizeof(uint32_t));
    if (cell) *cell = (uint32_t)count;
    return cell;
}

static uint8_t *out_bytes_fixed(State *s, uint64_t size) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(size);
    if (bytes) memset(bytes, 0xa5, size);
    return bytes;
}

static uint8_t *maybe_bytes(State *s, uint64_t size) {
    if (below(s, 16) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(size);
    if (bytes) {
        if (below(s, 4) == 0) memset(bytes, 0, size); else fill(s, bytes, size);
    }
    return bytes;
}

static uint8_t *pick_riid(State *s, int *interface) {
    if (below(s, 32) == 0) { s->counted[NULL_POINTERS]++; return NULL; }
    uint8_t *bytes = malloc(16);
    if (!bytes) return NULL;
    if (below(s, 8) == 0) { fill(s, bytes, 16); return bytes; }
    int at = (int)below(s, INTERFACES);
    memcpy(bytes, IIDS[at], 16);
    *interface = at;
    return bytes;
}

static void *any_held(State *s) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (s->count[at]) return s->held[at][below(s, s->count[at])];
    }
    return NULL;
}

/* An object parameter: one of ours (any kind), null, a released one, an address that is no object, or (`foreign`) the
   caller's own COM object. Without `foreign`, only what a COM call may be made on. */
static void *pick_object(State *s, int any_address) {
    switch (below(s, 16)) {
    case 0: s->counted[NULL_POINTERS]++; return NULL;
    case 1: { void *stale = s->stale[below(s, STALE)]; if (stale) return stale; break; }
    case 2: if (any_address) return (void *)(uintptr_t)((next(s) & 0x00007ffffffffff8ull) | 0x1000); break;
    case 3: return &FOREIGN;
    default: break;
    }
    return any_held(s);
}

/* A held object and the interface it was obtained as. */
static void *held_as(State *s, int *interface) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (s->count[at]) { *interface = (int)at; return s->held[at][below(s, s->count[at])]; }
    }
    return NULL;
}

/* `this`: mostly the object whose table is used, else null, a released object, an object of any kind, or an address
   that is no object (the glue finds its objects by address, never reading one it does not know). */
static void *pick_this(State *s, void *right) {
    switch (below(s, 16)) {
    case 0: s->counted[NULL_THIS]++; return NULL;
    case 1: { void *stale = s->stale[below(s, STALE)]; if (stale) { s->counted[STALE_THIS]++; return stale; } return right; }
    case 2: s->counted[GARBAGE_THIS]++; return (void *)(uintptr_t)((next(s) & 0x00007ffffffffff8ull) | 0x1000);
    case 3: { void *other = any_held(s); s->counted[WRONG_THIS]++; return other ? other : right; }
    default: return right;
    }
}

typedef uint32_t (*Counting)(void *);
typedef int32_t (*Querying)(void *, const uint8_t *, void **);

static void release(State *s, void *object) {
    if (!object) return;
    ((Counting)(*(void ***)object)[2])(object);
    s->counted[RELEASED]++;
}

/* One reference handed out, kept as the interface it was asked as; given back when the pool is full or the
   interface is not one the caller knows. */
static void adopt(State *s, void *object, int interface) {
    if (!object) return;
    if (interface < 0 || s->count[interface] >= POOL) { release(s, object); return; }
    s->held[interface][s->count[interface]++] = object;
    s->counted[ADOPTED]++;
}

/* Gives one held reference back and keeps its address as a released one. */
static void release_one(State *s) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (!s->count[at]) continue;
        uint32_t which = (uint32_t)below(s, s->count[at]);
        void *object = s->held[at][which];
        s->held[at][which] = s->held[at][--s->count[at]];
        release(s, object);
        s->stale[s->stale_at++ % STALE] = object;
        return;
    }
}

static void query_one(State *s) {
    void *object = any_held(s);
    if (!object) return;
    int interface = -1;
    uint8_t *riid = pick_riid(s, &interface);
    void *out = (void *)0x1;
    void **into = below(s, 16) ? &out : NULL;
    int32_t status = ((Querying)(*(void ***)object)[0])(pick_this(s, object), riid, into);
    s->counted[QUERY_INTERFACES]++;
    if (into && status >= 0) adopt(s, out, interface);
    else if (into && out != NULL) s->counted[QUERY_LEFT_POINTER]++;
    free(riid);
}

static void add_ref_one(State *s) {
    uint64_t start = below(s, INTERFACES);
    for (uint64_t step = 0; step < INTERFACES; step++) {
        uint64_t at = (start + step) % INTERFACES;
        if (!s->count[at] || s->count[at] >= POOL) continue;
        void *object = s->held[at][below(s, s->count[at])];
        ((Counting)(*(void ***)object)[1])(object);
        s->held[at][s->count[at]++] = object;
        return;
    }
}
"#;

/// The C caller's entry: threads of rounds.
const C_RUN_ENTRY: &str = r#"
typedef struct Job { uint64_t seed, calls; const uintptr_t *exports; State *state; } Job;

static void *run_thread(void *argument) {
    Job *job = argument;
    State *s = job->state;
    s->rng = job->seed;
    /* The exports first, until some object is held. */
    for (int tries = 0; tries < 4096; tries++) {
        call_export(s, job->exports, below(s, EXPORTS ? EXPORTS : 1));
        s->counted[EXPORTS_CALLED]++;
        if (any_held(s) && tries > 64) break;
    }
    for (uint64_t at = 0; at < job->calls; at++) {
        s->counted[CALLS]++;
        uint64_t roll = below(s, 256);
        if (roll == 0 && EXPORTS) { call_export(s, job->exports, below(s, EXPORTS)); s->counted[EXPORTS_CALLED]++; }
        else if (roll < 6) release_one(s);
        else if (roll < 14) query_one(s);
        else if (roll < 16) add_ref_one(s);
        else {
            int interface = -1;
            void *right = held_as(s, &interface);
            if (!right) s->counted[SKIPPED]++;
            else if (CASE_COUNT[interface]) call_slot(s, CASES_OF[interface][below(s, CASE_COUNT[interface])], right);
            else s->counted[SKIPPED]++;
        }
    }
    for (int at = 0; at < INTERFACES; at++) while (s->count[at]) release(s, s->held[at][--s->count[at]]);
    return NULL;
}

int hostile_c_run(uint64_t seed, uint64_t calls, uint32_t threads, const uintptr_t *exports, size_t count, uint64_t *report) {
    if (count != EXPORTS || threads == 0 || threads > 64) return 1;
    pthread_t ids[64];
    Job jobs[64];
    State *states = calloc(threads, sizeof(State));
    if (!states) return 2;
    for (uint32_t at = 0; at < threads; at++) {
        jobs[at] = (Job){seed * 0x9e3779b97f4a7c15ull + at + 1, calls, exports, &states[at]};
        if (pthread_create(&ids[at], NULL, run_thread, &jobs[at]) != 0) { free(states); return 3; }
    }
    for (uint32_t at = 0; at < threads; at++) pthread_join(ids[at], NULL);
    for (int counted = 0; counted < COUNTED; counted++) {
        report[counted] = 0;
        for (uint32_t at = 0; at < threads; at++) report[counted] += states[at].counted[counted];
    }
    free(states);
    return 0;
}
"#;
