//! Direct3D's 32 registers between the vertex and pixel stages on a back end with fewer
//! inter-stage locations: the values a pair of programs passes are moved onto free locations after
//! conversion, both programs alike.

use wcontainers_gpu::{reflect::StageValue, shader::ShaderModule};

/// `D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT` and `D3D12_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT`, which
/// the input elements' count shares from feature level 10_1 on.
pub const INPUT_SLOTS: u32 = 32;

/// `D3D11_VS_OUTPUT_REGISTER_COUNT` and `D3D12_VS_OUTPUT_REGISTER_COUNT`, as many as the pixel
/// shader's input registers; one of them holds the position.
pub const OUTPUT_REGISTERS: u32 = 32;

/// Refused: a vertex and pixel program pair passing more values than the back end's inter-stage
/// locations hold after moving them (`SlowPath::VaryingsBeyondLimit`).
pub const TOO_MANY_VALUES: &str = "more values between the stages than the back end passes";
/// Refused: stage values moved make a type equal to one the program has, which naga keeps once.
pub const MOVED_ONTO_A_TYPE: &str = "stage values moved onto a type the program has";
/// Refused: a program whose stage values moved fails validation.
pub const MOVED_INVALID: &str = "a program whose stage values moved does not validate";
/// Refused: moves asked of a program passing no values between the vertex and pixel stages.
pub const UNMOVABLE: &str = "stage values moved in a program of no such values";

/// The locations of the values `module` passes between the stages, in order, each once: a vertex
/// program's outputs, a pixel program's inputs, with those the converter adds beside its
/// reflection's (linked clip distances, a depth to clamp, a named layer); none of another stage.
pub fn passed_locations(module: &ShaderModule) -> Vec<u32> {
    use naga::{Binding, TypeInner};
    let Some(ep) = module
        .module
        .entry_points
        .iter()
        .find(|e| e.name == module.entry_point)
    else {
        return Vec::new();
    };
    let values: Vec<_> = match ep.stage {
        naga::ShaderStage::Vertex => ep
            .function
            .result
            .iter()
            .map(|r| (&r.binding, r.ty))
            .collect(),
        naga::ShaderStage::Fragment => ep
            .function
            .arguments
            .iter()
            .map(|a| (&a.binding, a.ty))
            .collect(),
        _ => Vec::new(),
    };
    let mut at = Vec::new();
    for (binding, ty) in values {
        let bindings: Vec<&Binding> = match (binding, &module.module.types[ty].inner) {
            (Some(b), _) => vec![b],
            (None, TypeInner::Struct { members, .. }) => {
                members.iter().filter_map(|m| m.binding.as_ref()).collect()
            }
            (None, _) => Vec::new(),
        };
        at.extend(bindings.into_iter().filter_map(|b| match b {
            Binding::Location { location, .. } => Some(*location),
            Binding::BuiltIn(_) => None,
        }));
    }
    at.sort_unstable();
    at.dedup();
    at
}

/// Where a pixel program's values at locations `read` ([`passed_locations`]) move (from, to) when
/// one lies past the back end's `slots` inter-stage locations: taken in order from location 0.
/// Nothing moves otherwise; none when they are more than `slots`.
pub fn input_moves(read: &[u32], slots: u32) -> Option<Vec<(u32, u32)>> {
    if read.last().is_none_or(|&l| l < slots) {
        return Some(Vec::new());
    }
    if read.len() > slots as usize {
        return None;
    }
    Some(
        read.iter()
            .copied()
            .zip(0..)
            .filter(|(f, t)| f != t)
            .collect(),
    )
}

/// Where the values a vertex program writes at locations `written` move to meet the pixel program
/// reading `read` (moved by [`input_moves`]): each value it reads where it reads it, each other at
/// its own location when that is inside `slots` and free, or else at the first free one; none
/// when they do not fit.
pub fn output_moves(written: &[u32], read: &[u32], slots: u32) -> Option<Vec<(u32, u32)>> {
    let read_moves = input_moves(read, slots)?;
    if read_moves.is_empty() && written.last().is_none_or(|&l| l < slots) {
        return Some(Vec::new());
    }
    let read_at = |l: u32| read_moves.iter().find(|m| m.0 == l).map_or(l, |m| m.1);
    let mut taken: Vec<u32> = read.iter().map(|&l| read_at(l)).collect();
    let mut moves = Vec::new();
    for &l in written {
        let to = if read.contains(&l) {
            read_at(l)
        } else if l < slots && !taken.contains(&l) {
            l
        } else {
            (0..slots).find(|t| !taken.contains(t))?
        };
        taken.push(to);
        if to != l {
            moves.push((l, to));
        }
    }
    Some(moves)
}

