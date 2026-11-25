//! One byte string per form, built from XED's decoder pattern of one of its encodings.
//!
//! The sample names a register of its own in each free register field (2 in ModRM.reg, 1 in ModRM.rm, 3 in vvvv, 5
//! as a vector index, k1 as the opmask), addresses memory as `[rbx]` (with a SIB byte where the pattern needs one),
//! and gives every immediate and displacement zero bytes but a register in an immediate's upper nibble, which is 2. The `samples_decode_to_their_forms` test
//! of `forms_source` decodes each sample with XED.

use super::form::{Encoding, EncodingSpace, InstructionForm, Prefix};

/// The register a free ModRM.reg field names, one every register class has (CR2, SS, BND2 and TMM2 among them).
const FREE_REGISTER: u8 = 2;
/// The register a free ModRM.rm names, other than ModRM.reg's, as forms such as gathers and AMX's require.
const FREE_RM_REGISTER: u8 = 1;
/// The register a free vvvv field names, other than both.
const FREE_VVVV_REGISTER: u8 = 3;
/// The vector index register of a vector SIB, other than the three.
const FREE_INDEX_REGISTER: u8 = 5;
/// The opmask register of a form with an opmask operand, other than k0, which gathers and scatters refuse.
const FREE_MASK_REGISTER: u8 = 1;
/// ModRM.rm of `[rbx]`.
const BASE_RBX: u8 = 3;
/// ModRM.rm that means a SIB byte follows.
const SIB_FOLLOWS: u8 = 4;
/// ModRM.rm that, with mod 0, means a 32-bit displacement from RIP.
const RELATIVE_TO_RIP: u8 = 5;
/// The SIB byte of `[rbx]` with no index.
const SIB_BASE_RBX: u8 = 0x23;
/// The SIB byte of `[rbx + register 5]`, a vector index.
const SIB_VECTOR_INDEX: u8 = FREE_INDEX_REGISTER << 3 | BASE_RBX;
/// An immediate whose upper nibble names register 2.
const REGISTER_IN_IMMEDIATE: u8 = (FREE_REGISTER) << 4;
const OPERAND_SIZE_PREFIX: u8 = 0x66;
const ADDRESS_SIZE_PREFIX: u8 = 0x67;
const LOCK_PREFIX: u8 = 0xF0;
const REPEAT_NOT_EQUAL_PREFIX: u8 = 0xF2;
const REPEAT_EQUAL_PREFIX: u8 = 0xF3;
const REX: u8 = 0x40;
const REX_W: u8 = 0x08;
const REX_B: u8 = 0x01;
const REX2: u8 = 0xD5;
const VEX3: u8 = 0xC4;
const EVEX: u8 = 0x62;
const XOP: u8 = 0x8F;
/// The inverted R, X and B bits of a VEX, EVEX or XOP prefix's first payload byte when they name registers 0 to 7.
const INVERTED_RXB: u8 = 0xE0;
/// The inverted vvvv of a VEX or EVEX prefix that names register 3.
const VVVV_FREE: u8 = !FREE_VVVV_REGISTER & 0xF;
/// The inverted vvvv that names no register.
const VVVV_UNUSED: u8 = 0xF;

/// A ModRM field: free for any value, or fixed by the pattern.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Free,
    Fixed(u8),
}

/// What the pattern fixes, and the bytes in their order.
#[derive(Default)]
struct Sample {
    operand_size: bool,
    address_size: bool,
    lock: bool,
    repeat: Option<u8>,
    rex_w: bool,
    rex_b: bool,
    rex2: bool,
    vector_length: u8,
    prefix_selector: u8,
    vvvv_high: Option<u8>,
    vvvv_low: Option<u8>,
    vector_dest_high: Option<u8>,
    zeroing: u8,
    broadcast_or_rounding: u8,
    mask: Option<u8>,
    condition: Option<u8>,
    no_flags: u8,
    new_destination: u8,
    map: Option<u8>,
    modrm_mod: Option<Field>,
    modrm_reg: Option<Field>,
    modrm_rm: Option<Field>,
    sib: Option<u8>,
    vector_index: bool,
    /// The bytes before ModRM, and those after it once its place is known.
    before_modrm: Vec<u8>,
    after_modrm: Vec<u8>,
    modrm_placed: bool,
    pending_opcode_bits: Option<u8>,
}

/// Returns a sample of the form from the first encoding the encoder can build, with the XED decoder settings it needs.
pub fn encode_sample(form: &InstructionForm) -> Option<(Vec<u8>, Vec<(String, u32)>)> {
    form.encodings.iter().find_map(encode_encoding)
}

