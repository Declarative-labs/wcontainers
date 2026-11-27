//! Turns the A64 instruction list in `list/a64-instructions.tsv` into the `Instruction` type and its records.
//!
//! The list has one row per encoding of Arm's machine-readable release (BSD-3-Clause, `list/LICENSE-AARCHMRS.txt`),
//! with the operand meanings of LLVM's AArch64 records (Apache-2.0 WITH LLVM-exception, `list/LICENSE-LLVM.txt`);
//! `list/from_arm_release.py` makes it and writes every difference between the two to `list/arm-llvm-differences.tsv`.

use std::fmt::Write;
use std::path::PathBuf;

const LIST: &str = "list/a64-instructions.tsv";
const GENERATED: &str = "instruction_list.rs";
const COMMENT: char = '#';
const COLUMN_SEPARATOR: char = '\t';
const FIELD_SEPARATOR: &str = "; ";
const RUN_SEPARATOR: char = ',';
/// What separates the reserved patterns of a row, and the mask from the value of one.
const PATTERN_SEPARATOR: char = ' ';
const MASK_VALUE_SEPARATOR: char = '/';
/// What a row holds where it has no reserved pattern or no example word.
const NONE: &str = "-";
const INSTRUCTION_KIND: &str = "instruction";
const SYNTAX_OPERAND: char = '$';
const GROUP_JOINER: &str = "_and_";
/// What ends the LLVM name of a 64-bit register class or of an operand of 64-bit registers.
const SIXTY_FOUR_BITS: &str = "64";
/// What ends the LLVM name of a register class whose number 31 is the stack pointer.
const STACK_POINTER_SUFFIX: &str = "sp";
/// What starts the LLVM name of a shifted register operand of a logical instruction, which may rotate.
const LOGICAL_PREFIX: &str = "logical";
/// What this script calls a field that selects one of an instruction's spellings, such as BTI's target.
const SELECTOR: &str = "selector";
/// What the list calls a field LLVM leaves undecoded.
const IGNORED_FIELD: &str = "ignored";
/// The field of the base register of a load or store.
const BASE_REGISTER: &str = "Rn";
/// What the syntax of a load or store puts before its base register.
const ADDRESS_BASE: &str = "[$Rn";
/// What follows the address of a load or store that writes its address back before the access.
const PRE_INDEX: &str = "]!";
/// What follows the address of a load or store that writes its address back after the access.
const POST_INDEX: &str = "], ";
/// The names of the fields of a register offset: the register, option<2> with S, and option<0>.
const REGISTER_OFFSET_FIELDS: [&str; 3] = ["Rm", "extend", "arrangement"];
/// What starts the name of an LLVM record of a register offset load or store, after its access.
const REGISTER_OFFSET_RECORD: &str = "ro";
/// ADRP's offset counts 4 KiB pages, as a left shift of its bytes.
const PAGE_SCALE: u32 = 12;
/// A branch's offset counts instructions of 4 bytes, as a left shift of its bytes.
const INSTRUCTION_SCALE: u32 = 2;
/// The variant of `Instruction` that every level has but the list does not hold.
const UNLISTED_VARIANT: &str = "MrsCtrEl0";

/// List names this tree spells otherwise, because the list kept an LLVM record name or a bare digit run.
const RENAMES: [(&str, &str); 24] = [
    ("ExtrWrri32", "Extr32"),
    ("ExtrXrri64", "Extr64"),
    ("Rev32", "RevRegister32"),
    ("Rev16_32", "Rev16Register32"),
    ("Rev16_64", "Rev16Register64"),
    ("Rev32_64", "Rev32Register64"),
    ("LdrWroW32", "LdrRegister32"),
    ("LdrXroW64", "LdrRegister64"),
    ("LdrbBroWExtendedRegister32", "LdrbExtendedRegister32"),
    ("LdrbBroXShiftedRegister32", "LdrbShiftedRegister32"),
    ("LdrhHroW32", "LdrhRegister32"),
    ("LdrsbXroWExtendedRegister64", "LdrsbExtendedRegister64"),
    ("LdrsbXroXShiftedRegister64", "LdrsbShiftedRegister64"),
    ("LdrsbWroWExtendedRegister32", "LdrsbExtendedRegister32"),
    ("LdrsbWroXShiftedRegister32", "LdrsbShiftedRegister32"),
    ("LdrshXroW64", "LdrshRegister64"),
    ("LdrshWroW32", "LdrshRegister32"),
    ("LdrswRoW64", "LdrswRegister64"),
    ("StrWroW32", "StrRegister32"),
    ("StrXroW64", "StrRegister64"),
    ("StrbBroWExtendedRegister32", "StrbExtendedRegister32"),
    ("StrbBroXShiftedRegister32", "StrbShiftedRegister32"),
    ("StrhHroW32", "StrhRegister32"),
    ("PrfmRoW64", "PrfmRegister"),
];