/// Moves the values `module` passes between the stages (a vertex program's outputs, a pixel
/// program's inputs) from and to the locations `moves` names, and `values`, its reflection's list
/// of them, alike. The module's content key becomes one of its key and the moves. Fails with the
/// refusal's name.
pub fn relocate(
    module: &mut ShaderModule,
    values: &mut [StageValue],
    moves: &[(u32, u32)],
) -> Result<(), &'static str> {
    use naga::{Binding, TypeInner};
    if moves.is_empty() {
        return Ok(());
    }
    let to = |l: u32| moves.iter().find(|m| m.0 == l).map_or(l, |m| m.1);
    let shift = |b: &mut Binding| {
        if let Binding::Location { location, .. } = b {
            *location = to(*location);
        }
    };
    let ep = module
        .module
        .entry_points
        .iter_mut()
        .find(|e| e.name == module.entry_point)
        .ok_or(UNMOVABLE)?;
    let mut structs = Vec::new();
    let mut each = |binding: &mut Option<Binding>, ty| match binding {
        Some(b) => shift(b),
        None => structs.push(ty),
    };
    match ep.stage {
        naga::ShaderStage::Vertex => {
            if let Some(r) = ep.function.result.as_mut() {
                each(&mut r.binding, r.ty);
            }
        }
        naga::ShaderStage::Fragment => {
            for a in &mut ep.function.arguments {
                each(&mut a.binding, a.ty);
            }
        }
        _ => return Err(UNMOVABLE),
    }
    let types = &mut module.module.types;
    for ty in structs {
        let mut t = types[ty].clone();
        if let TypeInner::Struct { members, .. } = &mut t.inner {
            members
                .iter_mut()
                .filter_map(|m| m.binding.as_mut())
                .for_each(shift);
        }
        if t != types[ty] {
            if types.get(&t).is_some() {
                return Err(MOVED_ONTO_A_TYPE);
            }
            types.replace(ty, t);
        }
    }
    // The module validated with the capabilities it was converted for; locations need none.
    module.info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module.module)
    .map_err(|_| MOVED_INVALID)?;
    let mut key = blake3::Hasher::new();
    key.update(&module.key);
    for (from, to) in moves {
        key.update(&from.to_le_bytes()).update(&to.to_le_bytes());
    }
    module.key = *key.finalize().as_bytes();
    for v in values.iter_mut().filter(|v| v.system.is_none()) {
        v.location = to(v.location);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_past_the_back_ends_locations_move_onto_free_ones() {
        let read: Vec<u32> = (1..32).collect();
        let moves = input_moves(&read, 31).unwrap();
        assert_eq!(moves.len(), 31);
        assert_eq!(moves[0], (1, 0));
        assert_eq!(moves[30], (31, 30));
        // The vertex program writes one value the pixel program does not read: no room is left.
        let written: Vec<u32> = (0..32).collect();
        assert_eq!(output_moves(&written, &read, 31), None);
        assert_eq!(output_moves(&read, &read, 31).unwrap(), moves);
        assert_eq!(input_moves(&written, 31), None);
        assert!(input_moves(&read, 32).unwrap().is_empty());
    }

    #[test]
    fn passed_locations_include_values_beside_the_reflections() {
        let source = "struct O { @builtin(position) p: vec4f, @location(31) a: vec4f, @location(2) b: vec4f }
            @vertex fn main() -> O { return O(vec4f(), vec4f(), vec4f()); }
            @fragment fn pixel(@location(32) c: vec4f, @builtin(position) p: vec4f) -> @location(0) vec4f { return c; }";
        let vertex =
            ShaderModule::from_wgsl(source, "main", wcontainers_gpu::shader::ShaderStage::Vertex)
                .unwrap();
        assert_eq!(passed_locations(&vertex), [2, 31]);
        let pixel = ShaderModule::from_wgsl(
            source,
            "pixel",
            wcontainers_gpu::shader::ShaderStage::Fragment,
        )
        .unwrap();
        assert_eq!(passed_locations(&pixel), [32]);
    }
}
