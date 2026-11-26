//! The data file's records, and how they are made from XED's database.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::amd_cpuid::find_amd_cpuid;
use super::privilege::classify_ring3_behavior;
use super::xed::{self, Database, Record};

/// The mode-restriction value of 64-bit mode.
pub const LONG_MODE: u8 = 64;
const HEXADECIMAL_PREFIX: &str = "0x";
const HEXADECIMAL: u32 = 16;
const FIXED_REGISTER_PREFIX: &str = "XED_REG_";
/// The value of ModRM.mod that makes ModRM.rm name a register.
const REGISTER_MOD: u8 = 3;
/// XED's width code for a memory operand as wide as the vector length.
const VECTOR_LENGTH_WIDTH: &str = "vv";

/// One XED instruction form, with every encoding of it that is valid in 64-bit mode.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct InstructionForm {
    /// The Rust variant: the iform in UpperCamelCase.
    pub name: String,
    pub iform: String,
    /// The Intel-syntax mnemonic.
    pub mnemonic: String,
    pub iclass: String,
    pub category: String,
    pub extension: String,
    pub isa_set: String,
    /// The CPUID bits that enable the form: any one alternative suffices, and an empty list means none are needed.
    pub cpuid: Vec<CpuidAlternative>,
    pub ring3_behavior: Ring3Behavior,
    /// Why the behavior at ring 3 is not what XED's CPL field alone says, where it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ring3_behavior_reason: Option<String>,
    pub xed_cpl: u8,
    pub encodings: Vec<Encoding>,
}

/// How the instruction behaves at ring 3 compared with ring 0.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Ring3Behavior {
    Unprivileged,
    Faults,
    DependsOnPrivilegeLevel,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct CpuidAlternative {
    pub group: String,
    pub source: CpuidSource,
    /// The bits that must all hold.
    pub requirements: Vec<CpuidRequirement>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CpuidSource {
    Xed,
    AmdManual,
}

/// A field of a CPUID result register that must hold a value.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct CpuidRequirement {
    pub name: String,
    pub leaf: u32,
    pub subleaf: u32,
    pub register: String,
    pub lowest_bit: u8,
    pub highest_bit: u8,
    pub value: u32,
}

