//! The inputs of AMX's tile forms: XCR0 and IA32_XFD with TILECFG before, together; the tiles a form names; where
//! a memory operand lies and how its rows step; the 64 bytes LDTILECFG reads; and what the tiles hold.
//!
//! A state whose XCR0 does not enable the tiles holds no tile state, and one whose XFD disables TILEDATA holds it in
//! its initial configuration, as Linux keeps a thread's that has not asked for the tiles; so does one with no tiles
//! configured, as no chip loads TILEDATA then. XINUSE marks TILECFG where tiles are configured and TILEDATA where it
//! holds anything. IA32_XFD_ERR is clear where XFD disables TILEDATA, so that its #NM shows the load, and holds
//! TILEDATA's bit where the tiles are enabled.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Instruction as Encoded, MemoryOperand, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::amx::{
    AmxForm, AmxOperation, ROW_BYTES, TILE_COMPONENTS, TILE_CONFIGURATION_BYTES, TILE_CONFIGURATION_COMPONENT,
    TILE_DATA_BYTES, TILE_DATA_COMPONENT, TILE_ROWS, TILES, TileData, TileShape, find_amx_form,
    read_shape, read_start_row, write_configuration,
};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::xsave::LINUX_XCR0;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Instruction, InstructionFetch, PKRU_DEFAULT, PagePermission, Processor, STACK_POINTER, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, encode_at_code_address, lay_out};
use crate::{CODE_ADDRESS, FlagSet, LOWER_DATA_ADDRESS, OperandType, OperandValue, UPPER_DATA_ADDRESS};

/// RBX and RSI, the base and index of a memory operand that does not name RSP.
const BASE: usize = 3;
const INDEX: usize = 6;
/// The lowest address above the lower canonical half.
const NON_CANONICAL_ADDRESS: u64 = 1 << 63;
/// A canonical address in the kernel's half.
const KERNEL_ADDRESS: u64 = 0xFFFF_8000_0000_1000;

/// XCR0 and IA32_XFD: the tiles not enabled, enabled, or enabled with TILEDATA disabled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Environment {
    NotEnabled,
    Enabled,
    DataDisabled,
}

/// What TILECFG holds before, as an index into [`CONFIGURATIONS`], with the environment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileState {
    pub environment: Environment,
    pub configuration: usize,
}

/// The configurations a form's states start from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfigurationSet {
    /// Every one: for the forms that read TILECFG whole or only replace it.
    All,
    /// None, every tile whole, and tiles of mixed shapes: for TDP and TILEZERO.
    Shapes,
}

/// What the tiles hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentSet {
    /// Zeros and bytes of a pattern: for the forms that only move or zero tiles.
    Moved,
    /// Bytes for the integer dot products: their every sign, the extremes and sums at the doubleword's edges.
    Integer,
    /// Bfloat16 pairs and singles for TDPBF16PS: rounding, cancellation, the specials and both ends of the range.
    Bfloat16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Contents {
    Zero,
    /// Bytes of a pattern of this seed.
    Pattern(u32),
    /// Doublewords whose bytes are each sign's extremes, the accumulators at the doubleword's edges.
    IntegerEdges,
    /// Bfloat16 values of the kind, from this seed.
    Bfloat16 { kind: Bfloat16Kind, seed: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bfloat16Kind {
    /// Normal values of every sign and of exponents near each other, whose sums round.
    Rounding,
    /// Values with their negations and nearly so, whose sums cancel.
    Cancelling,
    /// Zeros, infinities, quiet and signaling NaNs, denormals and values at both ends of the range among normal ones.
    Special,
}

/// Where a tile form's memory operand lies: its base, the index register's value and scale where it names one, the
/// two data pages' permissions, and whether the base is RSP.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Placement {
    pub base: u64,
    pub index: Option<(u64, u32)>,
    pub lower: PagePermission,
    pub upper: PagePermission,
    pub stack: bool,
}

const READ_WRITE: PagePermission =
    PagePermission::Mapped { data_access: DataAccess::ReadWrite, instruction_fetch: InstructionFetch::Forbidden };
const READ_ONLY: PagePermission =
    PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Forbidden };
const UNMAPPED: PagePermission = PagePermission::Unmapped;

const fn place(base: u64, index: Option<(u64, u32)>, lower: PagePermission, upper: PagePermission) -> Placement {
    Placement { base, index, lower, upper, stack: false }
}

