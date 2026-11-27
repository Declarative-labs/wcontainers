//! A reference that shares no code with Mesa or the bridge: an evaluator of the trait's flat form (`Flat`), written
//! from NIR's documented meaning of each operation (nir_opcodes.py's descriptions, nir_intrinsics.py's sources) and
//! the bridge's comments on the flat form (flat.h). The suite runs a variant's flat form on inputs it chooses, with
//! buffers and textures it makes, and states what the outputs must be; so a step of the bridge that reads an argument
//! wrong gives an output the test can say is wrong, not only different.
//!
//! It takes what the suite's programs need: straight code, ifs and loops, registers, the ALU operations of 32-bit
//! values, stage loads and stores, emitted vertices, buffer loads through derefs, and texture size, level count,
//! texel loads and samples. A sample (of a float texture) reads the texture's one colour: the suite's float textures
//! hold one colour on every texel of every level, so no filtering rule is needed to know the right value.

use std::collections::BTreeMap;

use wcontainers_mesa_compiler::{Flat, FlatInstr, constants};

/// The four 32-bit words of a value (fewer used for a narrower one); a boolean is 0 or 1.
pub type Words = [u32; 4];

/// A texture the evaluator reads: its levels' sizes (width, height, layers), a texel of an integer texture by level
/// and place, and the one colour of a float texture.
#[derive(Clone, Debug)]
pub struct Texture {
    pub sizes: Vec<[u32; 3]>,
    pub integer: bool,
    pub colour: Words,
}

impl Texture {
    /// The texel the suite's integer textures hold: words that name the level and the place, so a test can say which
    /// texel was read (level 1000s, x 10s, y units; the four components 0 to 3 apart).
    pub fn texel(level: u32, x: u32, y: u32) -> Words {
        let base = level * 1000 + x * 10 + y;
        [base, base + 1, base + 2, base + 3]
    }
}

/// What a run gives: every vertex emitted (a geometry program) or the stage's outputs at its end (any other), each
/// output by (built-in, location) with the components written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outputs {
    pub values: BTreeMap<(u32, u32), Words>,
    pub written: BTreeMap<(u32, u32), u32>,
}

impl Outputs {
    /// The output at the built-in (or, for `BUILTIN_NONE`, at the generic location) as floats, None where not written.
    pub fn floats(&self, builtin: u32, location: u32) -> Option<[f32; 4]> {
        self.values.get(&(builtin, location)).map(|words| words.map(f32::from_bits))
    }
}

/// What the evaluator is given: stage inputs by (built-in, location), one value per input vertex, and generic inputs
/// by their component count where the location is the linker's (`generic`); uniform and storage buffers' words by
/// (set, binding); textures by (set, binding).
#[derive(Clone, Debug, Default)]
pub struct Given {
    pub inputs: BTreeMap<(u32, u32), Vec<Words>>,
    pub generic: BTreeMap<u32, Vec<Words>>,
    pub buffers: BTreeMap<(u32, u32), Vec<u32>>,
    pub textures: BTreeMap<(u32, u32), Texture>,
}

/// A value of the run: words, a place in memory (a deref), or a buffer's descriptor.
#[derive(Clone, Debug)]
enum Value {
    Words(Words),
    /// A variable (by index) and a byte offset into it, with the type now pointed at.
    Place { var: u32, buffer: Option<(u32, u32)>, offset: u32, type_: u32 },
    Resource { set: u32, binding: u32 },
}

/// Why a run stopped short.
pub type Refusal = String;

struct Run<'a> {
    flat: &'a Flat,
    given: &'a Given,
    values: BTreeMap<u32, Value>,
    registers: BTreeMap<u32, Words>,
    /// The words of the program's own variables (function and private), by variable and byte offset.
    memory: BTreeMap<(u32, u32), Words>,
    outputs: Outputs,
    emitted: Vec<Outputs>,
}