/// LLVM names a few register operands after the record's own operand names.
const SYNTAX_SYNONYMS: [(&str, &str); 3] = [("dst", "Rd"), ("src1", "Rn"), ("src2", "Rm")];

/// The LLVM records whose `imm` field is the 12-bit immediate and its shift of an add or subtract.
const ARITHMETIC_IMMEDIATE_RECORDS: [&str; 8] = [
    "ADDWri", "ADDXri", "ADDSWri", "ADDSXri", "SUBWri", "SUBXri", "SUBSWri", "SUBSXri",
];

/// The columns of the list, in its order.
#[derive(Clone, Copy)]
enum Column {
    Kind,
    Name,
    Mnemonic,
    Class,
    Feature,
    Privilege,
    Mask,
    Value,
    ShouldBeMask,
    ShouldBeValue,
    Reserved,
    Example,
    Fields,
}

struct Run {
    high: u32,
    low: u32,
}

impl Run {
    fn render(&self) -> String {
        format!("Field::new({}, {})", self.low, self.high - self.low + 1)
    }
}

#[derive(PartialEq)]
enum Access {
    Read,
    Written,
}

struct ListField {
    name: String,
    runs: Vec<Run>,
    access: Option<Access>,
    /// The LLVM operand type, or for an encoding field the LLVM record it belongs to.
    llvm_type: String,
    /// The part of a grouped LLVM operand this field is, such as the `shift` of a shifted register.
    part: Option<String>,
}