/// Every tile of 16 rows of 64 bytes.
const FULL: [TileShape; TILES] = [TileShape { rows: TILE_ROWS, row_bytes: ROW_BYTES }; TILES];
/// Tiles of mixed shapes: TMM0 to TMM2 fit TDP as destination and sources, TMM3 is not valid, TMM4's rows are not
/// doublewords, and TMM5 to TMM7 fit no TDP together.
const MIXED: [TileShape; TILES] = [
    TileShape { rows: 3, row_bytes: 12 },
    TileShape { rows: 3, row_bytes: 8 },
    TileShape { rows: 2, row_bytes: 12 },
    TileShape { rows: 0, row_bytes: 0 },
    TileShape { rows: 16, row_bytes: 6 },
    TileShape { rows: 1, row_bytes: 4 },
    TileShape { rows: 16, row_bytes: 64 },
    TileShape { rows: 5, row_bytes: 20 },
];

/// TILECFG before: none configured; every tile whole; mixed shapes; every tile whole from row 3, as a restart leaves
/// it; mixed shapes from row 3, at or past some tiles' rows.
pub fn list_configurations() -> [[u8; TILE_CONFIGURATION_BYTES]; 5] {
    [
        [0; TILE_CONFIGURATION_BYTES],
        write_configuration(FULL, 0),
        write_configuration(MIXED, 0),
        write_configuration(FULL, 3),
        write_configuration(MIXED, 3),
    ]
}

/// The bytes LDTILECFG reads: palette 0 plain and with each field set; palette 1 whole, mixed from row 7, with one
/// tile of an odd width, with no tile, from row 255; and each of the checks palette 1 makes failing once.
pub fn list_images() -> Vec<[u8; TILE_CONFIGURATION_BYTES]> {
    let with = |mut bytes: [u8; TILE_CONFIGURATION_BYTES], at: usize, value: u8| {
        bytes[at] = value;
        bytes
    };
    let zero = [0; TILE_CONFIGURATION_BYTES];
    let full = write_configuration(FULL, 0);
    let mut odd = [TileShape { rows: 0, row_bytes: 0 }; TILES];
    odd[1] = TileShape { rows: 1, row_bytes: 7 };
    vec![
        zero,
        full,
        write_configuration(MIXED, 7),
        with(zero, 1, 5),
        with(zero, 16, 16),
        with(zero, 2, 1),
        with(zero, 48, 3),
        with(zero, 0, 2),
        with(zero, 0, 0xFF),
        with(full, 2, 1),
        with(full, 15, 0x80),
        with(full, 32, 1),
        with(full, 47, 1),
        with(full, 56, 1),
        with(full, 63, 1),
        with(full, 16 + 2 * 3, 65),
        with(full, 16 + 2 * 3 + 1, 1),
        with(full, 48 + 5, 17),
        with(with(full, 16 + 2 * 2, 0), 48 + 2, 4),
        with(write_configuration(odd, 0), 48 + 2, 0),
        with(with(zero, 0, 1), 16 + 2 * 2, 4),
        write_configuration(odd, 0),
        write_configuration([TileShape { rows: 0, row_bytes: 0 }; TILES], 0),
        write_configuration(FULL, 255),
    ]
}

/// The operand types of a form of this family, in the order the build reads them.
pub fn list_operand_types(form: &AmxForm) -> Vec<OperandType> {
    let unread = OperandType::Flags(FlagSet::Unread);
    match form.operation {
        AmxOperation::LoadConfiguration => vec![
            OperandType::TileState(ConfigurationSet::All),
            OperandType::TilePlacement { write: false, rows: false },
            OperandType::TileConfigurationImage,
            OperandType::TileContents(ContentSet::Moved),
            unread,
        ],
        AmxOperation::StoreConfiguration => vec![
            OperandType::TileState(ConfigurationSet::All),
            OperandType::TilePlacement { write: true, rows: false },
            unread,
        ],
        AmxOperation::LoadTile | AmxOperation::StoreTile => vec![
            OperandType::TileState(ConfigurationSet::All),
            OperandType::TileNames(1),
            OperandType::TilePlacement { write: form.writes_memory(), rows: true },
            OperandType::TileContents(ContentSet::Moved),
            unread,
        ],
        AmxOperation::Release => {
            vec![OperandType::TileState(ConfigurationSet::All), OperandType::TileContents(ContentSet::Moved), unread]
        }
        AmxOperation::ZeroTile => vec![
            OperandType::TileState(ConfigurationSet::Shapes),
            OperandType::TileNames(1),
            OperandType::TileContents(ContentSet::Moved),
            unread,
        ],
        AmxOperation::DotBytes { .. } => vec![
            OperandType::TileState(ConfigurationSet::Shapes),
            OperandType::TileNames(3),
            OperandType::TileContents(ContentSet::Integer),
            unread,
        ],
        AmxOperation::DotBfloat16 => vec![
            OperandType::TileState(ConfigurationSet::Shapes),
            OperandType::TileNames(3),
            OperandType::TileContents(ContentSet::Bfloat16),
            unread,
        ],
    }
}

