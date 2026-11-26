//! The parts of the instruction database that XED's `pysrc/xed_to_db.py` writes which the data file is made from.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Database {
    #[serde(rename = "Version")]
    pub version: String,
    /// One record per encoding: an instruction form has one or more.
    #[serde(rename = "Instructions")]
    pub records: Vec<Record>,
}

#[derive(Deserialize)]
pub struct Record {
    pub iclass: String,
    pub disasm_intel: String,
    pub iform: String,
    pub isa_set: String,
    pub extension: String,
    pub category: String,
    pub pattern: String,
    pub attributes: Vec<String>,
    /// The lowest privilege level the instruction runs at: 0 or 3.
    pub cpl: u8,
    pub encoding_space: String,
    pub cpuid_groups: Vec<CpuidGroup>,
    pub flags: Option<String>,
    pub parsed_operands: Vec<Operand>,
    pub rexw_prefix: Option<bool>,
    pub vl: Option<u16>,
    pub evex_pp: Option<u8>,
    pub rex2_restriction: String,
    pub f2_required: bool,
    pub f3_required: bool,
    pub osz_66_required: bool,
    pub no_prefixes_allowed: bool,
    pub map: u8,
    pub opcode_base16: String,
    pub partial_opcode: bool,
    pub reg_required: Vec<u8>,
    pub rm_required: Option<u8>,
    pub has_modrm: bool,
    pub mod_required: Vec<u8>,
    /// The processor modes the encoding is valid in, by their default address size: 16, 32 and 64.
    pub mode_restriction: Vec<u8>,
}

/// CPUID bits that together enable an instruction; any one group of a record suffices.
#[derive(Deserialize)]
pub struct CpuidGroup {
    pub group_name: String,
    pub records: Vec<CpuidRecord>,
}

#[derive(Deserialize)]
pub struct CpuidRecord {
    pub fname: String,
    pub leaf: u32,
    pub subleaf: u32,
    pub reg: String,
    pub bit_start: u8,
    pub bit_end: u8,
    pub value: u32,
}

#[derive(Deserialize)]
pub struct Operand {
    pub name: String,
    #[serde(rename = "type")]
    pub operand_type: String,
    pub visibility: String,
    pub rw: String,
    pub lookupfn_name: Option<String>,
    /// The fixed register of a `reg` operand, such as `XED_REG_CL`.
    pub bits: Option<String>,
    pub oc2: Option<String>,
    pub xtype: Option<String>,
    pub op_widths: Option<Widths>,
}

#[derive(Deserialize)]
pub struct Widths {
    pub fixed: Option<u32>,
    pub scalable: Option<ScalableWidths>,
}

/// The width under each effective operand size.
#[derive(Deserialize)]
pub struct ScalableWidths {
    pub eosz16: u32,
    pub eosz32: u32,
    pub eosz64: u32,
}

impl Database {
    pub fn parse(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }
}