fn main() {
    println!("cargo::rerun-if-changed={LIST}");
    println!("cargo::rerun-if-changed=build.rs");
    let text = std::fs::read_to_string(LIST).expect("the list is readable");
    let mut variants = Vec::new();
    let mut records = String::new();
    for line in text.lines().filter(|line| !line.starts_with(COMMENT)) {
        let columns: Vec<&str> = line.split(COLUMN_SEPARATOR).collect();
        if columns[Column::Kind as usize] != INSTRUCTION_KIND {
            continue;
        }
        let listed = columns[Column::Name as usize];
        let name = RENAMES
            .iter()
            .find(|(from, _)| *from == listed)
            .map_or(listed, |(_, to)| to);
        let syntax = columns.last().expect("a row ends in its syntax");
        let fields = read_fields(columns[Column::Fields as usize]);
        let operands = render_operands(&fields, syntax);
        let reserved: Vec<String> = columns[Column::Reserved as usize]
            .split(PATTERN_SEPARATOR)
            .filter(|pattern| *pattern != NONE)
            .map(|pattern| {
                let (mask, value) = pattern
                    .split_once(MASK_VALUE_SEPARATOR)
                    .expect("a pattern is mask/value");
                format!("FixedBits {{ mask: {mask}, value: {value} }}")
            })
            .collect();
        let example = match columns[Column::Example as usize] {
            NONE => "None".to_owned(),
            word => format!("Some({word})"),
        };
        writeln!(
            records,
            "    InstructionRecord {{ mnemonic: {:?}, fixed_bits: FixedBits {{ mask: {}, value: {} }}, \
             should_be: FixedBits {{ mask: {}, value: {} }}, reserved: &[{}], example: {example}, operands: &[{}], \
             class: InstructionClass::{}, feature: {:?}, privilege: PrivilegeBehavior::{} }},",
            columns[Column::Mnemonic as usize],
            columns[Column::Mask as usize],
            columns[Column::Value as usize],
            columns[Column::ShouldBeMask as usize],
            columns[Column::ShouldBeValue as usize],
            reserved.join(", "),
            operands.join(", "),
            columns[Column::Class as usize],
            columns[Column::Feature as usize],
            columns[Column::Privilege as usize],
        )
        .expect("writing to a string succeeds");
        variants.push(name.to_owned());
    }
    let mut out = String::new();
    writeln!(
        out,
        "/// An A64 [instruction variant](https://developer.arm.com/documentation/ddi0487/mc/-Part-C-The-AArch64-Instruction-Set/-Chapter-C1-The-A64-Instruction-Set/-C1-1-About-the-A64-instruction-set#:~:text=each%20syntax%20is%20an%20instruction%20variant), \
         as `list/a64-instructions.tsv` lists it.\n\
         #[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub enum Instruction<LevelSpecific> {{"
    )
    .expect("writing to a string succeeds");
    for name in &variants {
        writeln!(out, "    {name},").expect("writing to a string succeeds");
    }
    out.push_str(
        "    /// MRS of CTR_EL0.\n    ///\n    /// TODO: temporary placeholder until `Mrs` is modeled with every system register it reads.\n    \
         MrsCtrEl0,\n    LevelSpecific(LevelSpecific),\n}\n\n",
    );
    let count = variants.len();
    writeln!(
        out,
        "impl<LevelSpecific> Instruction<LevelSpecific> {{\n    \
         /// Every variant of the list, in its order.\n    pub const LISTED: [Self; {count}] = ["
    )
    .expect("writing to a string succeeds");
    for name in &variants {
        writeln!(out, "        Self::{name},").expect("writing to a string succeeds");
    }
    writeln!(
        out,
        "    ];\n}}\n\n/// Every variant of the list, in its order.\n\
         pub const LISTED_INSTRUCTIONS: [Instruction<Infallible>; {count}] = Instruction::LISTED;\n\n\
         static RECORDS: [InstructionRecord; {count}] = [\n{records}];\n"
    )
    .expect("writing to a string succeeds");
    out.push_str(
        "impl<LevelSpecific> Instruction<LevelSpecific> {\n    \
         /// The variant's place in the list, none for those the list does not hold.\n    \
         const fn to_list_index(&self) -> Option<usize> {\n        Some(match self {\n",
    );
    for (index, name) in variants.iter().enumerate() {
        writeln!(out, "            Self::{name} => {index},")
            .expect("writing to a string succeeds");
    }
    out.push_str("            Self::MrsCtrEl0 | Self::LevelSpecific(_) => return None,\n        })\n    }\n}\n");
    // Written for the levels that add no instruction, since `Infallible` has no `Bake` for a derive to ask of it.
    out.push_str(
        "\n#[cfg(feature = \"bake\")]\nimpl databake::Bake for Instruction<Infallible> {\n    \
         fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {\n        \
         env.insert(env!(\"CARGO_CRATE_NAME\"));\n        match *self {\n",
    );
    for name in variants.iter().map(String::as_str).chain([UNLISTED_VARIANT]) {
        writeln!(
            out,
            "            Self::{name} => {{\n                \
             databake::quote!(wcontainers_processor_architecture_arm64::Instruction::{name})\n            }}"
        )
        .expect("writing to a string succeeds");
    }
    out.push_str(
        "            Self::LevelSpecific(never) => match never {},\n        }\n    }\n}\n",
    );
    let path =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join(GENERATED);
    std::fs::write(path, out).expect("the generated list is writable");
}

/// Reads `name@high:low[,high:low]=meaning; ...`, leaving out the bits the manual says should hold a value.
fn read_fields(column: &str) -> Vec<ListField> {
    column
        .split(FIELD_SEPARATOR)
        .filter(|field| !field.is_empty())
        .filter_map(|field| {
            let (name, rest) = field.split_once('@').expect("a field names its bits");
            let (runs, meaning) = rest.split_once('=').expect("a field has a meaning");
            let runs = runs
                .split(RUN_SEPARATOR)
                .map(|run| {
                    let (high, low) = run.split_once(':').expect("a run is high:low");
                    Run {
                        high: high.parse().expect("a bit number"),
                        low: low.parse().expect("a bit number"),
                    }
                })
                .collect();
            let words: Vec<&str> = meaning.split(' ').collect();
            let (access, part, llvm_type) = match words.as_slice() {
                ["read", "operand", llvm_type] => (Some(Access::Read), None, *llvm_type),
                ["written", "operand", llvm_type] => (Some(Access::Written), None, *llvm_type),
                ["read", part, "of", "operand", llvm_type] => {
                    (Some(Access::Read), Some((*part).to_owned()), *llvm_type)
                }
                ["encoding", "field", _, "of", record] => (None, None, *record),
                ["should", "be", ..] => return None,
                ["selects", "the", "operand" | "arrangement", ..] => {
                    (Some(Access::Read), None, SELECTOR)
                }
                _ => (None, None, ""),
            };
            Some(ListField {
                name: name.to_owned(),
                runs,
                access,
                llvm_type: llvm_type.to_owned(),
                part,
            })
        })
        .collect()
}