/// The tiles not enabled, then each configuration enabled, then each with TILEDATA disabled.
pub fn generate_states(set: ConfigurationSet) -> Vec<OperandValue> {
    let configurations: &[usize] = match set {
        ConfigurationSet::All => &[0, 1, 2, 3, 4],
        ConfigurationSet::Shapes => &[0, 1, 2],
    };
    let mut values = vec![TileState { environment: Environment::NotEnabled, configuration: 0 }];
    for environment in [Environment::Enabled, Environment::DataDisabled] {
        values.extend(configurations.iter().map(|&configuration| TileState { environment, configuration }));
    }
    values.into_iter().map(OperandValue::TileState).collect()
}

/// The tiles a form names: one each of the whole, the mixed and the invalid; for TDP the three that fit, three that do
/// not fit, one not valid, and one whose rows are not doublewords. No TDP names a tile twice: that encoding is not TDP's.
pub fn generate_names(count: usize) -> Vec<OperandValue> {
    let names: &[[u8; 3]] = match count {
        1 => &[[0, 0, 0], [3, 0, 0], [4, 0, 0], [6, 0, 0], [7, 0, 0]],
        _ => &[[0, 1, 2], [5, 6, 7], [0, 3, 2], [4, 1, 2], [7, 1, 2]],
    };
    names.iter().copied().map(OperandValue::TileNames).collect()
}

/// Where the memory operand lies. For a configuration: on the lower page; across the boundary into a page that
/// refuses the access; on a page that refuses it; not canonical, through RSP too; in the kernel's half. For tile rows
/// also: rows of 64 bytes scaled from the index, from a misaligned base; no index, so every row at the base; a row
/// across the boundary; rows stepping down; rows a page apart into an unmapped page; a step that leaves the
/// canonical half after the first row.
pub fn generate_placements(write: bool, rows: bool) -> Vec<OperandValue> {
    let refusing = if write { READ_ONLY } else { UNMAPPED };
    let lower = LOWER_DATA_ADDRESS;
    let upper = UPPER_DATA_ADDRESS;
    // A configuration's operand names no index; tile rows of 64 bytes step by RSI's 64.
    let index = rows.then_some((64, 1));
    let mut all = vec![
        place(lower, index, READ_WRITE, READ_WRITE),
        place(upper - 32, index, READ_WRITE, refusing),
        place(upper - 32, index, READ_WRITE, UNMAPPED),
        place(lower, index, refusing, READ_WRITE),
        place(NON_CANONICAL_ADDRESS, index, READ_WRITE, READ_WRITE),
        Placement { stack: true, ..place(NON_CANONICAL_ADDRESS, index, READ_WRITE, READ_WRITE) },
        place(KERNEL_ADDRESS, index, READ_WRITE, READ_WRITE),
    ];
    all.dedup();
    if rows {
        all.extend([
            place(lower + 4, Some((8, 8)), READ_WRITE, READ_WRITE),
            place(lower + 0x100, None, READ_WRITE, READ_WRITE),
            place(upper - 64 * 3 - 8, Some((64, 1)), READ_WRITE, refusing),
            place(upper + 0x100, Some((-64i64 as u64, 1)), refusing, READ_WRITE),
            place(lower + 0x80, Some((PAGE_BYTES, 1)), READ_WRITE, READ_WRITE),
            place(lower, Some((1 << 46, 2)), READ_WRITE, READ_WRITE),
        ]);
    }
    all.into_iter().map(OperandValue::TilePlacement).collect()
}