/// One encoding of the form, as XED's decoder pattern describes it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Encoding {
    pub space: EncodingSpace,
    /// XED's opcode map: 0 is one-byte, 1 is 0F, 2 is 0F38, 3 is 0F3A, and legacy map 4 is 3DNow! (0F0F, opcode last).
    pub map: u8,
    pub opcode: u8,
    /// Whether the low three bits of the opcode name a register.
    pub opcode_holds_register: bool,
    /// The prefixes that must be present; for VEX, EVEX and XOP, the prefix their pp field implies.
    pub mandatory_prefixes: Vec<Prefix>,
    /// Whether none of 66, F2 and F3 may be present (or pp must be zero).
    pub no_mandatory_prefix: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rex_w: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_length: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modrm: Option<ModRm>,
    pub rex2: Rex2,
    /// The modes the encoding is valid in, by default address size.
    pub modes: Vec<u8>,
    /// XED's decoder pattern, verbatim.
    pub pattern: String,
    pub operands: Vec<Operand>,
    pub flags: Vec<FlagEffect>,
    pub attributes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum EncodingSpace {
    Legacy,
    Vex,
    Evex,
    Xop,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prefix {
    #[serde(rename = "66")]
    OperandSize,
    #[serde(rename = "F2")]
    RepeatNotEqual,
    #[serde(rename = "F3")]
    RepeatEqual,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ModRm {
    /// What ModRM.mod lets ModRM.rm name.
    pub rm_operand: RmOperand,
    /// The values ModRM.reg must take; empty when it names a register.
    pub reg: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rm: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RmOperand {
    Register,
    Memory,
    Either,
}

/// Whether the APX REX2 prefix may come before the opcode.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Rex2 {
    Allowed,
    Prohibited,
    Required,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Operand {
    /// XED's operand name, such as REG0, MEM0 or IMM0.
    pub name: String,
    pub kind: OperandKind,
    /// The fixed register, or XED's lookup function: the register class and the encoding field naming it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,
    pub visibility: Visibility,
    pub read: Access,
    pub written: Access,
    /// XED's width code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits: Option<u32>,
    /// The width under an effective operand size of 16, 32 and 64 bits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits_by_operand_size: Option<[u32; 3]>,
    /// XED's element type, such as i8, u32 or f64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum OperandKind {
    Register,
    Memory,
    Immediate,
    /// An address computed but not accessed, as by LEA.
    Address,
    RelativeBranch,
    AbsoluteBranch,
    FarPointer,
    Broadcast,
    Segment,
    MemoryBase,
    MemoryIndex,
    MemoryScale,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Explicit,
    Implicit,
    Suppressed,
    /// A condition only XED's encoder uses, which the instruction does not access.
    EncoderOnly,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Always,
    Conditionally,
    Never,
}

/// The flags one condition of the instruction reads and writes.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlagEffect {
    pub condition: FlagCondition,
    /// The value of the count immediate this effect is for, where it depends on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immediate: Option<ImmediateCondition>,
    pub read: Vec<String>,
    pub written: Vec<String>,
    /// The written flags whose value the manuals leave undefined.
    pub undefined: Vec<String>,
    pub cleared: Vec<String>,
    pub set: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FlagCondition {
    Must,
    May,
    ReadOnly,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ImmediateCondition {
    Zero,
    One,
    Other,
}

/// The forms of a database, and the iforms left out because none of their encodings is valid in 64-bit mode.
pub struct Listing {
    pub forms: Vec<InstructionForm>,
    pub legacy_mode_only: Vec<String>,
}

/// Returns the Rust variant name of an iform: each underscore-separated word capitalized, the rest lowercase.
pub fn derive_variant_name(iform: &str) -> String {
    iform
        .split('_')
        .flat_map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase())
                .into_iter()
                .chain(characters.map(|character| character.to_ascii_lowercase()))
        })
        .collect()
}

/// Returns every form of the database that has an encoding valid in 64-bit mode, sorted by iform.
pub fn convert_database(database: &Database) -> Result<Listing, String> {
    let mut records_by_iform: BTreeMap<&str, Vec<&Record>> = BTreeMap::new();
    for record in &database.records {
        records_by_iform
            .entry(&record.iform)
            .or_default()
            .push(record);
    }
    let mut forms = Vec::new();
    let mut legacy_mode_only = Vec::new();
    for (iform, records) in records_by_iform {
        let long_mode: Vec<&Record> = records
            .into_iter()
            .filter(|record| record.mode_restriction.contains(&LONG_MODE))
            .collect();
        let Some(first) = long_mode.first() else {
            legacy_mode_only.push(iform.to_owned());
            continue;
        };
        if let Some(differing) = long_mode.iter().find(|record| {
            (
                &record.iclass,
                &record.disasm_intel,
                &record.category,
                &record.extension,
            ) != (
                &first.iclass,
                &first.disasm_intel,
                &first.category,
                &first.extension,
            ) || (&record.isa_set, record.cpl) != (&first.isa_set, first.cpl)
                || convert_cpuid(record) != convert_cpuid(first)
        }) {
            return Err(format!(
                "the encodings of {iform} differ in class, feature or privilege: {} and {}",
                first.pattern, differing.pattern
            ));
        }
        let (ring3_behavior, ring3_behavior_reason) =
            classify_ring3_behavior(&first.iclass, iform, first.cpl);
        forms.push(InstructionForm {
            name: derive_variant_name(iform),
            iform: iform.to_owned(),
            mnemonic: first.disasm_intel.clone(),
            iclass: first.iclass.clone(),
            category: first.category.clone(),
            extension: first.extension.clone(),
            isa_set: first.isa_set.clone(),
            cpuid: convert_cpuid(first),
            ring3_behavior,
            ring3_behavior_reason: ring3_behavior_reason.map(str::to_owned),
            xed_cpl: first.cpl,
            encodings: long_mode
                .iter()
                .map(|record| convert_encoding(record))
                .collect::<Result<_, _>>()
                .map_err(|error| format!("{iform}: {error}"))?,
        });
    }
    Ok(Listing {
        forms,
        legacy_mode_only,
    })
}

fn convert_cpuid(record: &Record) -> Vec<CpuidAlternative> {
    if record.cpuid_groups.is_empty() {
        return find_amd_cpuid(&record.isa_set, &record.iclass)
            .into_iter()
            .collect();
    }
    record
        .cpuid_groups
        .iter()
        .map(|group| CpuidAlternative {
            group: group.group_name.clone(),
            source: CpuidSource::Xed,
            requirements: group
                .records
                .iter()
                .map(|requirement| CpuidRequirement {
                    name: requirement.fname.clone(),
                    leaf: requirement.leaf,
                    subleaf: requirement.subleaf,
                    register: requirement.reg.clone(),
                    lowest_bit: requirement.bit_start,
                    highest_bit: requirement.bit_end,
                    value: requirement.value,
                })
                .collect(),
        })
        .collect()
}

fn convert_encoding(record: &Record) -> Result<Encoding, String> {
    let space = match record.encoding_space.as_str() {
        "legacy" => EncodingSpace::Legacy,
        "vex" => EncodingSpace::Vex,
        "evex" => EncodingSpace::Evex,
        "xop" => EncodingSpace::Xop,
        other => return Err(format!("unknown encoding space {other}")),
    };
    let opcode = record
        .opcode_base16
        .strip_prefix(HEXADECIMAL_PREFIX)
        .and_then(|digits| u8::from_str_radix(digits, HEXADECIMAL).ok())
        .ok_or_else(|| format!("unreadable opcode {}", record.opcode_base16))?;
    let mandatory_prefixes = [
        (record.osz_66_required, Prefix::OperandSize),
        (record.f2_required, Prefix::RepeatNotEqual),
        (record.f3_required, Prefix::RepeatEqual),
    ]
    .into_iter()
    .filter_map(|(required, prefix)| required.then_some(prefix))
    .collect();
    let modrm = record.has_modrm.then(|| ModRm {
        rm_operand: match record.mod_required.as_slice() {
            [] => RmOperand::Either,
            [REGISTER_MOD] => RmOperand::Register,
            _ => RmOperand::Memory,
        },
        reg: record.reg_required.clone(),
        rm: record.rm_required,
    });
    let rex2 = match record.rex2_restriction.as_str() {
        "" => Rex2::Allowed,
        "PROHIBITED" => Rex2::Prohibited,
        "REQUIRED" => Rex2::Required,
        other => return Err(format!("unknown REX2 restriction {other}")),
    };
    Ok(Encoding {
        space,
        map: record.map,
        opcode,
        opcode_holds_register: record.partial_opcode,
        mandatory_prefixes,
        no_mandatory_prefix: record.no_prefixes_allowed,
        rex_w: record.rexw_prefix.map(u8::from),
        vector_length: record.vl,
        modrm,
        rex2,
        modes: record.mode_restriction.clone(),
        pattern: record.pattern.clone(),
        operands: record
            .parsed_operands
            .iter()
            .map(|operand| convert_operand(operand, record.vl))
            .collect::<Result<_, _>>()?,
        flags: parse_flags(record.flags.as_deref())?,
        attributes: record.attributes.clone(),
    })
}

fn convert_operand(operand: &xed::Operand, vector_length: Option<u16>) -> Result<Operand, String> {
    let base_name = operand
        .name
        .trim_end_matches(|character: char| character.is_ascii_digit());
    let kind = match base_name {
        "REG" => OperandKind::Register,
        "MEM" => OperandKind::Memory,
        "IMM" => OperandKind::Immediate,
        "AGEN" => OperandKind::Address,
        "RELBR" => OperandKind::RelativeBranch,
        "ABSBR" => OperandKind::AbsoluteBranch,
        "PTR" => OperandKind::FarPointer,
        "BCAST" => OperandKind::Broadcast,
        "SEG" => OperandKind::Segment,
        "BASE" => OperandKind::MemoryBase,
        "INDEX" => OperandKind::MemoryIndex,
        "SCALE" => OperandKind::MemoryScale,
        other => return Err(format!("unknown operand {other}")),
    };
    let register = match operand.operand_type.as_str() {
        "nt_lookup_fn" => operand.lookupfn_name.clone(),
        "reg" => operand.bits.as_deref().map(|register| {
            register
                .trim_start_matches(FIXED_REGISTER_PREFIX)
                .to_owned()
        }),
        _ => None,
    };
    let visibility = match operand.visibility.as_str() {
        "DEFAULT" | "EXPLICIT" => Visibility::Explicit,
        "IMPLICIT" => Visibility::Implicit,
        "SUPPRESSED" => Visibility::Suppressed,
        "ECOND" => Visibility::EncoderOnly,
        other => return Err(format!("unknown visibility {other}")),
    };
    let (read, written) = match operand.rw.as_str() {
        "r" => (Access::Always, Access::Never),
        "w" => (Access::Never, Access::Always),
        "rw" => (Access::Always, Access::Always),
        "cr" => (Access::Conditionally, Access::Never),
        "cw" => (Access::Never, Access::Conditionally),
        "rcw" => (Access::Always, Access::Conditionally),
        "crw" => (Access::Conditionally, Access::Conditionally),
        other => return Err(format!("unknown operand action {other}")),
    };
    let fixed_bits = operand.op_widths.as_ref().and_then(|widths| widths.fixed);
    let bits = match (fixed_bits, operand.oc2.as_deref(), vector_length) {
        (Some(0), Some(VECTOR_LENGTH_WIDTH), Some(length)) => Some(u32::from(length)),
        (Some(0), ..) => None,
        (fixed, ..) => fixed,
    };
    let bits_by_operand_size = operand
        .op_widths
        .as_ref()
        .and_then(|widths| widths.scalable.as_ref())
        .map(|scalable| [scalable.eosz16, scalable.eosz32, scalable.eosz64]);
    Ok(Operand {
        name: operand.name.clone(),
        kind,
        register,
        visibility,
        read,
        written,
        width_code: operand.oc2.clone(),
        bits,
        bits_by_operand_size,
        element: operand.xtype.clone(),
    })
}

/// Parses XED's flags field, such as `IMM1 MUST [ of-mod cf-mod ], IMMx MUST [ of-u cf-mod ]`.
fn parse_flags(flags: Option<&str>) -> Result<Vec<FlagEffect>, String> {
    let mut effects = Vec::new();
    let mut immediate = None;
    let mut current: Option<FlagEffect> = None;
    for token in flags.unwrap_or_default().split_whitespace() {
        let token = token.trim_end_matches(',').to_ascii_lowercase();
        let condition = match token.as_str() {
            "imm0" => {
                immediate = Some(ImmediateCondition::Zero);
                continue;
            }
            "imm1" => {
                immediate = Some(ImmediateCondition::One);
                continue;
            }
            "immx" => {
                immediate = Some(ImmediateCondition::Other);
                continue;
            }
            "[" => continue,
            "]" => {
                effects.extend(current.take());
                continue;
            }
            "must" => FlagCondition::Must,
            "may" => FlagCondition::May,
            "readonly" => FlagCondition::ReadOnly,
            flag_action => {
                let effect = current
                    .as_mut()
                    .ok_or_else(|| format!("flag outside a group: {flag_action}"))?;
                let (flag, action) = flag_action
                    .split_once('-')
                    .ok_or_else(|| format!("unreadable flag {flag_action}"))?;
                let flag = flag.to_owned();
                match action {
                    "tst" => effect.read.push(flag),
                    "mod" | "pop" | "ah" => effect.written.push(flag),
                    "u" => {
                        effect.written.push(flag.clone());
                        effect.undefined.push(flag);
                    }
                    "0" => {
                        effect.written.push(flag.clone());
                        effect.cleared.push(flag);
                    }
                    "1" => {
                        effect.written.push(flag.clone());
                        effect.set.push(flag);
                    }
                    other => return Err(format!("unknown flag action {other}")),
                }
                continue;
            }
        };
        current = Some(FlagEffect {
            condition,
            immediate: immediate.take(),
            read: Vec::new(),
            written: Vec::new(),
            undefined: Vec::new(),
            cleared: Vec::new(),
            set: Vec::new(),
        });
    }
    match current {
        None => Ok(effects),
        Some(_) => Err("unclosed flag group".to_owned()),
    }
}

/// Returns the data file: one JSON record per line.
pub fn render_data_file(forms: &[InstructionForm]) -> serde_json::Result<String> {
    forms
        .iter()
        .map(|form| serde_json::to_string(form).map(|line| line + "\n"))
        .collect()
}

pub fn parse_data_file(text: &str) -> serde_json::Result<Vec<InstructionForm>> {
    text.lines().map(serde_json::from_str).collect()
}