/// Each structured instruction's partner: an if's else, an else's end, a loop's end and an end's loop; and each
/// instruction's innermost loop.
struct Shape {
    partner: Vec<usize>,
    innermost_loop: Vec<Option<usize>>,
}

fn shape(instrs: &[FlatInstr]) -> Result<Shape, Refusal> {
    let mut partner = vec![usize::MAX; instrs.len()];
    let mut innermost_loop = vec![None; instrs.len()];
    let mut ifs: Vec<usize> = Vec::new();
    let mut loops: Vec<usize> = Vec::new();
    for (at, instr) in instrs.iter().enumerate() {
        innermost_loop[at] = loops.last().copied();
        match instr.kind {
            constants::FLAT_IF => ifs.push(at),
            constants::FLAT_ELSE => {
                let opened = *ifs.last().ok_or("an else outside an if")?;
                partner[opened] = at;
                ifs.pop();
                ifs.push(at);
            }
            constants::FLAT_END_IF => {
                let opened = ifs.pop().ok_or("an end of if outside an if")?;
                partner[opened] = at;
            }
            constants::FLAT_LOOP => loops.push(at),
            constants::FLAT_END_LOOP => {
                let opened = loops.pop().ok_or("an end of loop outside a loop")?;
                partner[opened] = at;
                partner[at] = opened;
            }
            _ => {}
        }
    }
    if ifs.is_empty() && loops.is_empty() { Ok(Shape { partner, innermost_loop }) } else { Err("an if or loop left open".to_string()) }
}

/// The most instructions one run takes (a loop that never ends is refused).
const STEPS: usize = 1_000_000;

/// Runs the flat form on what is given; the outputs at the end and every vertex emitted.
pub fn run(flat: &Flat, given: &Given) -> Result<(Outputs, Vec<Outputs>), Refusal> {
    let shape = shape(&flat.instrs)?;
    let mut run = Run { flat, given, values: BTreeMap::new(), registers: BTreeMap::new(), memory: BTreeMap::new(), outputs: Outputs::default(), emitted: Vec::new() };
    let mut pc = 0;
    let mut steps = 0;
    while pc < flat.instrs.len() {
        steps += 1;
        if steps > STEPS {
            return Err("more steps than a run takes".to_string());
        }
        let instr = &flat.instrs[pc];
        pc = match instr.kind {
            constants::FLAT_IF => {
                let condition = run.words(instr.src[0].def)?[0];
                if condition != 0 { pc + 1 } else { shape.partner[pc] + 1 }
            }
            // The end of the then-list: past the else-list.
            constants::FLAT_ELSE => shape.partner[pc] + 1,
            constants::FLAT_END_IF | constants::FLAT_LOOP => pc + 1,
            constants::FLAT_END_LOOP => shape.partner[pc] + 1,
            constants::FLAT_BREAK => shape.partner[shape.innermost_loop[pc].ok_or("a break outside a loop")?] + 1,
            constants::FLAT_CONTINUE => shape.innermost_loop[pc].ok_or("a continue outside a loop")? + 1,
            constants::FLAT_RETURN => flat.instrs.len(),
            _ => {
                run.step(instr)?;
                pc + 1
            }
        };
    }
    Ok((run.outputs, run.emitted))
}

fn float(word: u32) -> f32 {
    f32::from_bits(word)
}

fn bits(value: f32) -> u32 {
    value.to_bits()
}

fn signed(word: u32) -> i32 {
    i32::from_ne_bytes(word.to_ne_bytes())
}

fn word(value: i32) -> u32 {
    u32::from_ne_bytes(value.to_ne_bytes())
}

fn truth(value: bool) -> u32 {
    u32::from(value)
}

fn name(instr: &FlatInstr) -> String {
    instr.name.as_deref().map(|bytes| String::from_utf8_lossy(bytes).into_owned()).unwrap_or_default()
}