/// Renders the operands in the order of the assembler syntax, then the fields the syntax does not name.
fn render_operands(fields: &[ListField], syntax: &str) -> Vec<String> {
    let mut used = vec![false; fields.len()];
    let mut operands = Vec::new();
    for token in syntax.split(SYNTAX_OPERAND).skip(1) {
        let token: String = token
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        let token = SYNTAX_SYNONYMS
            .iter()
            .find(|(synonym, _)| {
                *synonym == token && !fields.iter().any(|field| field.name == token)
            })
            .map_or(token.as_str(), |(_, name)| name);
        let find = |name: &str| {
            (0..fields.len()).find(|&index| !used[index] && fields[index].name == name)
        };
        if token == REGISTER_OFFSET_FIELDS[0]
            && let Some(extend) = find(REGISTER_OFFSET_FIELDS[1])
            && fields[extend].access.is_none()
            && fields[extend].llvm_type.contains(REGISTER_OFFSET_RECORD)
            && let Some(register) = find(REGISTER_OFFSET_FIELDS[0])
        {
            let doubleword = find(REGISTER_OFFSET_FIELDS[2]);
            for index in [Some(register), Some(extend), doubleword]
                .into_iter()
                .flatten()
            {
                used[index] = true;
            }
            operands.push(render_register_offset(
                &fields[register],
                &fields[extend],
                doubleword.map(|index| &fields[index]),
            ));
            continue;
        }
        if token == BASE_REGISTER
            && syntax.contains(ADDRESS_BASE)
            && let Some(base) = find(BASE_REGISTER)
            && let [run] = fields[base].runs.as_slice()
        {
            used[base] = true;
            let indexing = if syntax.contains(PRE_INDEX) {
                "Indexing::PreIndex"
            } else if syntax.contains(POST_INDEX) {
                "Indexing::PostIndex"
            } else {
                "Indexing::Offset"
            };
            operands.push(format!(
                "Operand::AddressBase {{ number: {}, indexing: {indexing} }}",
                run.render()
            ));
            continue;
        }
        let parts: Vec<&str> = token.split(GROUP_JOINER).collect();
        let members: Vec<usize> = parts
            .iter()
            .filter_map(|part| {
                (0..fields.len()).find(|&index| !used[index] && fields[index].name == *part)
            })
            .collect();
        if members.len() != parts.len() {
            continue;
        }
        for &index in &members {
            used[index] = true;
        }
        let members: Vec<&ListField> = members.iter().map(|&index| &fields[index]).collect();
        operands.push(render_operand(&members));
    }
    for (field, _) in fields.iter().zip(&used).filter(|(_, used)| !**used) {
        operands.push(render_operand(&[field]));
    }
    operands
}

/// A register offset from its register, the field of option<2> and S, and the field of option<0> where it varies.
fn render_register_offset(
    register: &ListField,
    extend: &ListField,
    doubleword: Option<&ListField>,
) -> String {
    let ([number], runs) = (register.runs.as_slice(), extend.runs.as_slice()) else {
        return "Operand::Unmodeled".to_owned();
    };
    let (signed, shift) = match runs {
        [signed, shift] => (format!("Some({})", signed.render()), shift.render()),
        [shift] => ("None".to_owned(), shift.render()),
        _ => return "Operand::Unmodeled".to_owned(),
    };
    let doubleword = match doubleword.map(|field| field.runs.as_slice()) {
        Some([run]) => format!("Some({})", run.render()),
        _ => "None".to_owned(),
    };
    format!(
        "Operand::RegisterOffset {{ number: {}, signed: {signed}, doubleword: {doubleword}, shift: {shift} }}",
        number.render()
    )
}