/// Returns the bytes of one encoding, and the decoder settings its pattern names; `None` where a token is not known.
fn encode_encoding(encoding: &Encoding) -> Option<(Vec<u8>, Vec<(String, u32)>)> {
    let mut sample = Sample::default();
    let mut settings = Vec::new();
    let has_modrm = encoding.modrm.is_some();
    for token in encoding.pattern.split_whitespace() {
        if !sample.take(token, encoding, &mut settings)? {
            return None;
        }
    }
    if encoding.mandatory_prefixes.contains(&Prefix::OperandSize) && encoding.space == EncodingSpace::Legacy {
        sample.operand_size = true;
    }
    if encoding.space == EncodingSpace::Legacy {
        for prefix in &encoding.mandatory_prefixes {
            match prefix {
                Prefix::RepeatNotEqual => sample.repeat = Some(REPEAT_NOT_EQUAL_PREFIX),
                Prefix::RepeatEqual => sample.repeat = Some(REPEAT_EQUAL_PREFIX),
                Prefix::OperandSize => {}
            }
        }
    }
    sample.rex_w |= encoding.rex_w == Some(1);
    sample.finish(encoding, has_modrm).map(|bytes| (bytes, settings))
}

/// Returns the bits of a token such as `0b1100_1`, and how many there are.
fn parse_bits(text: &str) -> Option<(u8, u32)> {
    let digits: String = text.strip_prefix("0b")?.chars().filter(|digit| *digit != '_').collect();
    Some((u8::from_str_radix(&digits, 2).ok()?, digits.len() as u32))
}

/// Returns the field a token such as `REG[0b001]`, `REG[rrr]` or `REG[1-7]` (its lowest value) gives.
fn parse_field(inside: &str) -> Option<Field> {
    if let Some((value, _)) = parse_bits(inside) {
        return Some(Field::Fixed(value));
    }
    if inside.chars().all(|letter| letter.is_ascii_lowercase()) {
        return Some(Field::Free);
    }
    let (low, _) = inside.split_once('-')?;
    low.parse().ok().map(Field::Fixed)
}

/// The decoder settings that select between forms of the same bytes.
const DECODER_SETTINGS: [&str; 12] = [
    "MPXMODE",
    "TZCNT",
    "LZCNT",
    "WBNOINVD",
    "CLDEMOTE",
    "PREFETCHIT",
    "PREFETCHRST",
    "P4",
    "MODEP5",
    "IBHF",
    "MODE_SHORT_UD0",
    "CET",
];

