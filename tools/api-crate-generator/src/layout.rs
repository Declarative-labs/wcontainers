//! The C layout of every record, as the glue will lay it out, written as compile-time assertions against the
//! headers a C compiler has (MinGW's, through zig), so that a wrong size or offset fails to compile.

use std::fmt::Write as _;
use std::path::Path;

use windows_metadata::Type;
use windows_metadata::reader::TypeIndex;

use crate::api::Api;
use crate::metadata::{self, Namespaces, Record, RecordKind};

/// Pointers, handles and interface pointers on the 64-bit Windows targets the glue is built for.
const POINTER: (u64, u64) = (8, 8);
const GUID: (u64, u64) = (16, 4);

/// The headers the assertions are compiled against, each only where the compiler has it.
const HEADERS: &[&str] = &[
    "windows.h",
    "d3dcommon.h",
    "dxgi1_6.h",
    "dxgidebug.h",
    "d3d11_4.h",
    "d3d11sdklayers.h",
    "d3d11shader.h",
    "d3d11shadertracing.h",
];

fn primitive(ty: &Type) -> Option<(u64, u64)> {
    Some(match ty {
        Type::Bool | Type::I8 | Type::U8 => (1, 1),
        Type::Char | Type::I16 | Type::U16 => (2, 2),
        Type::I32 | Type::U32 | Type::F32 => (4, 4),
        Type::I64 | Type::U64 | Type::F64 => (8, 8),
        Type::ISize | Type::USize => POINTER,
        _ => return None,
    })
}

fn round_up(value: u64, align: u64) -> u64 {
    value.div_ceil(align) * align
}

pub(crate) struct Layouts<'a> {
    pub(crate) read: &'a Namespaces,
}

impl Layouts<'_> {
    pub(crate) fn of_type(&self, ty: &Type, outer: &Record) -> Option<(u64, u64)> {
        if let Some(layout) = primitive(ty) {
            return Some(layout);
        }
        match ty {
            Type::PtrConst(..) | Type::PtrMut(..) => Some(POINTER),
            Type::ArrayFixed(inner, length) => {
                let (size, align) = self.of_type(inner, outer)?;
                Some((size * u64::try_from(*length).ok()?, align))
            }
            Type::Name(name) if name.namespace.is_empty() => {
                let nested = outer.nested.iter().find(|nested| nested.name == name.name)?;
                self.of_record(nested)
            }
            Type::Name(name) if name.namespace == "System" && name.name == "Guid" => Some(GUID),
            Type::Name(name) => {
                let key = (name.namespace.clone(), name.name.clone());
                if self.read.interfaces.contains_key(&key) || name.name == "IUnknown" {
                    return Some(POINTER);
                }
                let record = self.read.records.get(&key)?;
                match record.kind {
                    RecordKind::Enum => primitive(record.underlying.as_ref()?),
                    RecordKind::Delegate => Some(POINTER),
                    _ => self.of_record(record),
                }
            }
            _ => None,
        }
    }

    /// Size and alignment, by C's rules: each field at its alignment (no more than the packing), the whole padded
    /// to the largest; a union is as large as its largest member.
    pub(crate) fn of_record(&self, record: &Record) -> Option<(u64, u64)> {
        let pack = record.packing.map(u64::from);
        let mut size = 0;
        let mut align = 1;
        for field in &record.fields {
            let (field_size, field_align) = self.of_type(&field.ty, record)?;
            let field_align = pack.map_or(field_align, |pack| field_align.min(pack));
            align = align.max(field_align);
            if record.kind == RecordKind::Union {
                size = size.max(field_size);
            } else {
                size = round_up(size, field_align) + field_size;
            }
        }
        Some((round_up(size, align), align))
    }

    /// Every field's offset and size, anonymous members included; none when a field's layout is unknown.
    pub(crate) fn field_places(&self, record: &Record) -> Option<Vec<(u64, u64)>> {
        let pack = record.packing.map(u64::from);
        let mut offset = 0;
        let mut places = Vec::new();
        for field in &record.fields {
            let (field_size, field_align) = self.of_type(&field.ty, record)?;
            let field_align = pack.map_or(field_align, |pack| field_align.min(pack));
            if record.kind == RecordKind::Union {
                places.push((0, field_size));
            } else {
                offset = round_up(offset, field_align);
                places.push((offset, field_size));
                offset += field_size;
            }
        }
        Some(places)
    }

    fn offsets(&self, record: &Record) -> Vec<(String, u64)> {
        let pack = record.packing.map(u64::from);
        let mut offset = 0;
        let mut offsets = Vec::new();
        for field in &record.fields {
            let Some((field_size, field_align)) = self.of_type(&field.ty, record) else { break };
            let field_align = pack.map_or(field_align, |pack| field_align.min(pack));
            offset = round_up(offset, field_align);
            // An anonymous member has no name C code can write.
            let anonymous = matches!(&field.ty, Type::Name(name) if name.namespace.is_empty());
            if !anonymous && !field.name.starts_with('_') {
                offsets.push((field.name.clone(), offset));
            }
            offset += field_size;
        }
        offsets
    }
}

pub fn write_check(index: &TypeIndex, apis: &[&Api], out: &Path, left_out: &[String]) -> Result<usize, String> {
    let namespaces: Vec<&str> = apis
        .iter()
        .flat_map(|api| api.namespaces.iter().chain(api.referenced_namespaces))
        .copied()
        .chain(["Windows.Win32.System.Com"])
        .collect();
    let read = metadata::read(index, &namespaces);
    let layouts = Layouts { read: &read };
    let mut text = String::new();
    let _ = writeln!(text, "// {}\n#define COBJMACROS\n#define WIN32_LEAN_AND_MEAN", crate::emit::HEADER);
    for header in HEADERS {
        let _ = writeln!(text, "#if __has_include(<{header}>)\n#include <{header}>\n#endif");
    }
    text.push_str("#include <stddef.h>\n\n");
    let mut count = 0;
    for ((namespace, name), record) in &read.records {
        let owned = apis.iter().any(|api| api.namespaces.contains(&namespace.as_str()));
        if !owned || left_out.iter().any(|left| left == name) {
            continue;
        }
        if !matches!(record.kind, RecordKind::Struct | RecordKind::Union) {
            continue;
        }
        let Some((size, _)) = layouts.of_record(record) else {
            let _ = writeln!(text, "// {name}: a field of a type whose layout the generator does not know");
            continue;
        };
        let _ = writeln!(text, "_Static_assert(sizeof({name}) == {size}, \"SIZE {name} {size}\");");
        count += 1;
        if record.kind == RecordKind::Struct {
            for (field, offset) in layouts.offsets(record) {
                let _ = writeln!(text, "_Static_assert(offsetof({name}, {field}) == {offset}, \"OFFSET {name}.{field} {offset}\");");
                count += 1;
            }
        }
    }
    std::fs::write(out, text).map_err(|error| format!("{}: {error}", out.display()))?;
    Ok(count)
}
