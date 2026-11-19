//! The inputs and outputs of a SPIR-V entry point (by location or built-in, with their types), read
//! from the module's words, and the module patched so that transform feedback captures its outputs.

use spirv::{BuiltIn, Decoration, Op, StorageClass};
use std::collections::HashMap;
use wcontainers_nir::spirv::{HEADER_WORDS, WORD_COUNT_SHIFT, instructions};

/// Transform feedback captures 32-bit components, each at a multiple of their size.
const COMPONENT_BYTES: u32 = u32::BITS / u8::BITS;
const COMPONENT_BITS: u32 = u32::BITS;

/// The kind of a component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Float,
    Sint,
    Uint,
}

/// A stage input or output: `count` values (array elements or matrix columns) of `components`
/// 32-bit components each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ty {
    pub kind: Kind,
    pub components: u32,
    pub count: u32,
}

impl Ty {
    pub const fn bytes(self) -> u32 {
        self.components * self.count * COMPONENT_BYTES
    }
}

/// Where a stage input or output is matched with its neighbour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    Location(u32),
    BuiltIn(u32),
}

/// One input or output: its variable, the member of its block where it is one, and its type.
#[derive(Clone, Debug)]
pub struct Var {
    pub id: u32,
    pub member: Option<(u32, u32)>,
    pub slot: Slot,
    pub ty: Ty,
}

/// An entry point's inputs and outputs, or why one cannot be read here.
pub struct Interface {
    pub inputs: Vec<Var>,
    pub outputs: Vec<Var>,
}

#[derive(Clone, Debug)]
enum Type {
    Bool,
    Int { bits: u32, signed: bool },
    Float { bits: u32 },
    Vector { component: u32, count: u32 },
    Matrix { column: u32, count: u32 },
    Array { element: u32, length: u32 },
    Struct { members: Vec<u32> },
    Pointer { class: u32, pointee: u32 },
    Other,
}

/// What the module declares, by id.
#[derive(Default)]
struct Declarations {
    types: HashMap<u32, Type>,
    constants: HashMap<u32, u32>,
    variables: HashMap<u32, u32>,
    decorations: HashMap<u32, Vec<(u32, u32)>>,
    member_decorations: HashMap<(u32, u32), Vec<(u32, u32)>>,
    interface: Vec<u32>,
    entry: Option<u32>,
}

/// The words of a SPIR-V literal string, so the operands after it can be found.
fn string_words(words: &[u32]) -> usize {
    words
        .iter()
        .position(|w| w.to_le_bytes().contains(&0))
        .map_or(words.len(), |at| at + 1)
}

/// What the module declares; each instruction's operands are named by its shape (the opcode word
/// first).
fn declarations(words: &[u32]) -> Declarations {
    let mut d = Declarations::default();
    for (opcode, inst) in instructions(words) {
        let Some(op) = Op::from_u32(opcode) else {
            continue;
        };
        let ty = match (op, inst) {
            (Op::TypeBool, &[_, result]) => Some((result, Type::Bool)),
            (Op::TypeInt, &[_, result, bits, signed]) => Some((
                result,
                Type::Int {
                    bits,
                    signed: signed != 0,
                },
            )),
            (Op::TypeFloat, &[_, result, bits, ..]) => Some((result, Type::Float { bits })),
            (Op::TypeVector, &[_, result, component, count]) => {
                Some((result, Type::Vector { component, count }))
            }
            (Op::TypeMatrix, &[_, result, column, count]) => {
                Some((result, Type::Matrix { column, count }))
            }
            (Op::TypeArray, &[_, result, element, length]) => {
                Some((result, Type::Array { element, length }))
            }
            (Op::TypeStruct, [_, result, members @ ..]) => Some((
                *result,
                Type::Struct {
                    members: members.to_vec(),
                },
            )),
            (Op::TypePointer, &[_, result, class, pointee]) => {
                Some((result, Type::Pointer { class, pointee }))
            }
            (_, &[_, result, ..]) if declares_type(opcode) => Some((result, Type::Other)),
            _ => None,
        };
        if let Some((result, ty)) = ty {
            d.types.insert(result, ty);
            continue;
        }
        match (op, inst) {
            (Op::Constant, &[_, _, result, value, ..]) => {
                d.constants.insert(result, value);
            }
            (Op::Variable, &[_, pointer, result, ..]) => {
                d.variables.insert(result, pointer);
            }
            (Op::Decorate, &[_, target, decoration, ref values @ ..]) => d
                .decorations
                .entry(target)
                .or_default()
                .push((decoration, values.first().copied().unwrap_or_default())),
            (Op::MemberDecorate, &[_, target, member, decoration, ref values @ ..]) => d
                .member_decorations
                .entry((target, member))
                .or_default()
                .push((decoration, values.first().copied().unwrap_or_default())),
            (Op::EntryPoint, [_, _, function, name_and_interface @ ..]) if d.entry.is_none() => {
                d.entry = Some(*function);
                d.interface = name_and_interface[string_words(name_and_interface)..].to_vec();
            }
            _ => {}
        }
    }
    d
}