impl Sample {
    /// Takes one token; `Some(false)` where the encoding cannot be built in 64-bit mode, `None` where it is not known.
    fn take(&mut self, token: &str, encoding: &Encoding, settings: &mut Vec<(String, u32)>) -> Option<bool> {
        if let Some(hexadecimal) = token.strip_prefix("0x") {
            let byte = u8::from_str_radix(hexadecimal, 16).ok()?;
            self.emit(byte);
            return Some(true);
        }
        if let Some((bits, count)) = parse_bits(token) {
            if count == u8::BITS {
                self.emit(bits);
            } else {
                self.pending_opcode_bits = Some(bits << (u8::BITS - count));
            }
            return Some(true);
        }
        if let Some((name, inside)) = token.split_once('[') {
            let field = parse_field(inside.strip_suffix(']')?)?;
            match name {
                "MOD" => self.modrm_mod = Some(field),
                "REG" => self.modrm_reg = Some(field),
                "RM" => self.modrm_rm = Some(field),
                "SRM" => {
                    let high = self.pending_opcode_bits.take()?;
                    let low = match field {
                        Field::Fixed(value) => value,
                        Field::Free => FREE_REGISTER,
                    };
                    self.emit(high | low);
                }
                _ => return None,
            }
            return Some(true);
        }
        if let Some((name, value)) = token.split_once("!=") {
            match (name, value) {
                ("MOD", "3") => {
                    if !matches!(self.modrm_mod, Some(Field::Fixed(0..=2))) {
                        self.modrm_mod = Some(Field::Fixed(0));
                    }
                }
                ("MASK", "0") => self.mask = Some(FREE_MASK_REGISTER),
                ("EASZ", "1") | ("REP", "3") | ("SRM", "0") => {}
                _ => return None,
            }
            return Some(true);
        }
        if let Some((name, value)) = token.split_once('=') {
            let number = match parse_bits(value) {
                Some((bits, _)) => u32::from(bits),
                None => value.parse::<u32>().ok()?,
            };
            let byte = u8::try_from(number).ok()?;
            match name {
                "MOD" => self.modrm_mod = Some(Field::Fixed(byte)),
                "RM" => self.modrm_rm = Some(Field::Fixed(byte)),
                "SRM" => {}
                "OSZ" => self.operand_size = number == 1,
                "EASZ" => match number {
                    2 => self.address_size = true,
                    3 => {}
                    _ => return Some(false),
                },
                "ASZ" => self.address_size = number == 1,
                "LOCK" => self.lock = number == 1,
                "REP" => {
                    self.repeat = match number {
                        2 => Some(REPEAT_NOT_EQUAL_PREFIX),
                        3 => Some(REPEAT_EQUAL_PREFIX),
                        _ => None,
                    }
                }
                "REXW" => self.rex_w = number == 1,
                "REXB" => self.rex_b = number == 1,
                "REXR" | "REXR4" | "REXB4" | "NOREX2" => {}
                "REX2" => self.rex2 = number == 1,
                "VL" => self.vector_length = byte,
                // XED numbers the implied prefixes none, 66, F2 and F3; the pp field gives F3 before F2.
                "VEX_PREFIX" => {
                    self.prefix_selector = match byte {
                        2 => 3,
                        3 => 2,
                        other => other,
                    }
                }
                "MAP" => self.map = Some(byte),
                "VEXDEST3" => self.vvvv_high = Some(byte),
                "VEXDEST210" => self.vvvv_low = Some(byte),
                "VEXDEST4" => self.vector_dest_high = Some(byte),
                "ZEROING" => self.zeroing = byte,
                "BCRC" => self.broadcast_or_rounding = byte,
                "MASK" => self.mask = Some(byte),
                "SCC" => self.condition = Some(byte),
                "NF" => self.no_flags = byte,
                "ND" => self.new_destination = byte,
                "NEED_SIB" => self.sib = Some(SIB_BASE_RBX),
                "UBIT" | "SKIP_OSZ" | "VEXVALID" | "ENC_DELETE" | "MODE" => {}
                _ if DECODER_SETTINGS.contains(&name) => settings.push((name.to_owned(), number)),
                _ => return None,
            }
            return Some(true);
        }
        let call = token.strip_suffix("()")?;
        match call {
            "MODRM" => self.place_modrm(),
            "UISA_VMODRM_XMM" | "UISA_VMODRM_YMM" | "UISA_VMODRM_ZMM" | "VMODRM_XMM" | "VMODRM_YMM" => {
                self.vector_index = true;
                self.modrm_rm = Some(Field::Fixed(SIB_FOLLOWS));
                self.place_modrm();
            }
            "UIMM8" | "SIMM8" | "UIMM8_1" | "BRDISP8" => self.emit(0),
            "SE_IMM8" => self.emit(REGISTER_IN_IMMEDIATE),
            "UIMM16" => self.emit_zeros(2),
            "UIMM32" | "BRDISP32" => self.emit_zeros(4),
            "BRDISP64" => self.emit_zeros(8),
            "UIMMv" => self.emit_zeros(if self.rex_w || encoding.rex_w == Some(1) { 8 } else if self.operand_size { 2 } else { 4 }),
            "SIMMz" | "BRDISPz" => self.emit_zeros(if self.operand_size { 2 } else { 4 }),
            "MEMDISPv" => self.emit_zeros(if self.address_size { 4 } else { 8 }),
            "ONE" | "IMMUNE66" | "IGNORE66" | "REFINING66" | "FORCE64" | "DF64" | "CR_WIDTH" | "OVERRIDE_SEG0"
            | "OVERRIDE_SEG1" | "REMOVE_SEGMENT" | "BRANCH_HINT" | "CET_NO_TRACK" | "IMMUNE_REXW" | "IMMUNE66_LOOP64"
            | "AVX512_ROUND" | "SAE" | "FIX_ROUND_LEN128" | "FIX_ROUND_LEN512" | "EVAPX" | "EVAPX_SCC"
            | "EVEXR4_ONE" => {}
            _ if call.starts_with("ESIZE_") || call.starts_with("NELEM_") => {}
            _ => return None,
        }
        Some(true)
    }

    /// Appends a byte where the bytes so far end.
    fn emit(&mut self, byte: u8) {
        if self.modrm_placed {
            self.after_modrm.push(byte);
        } else if self.modrm_fields_seen() {
            self.place_modrm();
            self.after_modrm.push(byte);
        } else {
            self.before_modrm.push(byte);
        }
    }

    fn emit_zeros(&mut self, count: usize) {
        for _ in 0..count {
            self.emit(0);
        }
    }

    fn modrm_fields_seen(&self) -> bool {
        self.modrm_mod.is_some() || self.modrm_reg.is_some() || self.modrm_rm.is_some()
    }

    /// Marks the place of ModRM: the bytes from here on follow it.
    fn place_modrm(&mut self) {
        self.modrm_placed = true;
    }