fn render_operand(members: &[&ListField]) -> String {
    let unmodeled = "Operand::Unmodeled".to_owned();
    let first = members[0];
    let single_run = |field: &ListField| match field.runs.as_slice() {
        [run] => Some(run.render()),
        _ => None,
    };
    let find_part = |part: &str| {
        members
            .iter()
            .find(|member| member.part.as_deref() == Some(part))
    };
    let width = |llvm_type: &str| {
        if llvm_type.contains(SIXTY_FOUR_BITS) {
            "RegisterWidth::Bits64"
        } else {
            "RegisterWidth::Bits32"
        }
    };
    if first.name == IGNORED_FIELD
        && let Some(field) = single_run(first)
    {
        return format!("Operand::Ignored({field})");
    }
    match (members.len(), first.llvm_type.as_str()) {
        (1, "FPR16" | "FPR32" | "FPR64") => {
            let Some(number) = single_run(first) else {
                return unmodeled;
            };
            let precision = match first.llvm_type.as_str() {
                "FPR16" => "Precision::Half",
                "FPR32" => "Precision::Single",
                _ => "Precision::Double",
            };
            let access = if first.access == Some(Access::Written) {
                "OperandAccess::Written"
            } else {
                "OperandAccess::Read"
            };
            format!(
                "Operand::FloatingPointRegister {{ number: {number}, precision: {precision}, access: {access} }}"
            )
        }
        // The data register of a load or store of SIMD and floating-point registers.
        (
            1,
            "FPR8" | "FPR8Op" | "FPR16Op" | "FPR32Op" | "FPR64Op" | "FPR128" | "FPR128Op" | "V64"
            | "V128",
        ) => {
            let Some(number) = single_run(first) else {
                return unmodeled;
            };
            let access = if first.access == Some(Access::Written) {
                "OperandAccess::Written"
            } else {
                "OperandAccess::Read"
            };
            format!("Operand::VectorRegister {{ number: {number}, access: {access} }}")
        }
        (1, record) if record.starts_with("fpimm") || record.starts_with("fixedpoint_") => {
            match single_run(first) {
                Some(field) => format!("Operand::UnsignedImmediate({field})"),
                None => unmodeled,
            }
        }
        (1, "GPR32" | "GPR64" | "GPR32sp" | "GPR64sp" | "GPR32z" | "GPR64z" | "GPR64sp0") => {
            let Some(number) = single_run(first) else {
                return unmodeled;
            };
            let thirty_one = if first
                .llvm_type
                .trim_end_matches('0')
                .ends_with(STACK_POINTER_SUFFIX)
            {
                "RegisterThirtyOne::StackPointer"
            } else {
                "RegisterThirtyOne::ZeroRegister"
            };
            let access = if first.access == Some(Access::Written) {
                "OperandAccess::Written"
            } else {
                "OperandAccess::Read"
            };
            format!(
                "Operand::GeneralPurposeRegister {{ number: {number}, width: {}, thirty_one: {thirty_one}, access: {access} }}",
                width(&first.llvm_type)
            )
        }
        (
            2,
            "arith_shifted_reg32"
            | "arith_shifted_reg64"
            | "logical_shifted_reg32"
            | "logical_shifted_reg64",
        ) => {
            let (Some(register), Some(shift)) = (find_part("Rm"), find_part("shift")) else {
                return unmodeled;
            };
            let ([number], [shift_type, amount]) =
                (register.runs.as_slice(), shift.runs.as_slice())
            else {
                return unmodeled;
            };
            let rotation = if first.llvm_type.starts_with(LOGICAL_PREFIX) {
                "Rotation::Allowed"
            } else {
                "Rotation::Reserved"
            };
            format!(
                "Operand::ShiftedRegister {{ number: {}, shift_type: {}, amount: {}, width: {}, rotation: {rotation} }}",
                number.render(),
                shift_type.render(),
                amount.render(),
                width(&first.llvm_type)
            )
        }
        (
            2,
            "arith_extended_reg32_i32"
            | "arith_extended_reg32to64_i64"
            | "arith_extended_reg32_i64",
        ) => {
            let (Some(register), Some(extend)) = (find_part("Rm"), find_part("extend")) else {
                return unmodeled;
            };
            let ([number], [extend]) = (register.runs.as_slice(), extend.runs.as_slice()) else {
                return unmodeled;
            };
            // The extend field is the extend type above a three-bit left shift.
            const AMOUNT_BITS: u32 = 3;
            let amount = Run {
                high: extend.low + AMOUNT_BITS - 1,
                low: extend.low,
            };
            let extend_type = Run {
                high: extend.high,
                low: extend.low + AMOUNT_BITS,
            };
            format!(
                "Operand::ExtendedRegister {{ number: {}, extend_type: {}, amount: {}, width: {} }}",
                number.render(),
                extend_type.render(),
                amount.render(),
                width(&first.llvm_type)
            )
        }
        (1, record) if ARITHMETIC_IMMEDIATE_RECORDS.contains(&record) => {
            let [run] = first.runs.as_slice() else {
                return unmodeled;
            };
            let value = Run {
                high: run.high - 1,
                low: run.low,
            };
            let shift = Run {
                high: run.high,
                low: run.high,
            };
            format!(
                "Operand::ArithmeticImmediate {{ value: {}, shift: {} }}",
                value.render(),
                shift.render()
            )
        }
        (1, "logical_imm32" | "logical_imm64") => match single_run(first) {
            Some(field) => format!(
                "Operand::BitmaskImmediate {{ field: {field}, width: {} }}",
                width(&first.llvm_type)
            ),
            None => unmodeled,
        },
        (1, "movimm32_shift" | "movimm64_shift") => match single_run(first) {
            Some(field) => format!(
                "Operand::WideImmediateShift {{ field: {field}, width: {} }}",
                width(&first.llvm_type)
            ),
            None => unmodeled,
        },
        (
            1,
            "movimm32_imm" | "imm0_15" | "imm0_31" | "imm0_63" | "imm32_0_15" | "imm32_0_31"
            | "uimm6" | "uimm8_32b" | "uimm8_64b" | "timm32_0_65535" | "uimm16" | "barrier_op"
            | SELECTOR | "tbz_imm0_31_diag" | "tbz_imm32_63",
        ) => match single_run(first) {
            Some(field) => format!("Operand::UnsignedImmediate({field})"),
            // An arrangement split over Q and size.
            None if first.llvm_type == SELECTOR => {
                let fields: Vec<String> = first.runs.iter().map(Run::render).collect();
                format!("Operand::SplitImmediate(&[{}])", fields.join(", "))
            }
            None => unmodeled,
        },
        (
            1,
            "simm9" | "uimm12s1" | "uimm12s2" | "uimm12s4" | "uimm12s8" | "uimm12s16" | "simm7s4"
            | "simm7s8" | "simm7s16",
        ) => {
            let Some(field) = single_run(first) else {
                return unmodeled;
            };
            let signedness = if first.llvm_type.starts_with('s') {
                "Signedness::Signed"
            } else {
                "Signedness::Unsigned"
            };
            // The units of the offset follow the `s` that ends the type's bit count, in bytes.
            let units: u32 = first
                .llvm_type
                .rsplit_once('s')
                .and_then(|(_, units)| units.parse().ok())
                .unwrap_or(1);
            format!(
                "Operand::AddressOffset {{ field: {field}, signedness: {signedness}, scale: {} }}",
                units.trailing_zeros()
            )
        }
        (1, "WSeqPairClassOperand" | "XSeqPairClassOperand") => match single_run(first) {
            Some(number) => format!(
                "Operand::GeneralPurposeRegisterPair {{ number: {number}, width: {} }}",
                if first.llvm_type.starts_with('X') {
                    "RegisterWidth::Bits64"
                } else {
                    "RegisterWidth::Bits32"
                }
            ),
            None => unmodeled,
        },
        // TBZ's bit number, b5:b40, as Arm's one encoding has it for both register widths.
        (1, "rprfop" | "tbz_imm0_63") => {
            let fields: Vec<String> = first.runs.iter().map(Run::render).collect();
            format!("Operand::SplitImmediate(&[{}])", fields.join(", "))
        }
        (1, "prfop") => match single_run(first) {
            Some(field) => format!("Operand::UnsignedImmediate({field})"),
            None => unmodeled,
        },
        (1, "simm8_32b" | "simm8_64b") => match single_run(first) {
            Some(field) => format!("Operand::SignedImmediate({field})"),
            None => unmodeled,
        },
        (1, "ccode") => match single_run(first) {
            Some(field) => format!("Operand::Condition({field})"),
            None => unmodeled,
        },
        (
            1,
            "adrlabel" | "adrplabel" | "am_b_target" | "am_bl_target" | "am_brcond" | "am_tbrcond"
            | "am_ldrlit",
        ) => {
            // The units of the offset, as a left shift of its bytes.
            let scale = match first.llvm_type.as_str() {
                "adrlabel" => 0,
                "adrplabel" => PAGE_SCALE,
                _ => INSTRUCTION_SCALE,
            };
            let fields: Vec<String> = first.runs.iter().map(Run::render).collect();
            format!(
                "Operand::ProgramCounterOffset {{ fields: &[{}], scale: {scale} }}",
                fields.join(", ")
            )
        }
        _ => unmodeled,
    }
}