/// Whether the opcode declares a type (the section where module-level declarations start).
fn declares_type(opcode: u32) -> bool {
    (Op::TypeVoid as u32..=Op::TypeForwardPointer as u32).contains(&opcode)
}

impl Declarations {
    /// The type as a stage input or output takes it, or why it cannot be read here.
    fn ty(&self, id: u32) -> Result<Ty, String> {
        let scalar = |id: u32| -> Result<Kind, String> {
            match self.types.get(&id) {
                Some(Type::Float { bits }) if *bits == COMPONENT_BITS => Ok(Kind::Float),
                Some(Type::Int { bits, signed }) if *bits == COMPONENT_BITS => {
                    Ok(if *signed { Kind::Sint } else { Kind::Uint })
                }
                Some(Type::Bool) => Ok(Kind::Uint),
                other => Err(format!("a stage value of type {other:?}")),
            }
        };
        let vector = |id: u32| -> Result<(Kind, u32), String> {
            match self.types.get(&id) {
                Some(Type::Vector { component, count }) => Ok((scalar(*component)?, *count)),
                _ => Ok((scalar(id)?, 1)),
            }
        };
        match self.types.get(&id) {
            Some(Type::Array { element, length }) => {
                let (kind, components) = vector(*element)?;
                let count = *self
                    .constants
                    .get(length)
                    .ok_or_else(|| "an array whose length is not a constant".to_string())?;
                Ok(Ty {
                    kind,
                    components,
                    count,
                })
            }
            Some(Type::Matrix { column, count }) => {
                let (kind, components) = vector(*column)?;
                Ok(Ty {
                    kind,
                    components,
                    count: *count,
                })
            }
            _ => {
                let (kind, components) = vector(id)?;
                Ok(Ty {
                    kind,
                    components,
                    count: 1,
                })
            }
        }
    }

    fn slot(decorations: &[(u32, u32)]) -> Option<Slot> {
        decorations.iter().find_map(|&(d, v)| {
            if d == Decoration::Location as u32 {
                Some(Slot::Location(v))
            } else if d == Decoration::BuiltIn as u32 {
                Some(Slot::BuiltIn(v))
            } else {
                None
            }
        })
    }
}