    /// Returns ModRM, its SIB byte and displacement.
    fn build_modrm(&self) -> Vec<u8> {
        let fixed_or = |field: Option<Field>, free: u8| match field {
            Some(Field::Fixed(value)) => value,
            Some(Field::Free) | None => free,
        };
        let mode = fixed_or(self.modrm_mod, 3);
        let memory = mode != 3;
        let reg = fixed_or(self.modrm_reg, FREE_REGISTER);
        let rm = fixed_or(self.modrm_rm, if memory { BASE_RBX } else { FREE_RM_REGISTER });
        let mut bytes = vec![mode << 6 | (reg & 7) << 3 | (rm & 7)];
        if memory && rm == SIB_FOLLOWS {
            bytes.push(if self.vector_index { SIB_VECTOR_INDEX } else { self.sib.unwrap_or(SIB_BASE_RBX) });
        }
        let displacement = match mode {
            0 if rm == RELATIVE_TO_RIP => 4,
            1 => 1,
            2 => 4,
            _ => 0,
        };
        bytes.extend(std::iter::repeat_n(0, displacement));
        bytes
    }

    /// Returns the inverted vvvv field: what the pattern fixes, else register 3 where the form names one there.
    fn build_vvvv(&self, encoding: &Encoding) -> u8 {
        match (self.vvvv_high, self.vvvv_low) {
            (Some(high), Some(low)) => high << 3 | low,
            (high, low) => {
                let names_register = encoding.operands.iter().any(|operand| {
                    operand.register.as_deref().is_some_and(|register| register.contains("_N"))
                });
                let free = if names_register { VVVV_FREE } else { VVVV_UNUSED };
                high.unwrap_or(free >> 3) << 3 | low.unwrap_or(free & 7)
            }
        }
    }

    /// Returns the whole byte string, prefixes first.
    fn finish(mut self, encoding: &Encoding, has_modrm: bool) -> Option<Vec<u8>> {
        if let Some(high) = self.pending_opcode_bits.take() {
            self.before_modrm.push(high);
        }
        let mut bytes = Vec::new();
        if self.lock {
            bytes.push(LOCK_PREFIX);
        }
        if self.address_size {
            bytes.push(ADDRESS_SIZE_PREFIX);
        }
        let modrm = (has_modrm || self.modrm_fields_seen()).then(|| self.build_modrm());
        match encoding.space {
            EncodingSpace::Legacy => {
                if self.operand_size {
                    bytes.push(OPERAND_SIZE_PREFIX);
                }
                bytes.extend(self.repeat);
                if self.rex2 {
                    let map_bit = u8::from(self.before_modrm.first() == Some(&0x0F)) << 7;
                    if map_bit != 0 {
                        self.before_modrm.remove(0);
                    }
                    bytes.push(REX2);
                    bytes.push(map_bit | u8::from(self.rex_w) << 3 | u8::from(self.rex_b));
                } else if self.rex_w || self.rex_b {
                    bytes.push(REX | (u8::from(self.rex_w) * REX_W) | (u8::from(self.rex_b) * REX_B));
                }
            }
            EncodingSpace::Vex => {
                let map = self.map.unwrap_or(encoding.map);
                bytes.push(VEX3);
                bytes.push(INVERTED_RXB | map);
                bytes.push(u8::from(self.rex_w) << 7 | self.build_vvvv(encoding) << 3 | (self.vector_length & 1) << 2 | self.prefix_selector);
            }
            EncodingSpace::Xop => {
                let map = self.map.unwrap_or(encoding.map);
                bytes.push(XOP);
                bytes.push(INVERTED_RXB | map);
                bytes.push(u8::from(self.rex_w) << 7 | self.build_vvvv(encoding) << 3 | (self.vector_length & 1) << 2 | self.prefix_selector);
            }
            EncodingSpace::Evex => {
                let map = self.map.unwrap_or(encoding.map);
                // R, X, B and R' inverted for registers below 8 and 16; bit 3 holds APX's B4, zero here.
                bytes.push(EVEX);
                bytes.push(0xF0 | (map & 7));
                bytes.push(u8::from(self.rex_w) << 7 | self.build_vvvv(encoding) << 3 | 1 << 2 | self.prefix_selector);
                let inverted_v_high = 1 - self.vector_dest_high.unwrap_or(0);
                let has_mask = encoding.operands.iter().any(|operand| {
                    operand.register.as_deref().is_some_and(|register| matches!(register, "MASK1" | "MASKNOT0"))
                });
                let mask = self.mask.unwrap_or(if has_mask { FREE_MASK_REGISTER } else { 0 }) & 7 | self.no_flags << 2;
                let low = match self.condition {
                    Some(condition) => condition & 0xF,
                    None => inverted_v_high << 3 | mask,
                };
                bytes.push(
                    self.zeroing << 7
                        | (self.vector_length & 3) << 5
                        | (self.broadcast_or_rounding | self.new_destination) << 4
                        | low,
                );
            }
        }
        bytes.extend(&self.before_modrm);
        if let Some(modrm) = modrm {
            bytes.extend(modrm);
        }
        bytes.extend(&self.after_modrm);
        (!bytes.is_empty()).then_some(bytes)
    }
}