pub fn generate_images() -> Vec<OperandValue> {
    (0..list_images().len()).map(OperandValue::TileConfigurationImage).collect()
}

/// What the tiles hold, a pattern second so that a state whose outcome does not depend on them holds one.
pub fn generate_contents(set: ContentSet) -> Vec<OperandValue> {
    let contents: Vec<Contents> = match set {
        ContentSet::Moved => vec![Contents::Zero, Contents::Pattern(1)],
        ContentSet::Integer => vec![Contents::Zero, Contents::Pattern(2), Contents::IntegerEdges, Contents::Pattern(3)],
        ContentSet::Bfloat16 => {
            let mut all = vec![Contents::Zero];
            for (kind, seeds) in [(Bfloat16Kind::Rounding, 1..4), (Bfloat16Kind::Cancelling, 4..6), (Bfloat16Kind::Special, 6..9)] {
                all.extend(seeds.map(|seed| Contents::Bfloat16 { kind, seed }));
            }
            all
        }
    };
    contents.into_iter().map(OperandValue::TileContents).collect()
}

/// A generator of 32 bits a step, the same for a seed wherever it runs.
struct Bits(u64);

impl Bits {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }
}

/// TILEDATA for the contents.
pub fn fill_tiles(contents: Contents) -> TileData {
    let mut data = TileData::ZERO;
    let bytes = match contents {
        Contents::Zero => return data,
        _ => data.as_mut_bytes(),
    };
    match contents {
        Contents::Zero => {}
        Contents::Pattern(seed) => {
            let mut bits = Bits(u64::from(seed));
            for chunk in bytes.as_chunks_mut::<4>().0 {
                chunk.copy_from_slice(&bits.next().to_le_bytes());
            }
        }
        Contents::IntegerEdges => {
            const EDGES: [u8; 8] = [0x00, 0x01, 0x7F, 0x80, 0x81, 0xFE, 0xFF, 0x40];
            const ACCUMULATORS: [u32; 6] = [0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF, 0, 0x7FFF_0000, 0x8001_0000];
            for (index, chunk) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let tile = index * 4 / (TILE_DATA_BYTES / TILES);
                let value = if tile.is_multiple_of(3) {
                    ACCUMULATORS[index % ACCUMULATORS.len()]
                } else {
                    u32::from_le_bytes(array::from_fn(|byte| EDGES[(index + byte * 3 + tile) % EDGES.len()]))
                };
                chunk.copy_from_slice(&value.to_le_bytes());
            }
        }
        Contents::Bfloat16 { kind, seed } => {
            let mut bits = Bits(u64::from(seed) << 8 | 0x5A);
            for chunk in bytes.as_chunks_mut::<4>().0 {
                let value = match kind {
                    Bfloat16Kind::Rounding => to_rounding_pair(&mut bits),
                    Bfloat16Kind::Cancelling => to_cancelling_pair(&mut bits),
                    Bfloat16Kind::Special => to_special_pair(&mut bits),
                };
                chunk.copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    data
}

/// A bfloat16 of the sign, an exponent near 127 and any significand.
fn to_near_one(bits: &mut Bits) -> u32 {
    let random = bits.next();
    let exponent = 120 + (random >> 8) % 16;
    (random & 0x8000) | exponent << 7 | random & 0x7F
}

/// Two bfloat16 values near one, as a doubleword that a destination also reads as a single.
fn to_rounding_pair(bits: &mut Bits) -> u32 {
    to_near_one(bits) | to_near_one(bits) << 16
}

/// Two bfloat16 values, the upper the lower's negation in most pairs and nearly so in the rest.
fn to_cancelling_pair(bits: &mut Bits) -> u32 {
    let lower = to_near_one(bits);
    let choice = bits.next();
    let upper = match choice % 4 {
        0 | 1 => lower ^ 0x8000,
        2 => (lower ^ 0x8000) + 1,
        _ => to_near_one(bits),
    };
    lower | (upper & 0xFFFF) << 16
}

/// Two bfloat16 values each a special one in some pairs: ±0, ±infinity, a quiet and a signaling NaN, the smallest
/// and largest denormal, the smallest and largest normal, or a normal value near one.
fn to_special_pair(bits: &mut Bits) -> u32 {
    const SPECIALS: [u32; 12] = [0x0000, 0x8000, 0x7F80, 0xFF80, 0x7FC1, 0x7F81, 0x0001, 0x007F, 0x0080, 0x7F7F, 0xFF7F, 0x8080];
    let half = |bits: &mut Bits| {
        let choice = bits.next();
        if choice.is_multiple_of(4) { SPECIALS[(choice >> 4) as usize % SPECIALS.len()] } else { to_near_one(bits) }
    };
    half(bits) | half(bits) << 16
}

/// Whether the values decide the state's outcome whatever the tiles and the memory hold: the tiles not enabled, or
/// TILEDATA disabled for a form that uses it, or the tiles named not fitting the form, or a memory operand whose first
/// access faults while the form starts at row 0.
pub fn decides_without_data(instruction: Instruction, operands: &[Option<OperandValue>]) -> bool {
    let Some(form) = find_amx_form(instruction) else {
        return false;
    };
    let (mut state, mut names, mut placement) = (None, [0; 3], None);
    for operand in operands.iter().flatten() {
        match *operand {
            OperandValue::TileState(chosen) => state = Some(chosen),
            OperandValue::TileNames(chosen) => names = chosen,
            OperandValue::TilePlacement(chosen) => placement = Some(chosen),
            _ => {}
        }
    }
    let Some(state) = state else {
        return false;
    };
    match state.environment {
        Environment::NotEnabled => return true,
        Environment::DataDisabled if form.uses_tile_data() => return true,
        _ => {}
    }
    let configuration = list_configurations()[state.configuration];
    if form.count_tiles() > 0 && !fits(form, &configuration, names) {
        return true;
    }
    let Some(placement) = placement else {
        return false;
    };
    let start = if form.count_tiles() > 0 { read_start_row(&configuration) } else { 0 };
    let row_bytes = if form.count_tiles() > 0 { read_shape(&configuration, usize::from(names[0])).row_bytes } else { TILE_CONFIGURATION_BYTES };
    start == 0 && faults(placement, form.writes_memory(), row_bytes as u64)
}

/// Whether the tiles named fit the form under the configuration, as the architecture checks them.
fn fits(form: &AmxForm, configuration: &[u8; TILE_CONFIGURATION_BYTES], names: [u8; 3]) -> bool {
    if configuration[0] == 0 {
        return false;
    }
    let shapes = names.map(|name| read_shape(configuration, usize::from(name)));
    let named = &shapes[..form.count_tiles()];
    if named.iter().any(|shape| !shape.is_valid() || !shape.row_bytes.is_multiple_of(4)) {
        return false;
    }
    match form.operation {
        AmxOperation::LoadTile | AmxOperation::StoreTile => read_start_row(configuration) < shapes[0].rows,
        AmxOperation::DotBytes { .. } | AmxOperation::DotBfloat16 => {
            let [destination, first, second] = shapes;
            destination.row_bytes == second.row_bytes
                && destination.rows == first.rows
                && first.row_bytes / 4 == second.rows
        }
        _ => true,
    }
}

/// Whether the first access of `bytes` at the placement faults before it moves anything: a store that faults past its
/// first byte writes the bytes below the fault in one behavior, which depend on what it stores.
fn faults(placement: Placement, write: bool, bytes: u64) -> bool {
    let address = placement.base;
    if address >= 1 << 47 {
        return true;
    }
    let refuses = |permission: PagePermission| match permission {
        PagePermission::Unmapped => true,
        PagePermission::Mapped { data_access, .. } => write && data_access == DataAccess::ReadOnly,
    };
    let page = |at: u64| if at < UPPER_DATA_ADDRESS { placement.lower } else { placement.upper };
    refuses(page(address)) || (!write && refuses(page(address + bytes - 1)))
}

pub fn build_state(form: &AmxForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        pkru: PKRU_DEFAULT,
        ..ZERO_PROCESSOR
    };
    let (mut state, mut names, mut placement, mut image, mut contents) = (None, [0u8; 3], None, None, Contents::Zero);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::TileState(_), OperandValue::TileState(chosen)) => state = Some(chosen),
            (OperandType::TileNames(_), OperandValue::TileNames(chosen)) => names = chosen,
            (OperandType::TilePlacement { .. }, OperandValue::TilePlacement(chosen)) => placement = Some(chosen),
            (OperandType::TileConfigurationImage, OperandValue::TileConfigurationImage(chosen)) => image = Some(chosen),
            (OperandType::TileContents(_), OperandValue::TileContents(chosen)) => contents = chosen,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let state = state.expect("a tile state");
    processor.xcr0 = LINUX_XCR0;
    if state.environment != Environment::NotEnabled {
        processor.xcr0 |= TILE_COMPONENTS;
        processor.tile_configuration = list_configurations()[state.configuration];
        if state.environment == Environment::DataDisabled {
            processor.xfd = TILE_DATA_COMPONENT;
        } else {
            // A stale IA32_XFD_ERR, so that a form shows it keeps it where XFD raises nothing.
            processor.xfd_err = TILE_DATA_COMPONENT;
            if processor.tile_configuration[0] != 0 {
                processor.tile_data = fill_tiles(contents);
            }
        }
    }
    if processor.tile_configuration[0] != 0 {
        processor.xinuse |= TILE_CONFIGURATION_COMPONENT;
    }
    if !processor.tile_data.is_zero() {
        processor.xinuse |= TILE_DATA_COMPONENT;
    }
    let mut memory_mutations = Vec::new();
    let encoded = match form.operation {
        AmxOperation::Release => Encoded::with(form.code),
        AmxOperation::ZeroTile => Encoded::with1(form.code, to_tile(names[0])).expect("TILEZERO takes a tile"),
        AmxOperation::DotBytes { .. } | AmxOperation::DotBfloat16 => {
            Encoded::with3(form.code, to_tile(names[0]), to_tile(names[1]), to_tile(names[2])).expect("TDP takes three tiles")
        }
        _ => {
            let placement = placement.expect("a placement");
            let base = if placement.stack { STACK_POINTER } else { BASE };
            processor.general_purpose_registers[base] = placement.base;
            let index = match placement.index {
                Some((value, scale)) => {
                    processor.general_purpose_registers[INDEX] = value;
                    (Register::RSI, scale)
                }
                None => (Register::None, 1),
            };
            let operand = MemoryOperand::new(GENERAL_64[base], index.0, index.1, 0, 0, false, Register::None);
            memory_mutations.extend(place_pages(placement));
            if let Some(image) = image {
                push_bytes(&mut memory_mutations, placement.base, &list_images()[image]);
            }
            let encoded = match form.operation {
                AmxOperation::LoadTile => Encoded::with2(form.code, to_tile(names[0]), operand),
                AmxOperation::StoreTile => Encoded::with2(form.code, operand, to_tile(names[0])),
                _ => Encoded::with1(form.code, operand),
            };
            encoded.unwrap_or_else(|error| panic!("{:?} takes its operands: {error}", form.code))
        }
    };
    let (code_page, start, bytes) = lay_out(CodeLayout::PageStart, &encode_at_code_address(&encoded), 0x3E);
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

const GENERAL_64: [Register; 16] = [
    Register::RAX, Register::RCX, Register::RDX, Register::RBX, Register::RSP, Register::RBP, Register::RSI, Register::RDI,
    Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13, Register::R14, Register::R15,
];

fn to_tile(name: u8) -> Register {
    [Register::TMM0, Register::TMM1, Register::TMM2, Register::TMM3, Register::TMM4, Register::TMM5, Register::TMM6, Register::TMM7]
        [usize::from(name)]
}

/// The data pages' bytes and the placement's permissions.
fn place_pages(placement: Placement) -> Vec<MemoryDelta<'static, X86_64>> {
    let bytes: Vec<u8> = (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES).map(crate::to_data_byte).collect();
    vec![
        MemoryDelta::ValueAssignment { start: LOWER_DATA_ADDRESS, bytes: Cow::Owned(bytes) },
        MemoryDelta::PermissionAssignment { addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS, permission: placement.lower },
        MemoryDelta::PermissionAssignment { addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES, permission: placement.upper },
    ]
}

/// Writes bytes inside the program's addresses, dropping those past them.
fn push_bytes(mutations: &mut Vec<MemoryDelta<'static, X86_64>>, start: u64, bytes: &[u8]) {
    if start.checked_add(bytes.len() as u64).is_some_and(|end| end <= 1 << 47) {
        mutations.push(MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes.to_vec()) });
    }
}