/// The entry point's inputs and outputs (the first entry point's), or why one cannot be read here.
pub fn interface(words: &[u32]) -> Result<Interface, String> {
    let d = declarations(words);
    let mut out = Interface {
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let none = Vec::new();
    for &id in &d.interface {
        let Some(Type::Pointer { class, pointee }) =
            d.variables.get(&id).and_then(|ty| d.types.get(ty))
        else {
            continue;
        };
        let (class, pointee) = (*class, *pointee);
        let list = if class == StorageClass::Input as u32 {
            &mut out.inputs
        } else if class == StorageClass::Output as u32 {
            &mut out.outputs
        } else {
            continue;
        };
        let decorations = d.decorations.get(&id).unwrap_or(&none);
        if let Some(slot) = Declarations::slot(decorations) {
            let ty = match slot {
                Slot::Location(_) => d.ty(pointee)?,
                Slot::BuiltIn(_) => d.ty(pointee).unwrap_or(Ty {
                    kind: Kind::Uint,
                    components: 0,
                    count: 0,
                }),
            };
            list.push(Var {
                id,
                member: None,
                slot,
                ty,
            });
        } else if let Some(Type::Struct { members }) = d.types.get(&pointee) {
            for (member, &ty) in (0..).zip(members) {
                let decorations = d
                    .member_decorations
                    .get(&(pointee, member))
                    .unwrap_or(&none);
                if let Some(slot) = Declarations::slot(decorations) {
                    list.push(Var {
                        id,
                        member: Some((pointee, member)),
                        slot,
                        ty: d.ty(ty).unwrap_or(Ty {
                            kind: Kind::Uint,
                            components: 0,
                            count: 0,
                        }),
                    });
                }
            }
        } else {
            return Err("a stage value without a location or built-in".into());
        }
    }
    Ok(out)
}

/// The outputs transform feedback captures: every output at a location, and the position.
pub fn captured(outputs: &[Var]) -> Vec<&Var> {
    outputs
        .iter()
        .filter(|v| match v.slot {
            Slot::Location(_) => true,
            Slot::BuiltIn(b) => b == BuiltIn::Position as u32,
        })
        .collect()
}

fn instruction(op: Op, operands: &[u32]) -> Vec<u32> {
    let count = u32::try_from(operands.len() + 1).expect("a short instruction");
    let mut inst = vec![(count << WORD_COUNT_SHIFT) | op as u32];
    inst.extend_from_slice(operands);
    inst
}

/// The module with transform feedback capturing `vars` into buffer 0, one after another in their
/// order, and the stride of one vertex's record.
pub fn with_capture(words: &[u32], vars: &[&Var]) -> (Vec<u32>, u32) {
    let d = declarations(words);
    let stride: u32 = vars.iter().map(|v| v.ty.bytes()).sum();
    let mut decorations = Vec::new();
    let mut offset = 0;
    let mut buffers: Vec<u32> = vars.iter().map(|v| v.id).collect();
    buffers.dedup();
    for v in vars {
        match v.member {
            Some((block, member)) => decorations.extend(instruction(
                Op::MemberDecorate,
                &[block, member, Decoration::Offset as u32, offset],
            )),
            None => decorations.extend(instruction(
                Op::Decorate,
                &[v.id, Decoration::Offset as u32, offset],
            )),
        }
        offset += v.ty.bytes();
    }
    for id in buffers {
        decorations.extend(instruction(
            Op::Decorate,
            &[id, Decoration::XfbBuffer as u32, 0],
        ));
        decorations.extend(instruction(
            Op::Decorate,
            &[id, Decoration::XfbStride as u32, stride],
        ));
    }
    let mut out = words[..HEADER_WORDS.min(words.len())].to_vec();
    out.extend(instruction(
        Op::Capability,
        &[spirv::Capability::TransformFeedback as u32],
    ));
    let mut placed = false;
    let mut after_entry = false;
    for (opcode, inst) in instructions(words) {
        if after_entry && opcode != Op::EntryPoint as u32 {
            out.extend(instruction(
                Op::ExecutionMode,
                &[
                    d.entry.unwrap_or_default(),
                    spirv::ExecutionMode::Xfb as u32,
                ],
            ));
            after_entry = false;
        }
        if !placed && declares_type(opcode) {
            out.extend_from_slice(&decorations);
            placed = true;
        }
        out.extend_from_slice(inst);
        after_entry |= opcode == Op::EntryPoint as u32;
    }
    (out, stride)
}