impl Run<'_> {
    fn value(&self, def: u32) -> Result<&Value, Refusal> {
        self.values.get(&def).ok_or_else(|| format!("value {def} read before it is made"))
    }

    fn words(&self, def: u32) -> Result<Words, Refusal> {
        match self.value(def)? {
            Value::Words(words) => Ok(*words),
            other => Err(format!("value {def} is {other:?}, not words")),
        }
    }

    fn set(&mut self, instr: &FlatInstr, words: Words) {
        self.values.insert(instr.def, Value::Words(words));
    }

    /// Source `at` of an ALU instruction, its components picked by the swizzle.
    fn swizzled(&self, instr: &FlatInstr, at: usize) -> Result<Words, Refusal> {
        let source = &instr.src[at];
        let words = self.words(source.def)?;
        Ok(std::array::from_fn(|c| words[usize::from(source.swizzle[c] & 3)]))
    }

    fn step(&mut self, instr: &FlatInstr) -> Result<(), Refusal> {
        match instr.kind {
            constants::FLAT_CONST => {
                self.set(instr, instr.value);
                Ok(())
            }
            constants::FLAT_UNDEF => {
                self.set(instr, [0; 4]);
                Ok(())
            }
            constants::FLAT_ALU => self.alu(instr),
            constants::FLAT_DEREF => self.deref(instr),
            constants::FLAT_INTRINSIC => self.intrinsic(instr),
            constants::FLAT_TEX => self.tex(instr),
            _ => Err(format!("an instruction the reference does not take: {instr:?}")),
        }
    }

    fn alu(&mut self, instr: &FlatInstr) -> Result<(), Refusal> {
        let op = name(instr);
        let n = usize::try_from(instr.components).unwrap_or(4).clamp(1, 4);
        let sources = usize::try_from(instr.sources).unwrap_or(0);
        let mut s: Vec<Words> = Vec::with_capacity(sources);
        for at in 0..sources {
            s.push(self.swizzled(instr, at)?);
        }
        let first = |c: usize| s[0][c];
        let result: Words = match op.as_str() {
            "vec2" | "vec3" | "vec4" => std::array::from_fn(|c| if c < sources { s[c][0] } else { 0 }),
            "fdot2" | "fdot3" | "fdot4" => {
                let k = usize::from(op.as_bytes()[4] - b'0');
                let sum: f32 = (0..k).map(|c| float(s[0][c]) * float(s[1][c])).sum();
                [bits(sum), 0, 0, 0]
            }
            "ball_fequal2" | "ball_fequal3" | "ball_fequal4" | "ball_iequal2" | "ball_iequal3" | "ball_iequal4" => {
                let k = usize::from(op.as_bytes()[op.len() - 1] - b'0');
                let float_kind = op.contains('f');
                [truth((0..k).all(|c| if float_kind { float(s[0][c]) == float(s[1][c]) } else { s[0][c] == s[1][c] })), 0, 0, 0]
            }
            "bany_fnequal2" | "bany_fnequal3" | "bany_fnequal4" | "bany_inequal2" | "bany_inequal3" | "bany_inequal4" => {
                let k = usize::from(op.as_bytes()[op.len() - 1] - b'0');
                let float_kind = op.contains("fn");
                [truth((0..k).any(|c| if float_kind { float(s[0][c]) != float(s[1][c]) } else { s[0][c] != s[1][c] })), 0, 0, 0]
            }
            _ => {
                let mut out = [0; 4];
                for (c, slot) in out.iter_mut().enumerate().take(n) {
                    let a = first(c);
                    let b = s.get(1).map_or(0, |words| words[c]);
                    let d = s.get(2).map_or(0, |words| words[c]);
                    *slot = componentwise(&op, a, b, d).ok_or_else(|| format!("an operation the reference does not take: {op}"))?;
                }
                out
            }
        };
        self.set(instr, result);
        Ok(())
    }

    fn deref(&mut self, instr: &FlatInstr) -> Result<(), Refusal> {
        let made = match instr.op {
            constants::DEREF_VAR => {
                let var = instr.value[0];
                let declared = self.flat.vars.get(usize::try_from(var).unwrap_or(usize::MAX)).ok_or("a deref of no variable")?;
                let buffer = matches!(declared.mode, constants::VAR_UNIFORM_BUFFER | constants::VAR_STORAGE_BUFFER).then_some((declared.set, declared.binding));
                Value::Place { var, buffer, offset: 0, type_: instr.type_ }
            }
            constants::DEREF_CAST => match self.value(instr.src[0].def)?.clone() {
                Value::Place { var, buffer, offset, .. } => Value::Place { var, buffer, offset, type_: instr.type_ },
                Value::Resource { set, binding } => Value::Place { var: u32::MAX, buffer: Some((set, binding)), offset: 0, type_: instr.type_ },
                Value::Words(_) => return Err("a cast of words".to_string()),
            },
            constants::DEREF_STRUCT => {
                let Value::Place { var, buffer, offset, type_ } = self.value(instr.src[0].def)?.clone() else {
                    return Err("a field of no place".to_string());
                };
                let outer = self.flat.types.get(usize::try_from(type_).unwrap_or(usize::MAX)).ok_or("a place of no type")?;
                let field = self.flat.fields.get(usize::try_from(outer.element + instr.value[0]).unwrap_or(usize::MAX)).ok_or("no such field")?;
                Value::Place { var, buffer, offset: offset + u32::try_from(field.offset).unwrap_or(0), type_: instr.type_ }
            }
            constants::DEREF_ARRAY => {
                let Value::Place { var, buffer, offset, type_ } = self.value(instr.src[0].def)?.clone() else {
                    return Err("an element of no place".to_string());
                };
                let index = self.words(instr.src[1].def)?[0];
                let outer = self.flat.types.get(usize::try_from(type_).unwrap_or(usize::MAX)).ok_or("a place of no type")?;
                let stride = if outer.stride != 0 { outer.stride } else { self.size_of(outer.element) };
                Value::Place { var, buffer, offset: offset + index * stride, type_: instr.type_ }
            }
            _ => return Err(format!("a deref the reference does not take: {instr:?}")),
        };
        self.values.insert(instr.def, made);
        Ok(())
    }

    /// The bytes a value of the type takes where it has no explicit stride (32-bit scalars, vectors packed).
    fn size_of(&self, type_: u32) -> u32 {
        self.flat.types.get(usize::try_from(type_).unwrap_or(usize::MAX)).map_or(4, |t| match t.kind {
            constants::TYPE_VECTOR => 4 * t.components,
            constants::TYPE_ARRAY => t.length * if t.stride != 0 { t.stride } else { self.size_of(t.element) },
            _ => 4,
        })
    }

    fn load_place(&self, def: u32, components: u32) -> Result<Words, Refusal> {
        let Value::Place { var, buffer, offset, .. } = self.value(def)? else {
            return Err("a load of no place".to_string());
        };
        let Some(buffer) = buffer else {
            // The program's own variable: what was stored there, zero before.
            return Ok(self.memory.get(&(*var, *offset)).copied().unwrap_or([0; 4]));
        };
        let words = self.given.buffers.get(buffer).ok_or_else(|| format!("a load from buffer {buffer:?}, which is not given"))?;
        let first = usize::try_from(offset / 4).unwrap_or(usize::MAX);
        let mut out = [0; 4];
        for (c, slot) in out.iter_mut().enumerate().take(usize::try_from(components).unwrap_or(0).min(4)) {
            *slot = *words.get(first + c).ok_or_else(|| format!("a load past buffer {buffer:?}'s end"))?;
        }
        Ok(out)
    }

    /// The components of a stage variable's type (of its element, for a per-vertex array).
    fn components_of(&self, var: u32) -> u32 {
        let mut type_ = self.flat.vars.get(usize::try_from(var).unwrap_or(usize::MAX)).map_or(u32::MAX, |declared| declared.type_);
        while let Some(ty) = self.flat.types.get(usize::try_from(type_).unwrap_or(usize::MAX)) {
            match ty.kind {
                constants::TYPE_ARRAY => type_ = ty.element,
                constants::TYPE_VECTOR => return ty.components,
                _ => return 1,
            }
        }
        0
    }

    /// The (built-in, location) of a stage variable.
    fn slot(&self, var: u32) -> Result<(u32, u32), Refusal> {
        let declared = self.flat.vars.get(usize::try_from(var).unwrap_or(usize::MAX)).ok_or("a stage value of no variable")?;
        Ok((declared.builtin, if declared.builtin == constants::BUILTIN_NONE { declared.location } else { 0 }))
    }

    fn intrinsic(&mut self, instr: &FlatInstr) -> Result<(), Refusal> {
        let op = name(instr);
        let components = usize::try_from(instr.components).unwrap_or(0).min(4);
        match op.as_str() {
            "store_output" | "store_per_vertex_output" => {
                let slot = self.slot(instr.value[0])?;
                let value = self.words(instr.src[0].def)?;
                let first = usize::try_from(instr.component).unwrap_or(0);
                let stored = self.outputs.values.entry(slot).or_insert([0; 4]);
                let mut mask = 0;
                for c in 0..4 {
                    if instr.value[2] & (1 << c) != 0 && first + c < 4 {
                        stored[first + c] = value[c];
                        mask |= 1 << (first + c);
                    }
                }
                *self.outputs.written.entry(slot).or_insert(0) |= mask;
            }
            "load_input" | "load_interpolated_input" | "load_per_vertex_input" => {
                let slot = self.slot(instr.value[0])?;
                let vertex = if op == "load_per_vertex_input" { usize::try_from(self.words(instr.src[0].def)?[0]).unwrap_or(usize::MAX) } else { 0 };
                let given = match self.given.inputs.get(&slot) {
                    Some(given) => given,
                    None if slot.0 == constants::BUILTIN_NONE => {
                        let count = self.components_of(instr.value[0]);
                        self.given.generic.get(&count).ok_or_else(|| format!("input {slot:?} of {count} components is not given"))?
                    }
                    None => return Err(format!("input {slot:?} is not given")),
                };
                let value = given.get(vertex).ok_or_else(|| format!("input {slot:?} of vertex {vertex} is not given"))?;
                let first = usize::try_from(instr.component).unwrap_or(0);
                let loaded = std::array::from_fn(|c| if c < components && first + c < 4 { value[first + c] } else { 0 });
                self.set(instr, loaded);
            }
            "decl_reg" => {
                self.registers.insert(instr.def, [0; 4]);
            }
            "load_reg" => {
                let register = *self.registers.get(&instr.src[0].def).ok_or("a load of no register")?;
                self.set(instr, register);
            }
            "store_reg" => {
                let value = self.words(instr.src[0].def)?;
                let register = self.registers.get_mut(&instr.src[1].def).ok_or("a store to no register")?;
                for c in 0..4 {
                    if instr.value[2] & (1 << c) != 0 {
                        register[c] = value[c];
                    }
                }
            }
            "vulkan_resource_index" => {
                self.values.insert(instr.def, Value::Resource { set: instr.value[0], binding: instr.value[1] });
            }
            "load_vulkan_descriptor" => {
                let value = self.value(instr.src[0].def)?.clone();
                self.values.insert(instr.def, value);
            }
            "store_deref" => {
                let Value::Place { var, buffer: None, offset, .. } = self.value(instr.src[0].def)?.clone() else {
                    return Err("a store to a buffer or no place".to_string());
                };
                let value = self.words(instr.src[1].def)?;
                let stored = self.memory.entry((var, offset)).or_insert([0; 4]);
                for c in 0..4 {
                    if instr.value[2] & (1 << c) != 0 {
                        stored[c] = value[c];
                    }
                }
            }
            "load_deref" => {
                let loaded = self.load_place(instr.src[0].def, instr.components)?;
                self.set(instr, loaded);
            }
            "emit_vertex" | "emit_vertex_with_counter" => {
                self.emitted.push(std::mem::take(&mut self.outputs));
            }
            "end_primitive" | "end_primitive_with_counter" => {}
            _ => return Err(format!("an intrinsic the reference does not take: {op}")),
        }
        Ok(())
    }

    /// The texture a texture instruction names, by its variable's (set, binding).
    fn texture(&self, instr: &FlatInstr) -> Result<&Texture, Refusal> {
        let at = usize::try_from(constants::TEX_SRC_TEXTURE).unwrap_or(0);
        let Value::Place { var, .. } = self.value(instr.tex[at])? else {
            return Err("a texture of no place".to_string());
        };
        let declared = self.flat.vars.get(usize::try_from(*var).unwrap_or(usize::MAX)).ok_or("a texture of no variable")?;
        let key = (declared.set, declared.binding);
        self.given.textures.get(&key).ok_or_else(|| format!("texture {key:?} is not given"))
    }

    fn tex_source(&self, instr: &FlatInstr, source: u32) -> Result<Option<Words>, Refusal> {
        let def = instr.tex[usize::try_from(source).unwrap_or(0)];
        if def == u32::MAX { Ok(None) } else { self.words(def).map(Some) }
    }

    fn tex(&mut self, instr: &FlatInstr) -> Result<(), Refusal> {
        let texture = self.texture(instr)?.clone();
        let level_count = u32::try_from(texture.sizes.len()).unwrap_or(0);
        let lod = self.tex_source(instr, constants::TEX_SRC_LOD)?.map_or(0, |words| words[0]);
        let result: Words = match instr.op {
            constants::TEX_SIZE => {
                let size = texture.sizes.get(usize::try_from(lod).unwrap_or(usize::MAX)).ok_or("a size of a level the texture lacks")?;
                [size[0], size[1], size[2], 0]
            }
            constants::TEX_LEVELS => [level_count, 0, 0, 0],
            constants::TEX_LOAD => {
                let coord = self.tex_source(instr, constants::TEX_SRC_COORD)?.ok_or("a load with no coordinate")?;
                let size = texture.sizes.get(usize::try_from(lod).unwrap_or(usize::MAX)).ok_or("a load of a level the texture lacks")?;
                // Out of range is undefined in NIR: the suite's right values never read there, so it is refused.
                if coord[0] >= size[0] || coord[1] >= size[1] {
                    return Err(format!("a texel load outside level {lod}: {coord:?} of {size:?}"));
                }
                if texture.integer { Texture::texel(lod, coord[0], coord[1]) } else { texture.colour }
            }
            constants::TEX_SAMPLE | constants::TEX_BIAS | constants::TEX_LEVEL | constants::TEX_GRADIENT => texture.colour,
            _ => return Err(format!("a texture operation the reference does not take: {instr:?}")),
        };
        self.set(instr, result);
        Ok(())
    }
}

/// One component of a componentwise operation, by NIR's meaning of it (nir_opcodes.py), None for one it does not take.
fn componentwise(op: &str, a: u32, b: u32, c: u32) -> Option<u32> {
    let (fa, fb, fc) = (float(a), float(b), float(c));
    let (ia, ib) = (signed(a), signed(b));
    Some(match op {
        // fcanonicalize: the value itself where denormals are kept (nir_opcodes.py).
        "mov" | "b2b1" | "u2u32" | "fcanonicalize" => a,
        "b2b32" => if a != 0 { u32::MAX } else { 0 },
        "fadd" => bits(fa + fb),
        "fsub" => bits(fa - fb),
        "fmul" => bits(fa * fb),
        "ffma" | "ffma_weak" => bits(fa.mul_add(fb, fc)),
        "flrp" => bits(fa * (1.0 - fc) + fb * fc),
        "fdiv" => bits(fa / fb),
        "fneg" => bits(-fa),
        "fabs" => bits(fa.abs()),
        "fsign" => bits(if fa > 0.0 { 1.0 } else if fa < 0.0 { -1.0 } else { 0.0 }),
        "fmin" => bits(fa.min(fb)),
        "fmax" => bits(fa.max(fb)),
        "ffloor" => bits(fa.floor()),
        "fceil" => bits(fa.ceil()),
        "ffract" => bits(fa - fa.floor()),
        "ftrunc" => bits(fa.trunc()),
        "fround_even" => bits(fa.round_ties_even()),
        "fsqrt" => bits(fa.sqrt()),
        "frsq" => bits(1.0 / fa.sqrt()),
        "frcp" => bits(1.0 / fa),
        "fexp2" => bits(fa.exp2()),
        "flog2" => bits(fa.log2()),
        "fpow" => bits(fa.powf(fb)),
        "flt" => truth(fa < fb),
        "fge" => truth(fa >= fb),
        "feq" => truth(fa == fb),
        "fneu" => truth(fa != fb),
        "iadd" => a.wrapping_add(b),
        "isub" => a.wrapping_sub(b),
        "imul" => a.wrapping_mul(b),
        "ineg" => word(ia.wrapping_neg()),
        "iabs" => word(ia.wrapping_abs()),
        "imin" => word(ia.min(ib)),
        "imax" => word(ia.max(ib)),
        "umin" => a.min(b),
        "umax" => a.max(b),
        "ishl" => a.wrapping_shl(b & 31),
        "ishr" => word(ia.wrapping_shr(b & 31)),
        "ushr" => a.wrapping_shr(b & 31),
        "iand" => a & b,
        "ior" => a | b,
        "ixor" => a ^ b,
        "inot" => !a,
        "ilt" => truth(ia < ib),
        "ige" => truth(ia >= ib),
        "ieq" => truth(a == b),
        "ine" => truth(a != b),
        "ult" => truth(a < b),
        "uge" => truth(a >= b),
        "bcsel" => if a != 0 { b } else { c },
        "i2f32" => bits(ia as f32),
        "u2f32" => bits(a as f32),
        "f2i32" => word(fa as i32),
        "f2u32" => fa as u32,
        "b2f32" => bits(if a != 0 { 1.0 } else { 0.0 }),
        "b2i32" => u32::from(a != 0),
        "fmod" => bits(fa - fb * (fa / fb).floor()),
        "imod" | "irem" | "idiv" | "udiv" | "umod" if b == 0 => 0,
        "idiv" => word(ia.wrapping_div(ib)),
        "udiv" => a / b,
        "irem" => word(ia.wrapping_rem(ib)),
        "umod" => a % b,
        "imod" => word(ia.rem_euclid(ib)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::componentwise;

    /// The operations by NIR's meaning, each on numbers worked by hand.
    #[test]
    fn operations_as_nir_means_them() {
        let f = f32::to_bits;
        assert_eq!(componentwise("fsub", f(2.0), f(0.75), 0), Some(f(1.25)));
        assert_eq!(componentwise("fdiv", f(2.75), f(2.0), 0), Some(f(1.375)));
        assert_eq!(componentwise("fneg", f(0.5), 0, 0), Some(f(-0.5)));
        assert_eq!(componentwise("bcsel", 1, 7, 9), Some(7));
        assert_eq!(componentwise("bcsel", 0, 7, 9), Some(9));
        assert_eq!(componentwise("ffloor", f(-0.5), 0, 0), Some(f(-1.0)));
        assert_eq!(componentwise("f2i32", f(-1.0), 0, 0), Some(u32::MAX));
        assert_eq!(componentwise("ilt", u32::MAX, 0, 0), Some(1));
        assert_eq!(componentwise("ult", u32::MAX, 0, 0), Some(0));
        assert_eq!(componentwise("imod", u32::MAX, 4, 0), Some(3));
        assert_eq!(componentwise("no such operation", 0, 0, 0), None);
    }
}
