//! AMX's tile forms this chip runs, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 gives them: LDTILECFG, STTILECFG, TILELOADD, TILELOADDT1, TILESTORED, TILERELEASE, TILEZERO, TDPBSSD to
//! TDPBUUD and TDPBF16PS, under palette 1 of eight tiles of up to 16 rows of up to 64 bytes.
//!
//! TILECFG and TILEDATA are XSAVE's components 17 and 18; every form raises #UD unless XCR0 enables both. IA32_XFD's
//! bit 18, which Linux arms for a thread that has not asked for the tiles, makes every form that uses TILEDATA raise
//! #NM; LDTILECFG and TILERELEASE only initialize it and STTILECFG does not use it, so they never do (volume 1,
//! 13.14). TILELOADD and TILESTORED move one row after another and, where a row faults, leave TILECFG's start row
//! at it, so that the form restarts there.

use alloc::boxed::Box;
use alloc::vec::Vec;

use iced_x86::Code;

use crate::behavior::StoreAcrossFault;
use crate::xsave::ZeroedTracking;
use crate::{Instruction, PreciseInstruction};

/// TILECFG's bytes, as LDTILECFG reads them.
pub const TILE_CONFIGURATION_BYTES: usize = 64;
/// The tiles palette 1 names, TMM0 to TMM7.
pub const TILES: usize = 8;
/// Palette 1's most rows of a tile and bytes of a row.
pub const TILE_ROWS: usize = 16;
pub const ROW_BYTES: usize = 64;
/// TILEDATA's bytes: every tile's every row.
pub const TILE_DATA_BYTES: usize = TILES * TILE_ROWS * ROW_BYTES;
/// XCR0's and IA32_XFD's bits of TILECFG and TILEDATA.
pub const TILE_CONFIGURATION_COMPONENT: u64 = 1 << 17;
pub const TILE_DATA_COMPONENT: u64 = 1 << 18;
pub const TILE_COMPONENTS: u64 = TILE_CONFIGURATION_COMPONENT | TILE_DATA_COMPONENT;

/// TILECFG's fields: the palette, the start row, each tile's bytes a row (two bytes each) and its rows.
const PALETTE: usize = 0;
const START_ROW: usize = 1;
const ROW_BYTES_FIELD: usize = 16;
const ROWS_FIELD: usize = 48;
/// The one palette besides palette 0, which is the initial configuration.
const LARGEST_PALETTE: u8 = 1;

/// Zeros, which TILEDATA holds in its initial configuration.
static ZERO_TILE_DATA: [u8; TILE_DATA_BYTES] = [0; TILE_DATA_BYTES];

/// TILEDATA: each tile's rows of [`ROW_BYTES`] in turn, tile 0's first, as XSAVE lays them out; zeros are kept as
/// nothing, so that a state without tiles costs nothing to copy.
#[derive(Clone, Debug)]
pub struct TileData(Option<Box<[u8; TILE_DATA_BYTES]>>);

impl TileData {
    pub const ZERO: Self = Self(None);

    pub fn as_bytes(&self) -> &[u8; TILE_DATA_BYTES] {
        self.0.as_deref().unwrap_or(&ZERO_TILE_DATA)
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8; TILE_DATA_BYTES] {
        self.0.get_or_insert_with(|| Box::new([0; TILE_DATA_BYTES]))
    }

    pub fn is_zero(&self) -> bool {
        self.0.as_deref().is_none_or(|bytes| bytes.iter().all(|byte| *byte == 0))
    }

    pub fn read_row(&self, tile: usize, row: usize) -> &[u8] {
        let start = (tile * TILE_ROWS + row) * ROW_BYTES;
        &self.as_bytes()[start..start + ROW_BYTES]
    }

    pub fn write_row(&mut self, tile: usize, row: usize) -> &mut [u8] {
        let start = (tile * TILE_ROWS + row) * ROW_BYTES;
        &mut self.as_mut_bytes()[start..start + ROW_BYTES]
    }
}

impl PartialEq for TileData {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            _ => self.as_bytes() == other.as_bytes(),
        }
    }
}

impl Eq for TileData {}

/// One tile's shape in TILECFG: its rows and its bytes a row; a tile of neither is not valid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileShape {
    pub rows: usize,
    pub row_bytes: usize,
}

impl TileShape {
    pub const fn is_valid(self) -> bool {
        self.rows != 0
    }
}

/// Whether TILECFG configures tiles: its palette is not 0.
pub const fn is_configured(configuration: &[u8; TILE_CONFIGURATION_BYTES]) -> bool {
    configuration[PALETTE] != 0
}

pub const fn read_start_row(configuration: &[u8; TILE_CONFIGURATION_BYTES]) -> usize {
    configuration[START_ROW] as usize
}

pub fn read_shape(configuration: &[u8; TILE_CONFIGURATION_BYTES], tile: usize) -> TileShape {
    let at = ROW_BYTES_FIELD + 2 * tile;
    TileShape {
        rows: usize::from(configuration[ROWS_FIELD + tile]),
        row_bytes: usize::from(u16::from_le_bytes([configuration[at], configuration[at + 1]])),
    }
}

/// The configuration that names these shapes under palette 1, with the start row.
pub fn write_configuration(shapes: [TileShape; TILES], start_row: u8) -> [u8; TILE_CONFIGURATION_BYTES] {
    let mut configuration = [0; TILE_CONFIGURATION_BYTES];
    configuration[PALETTE] = LARGEST_PALETTE;
    configuration[START_ROW] = start_row;
    for (tile, shape) in shapes.iter().enumerate() {
        let at = ROW_BYTES_FIELD + 2 * tile;
        configuration[at..at + 2].copy_from_slice(&(shape.row_bytes as u16).to_le_bytes());
        configuration[ROWS_FIELD + tile] = shape.rows as u8;
    }
    configuration
}

/// The TILECFG LDTILECFG loads from these bytes, or `None` where it raises #GP, as the manual's pseudocode checks
/// them: palette 0 or 1, every reserved byte zero, each tile of palette 1 at most 16 rows of at most 64 bytes, and of
/// rows and bytes either both or neither. Palette 0 reads no start row and names no tile, so it loads zeros.
pub fn check_configuration(bytes: &[u8; TILE_CONFIGURATION_BYTES], palette_zero_ignores_fields: bool) -> Option<[u8; TILE_CONFIGURATION_BYTES]> {
    let palette = bytes[PALETTE];
    if palette > LARGEST_PALETTE {
        return None;
    }
    if palette == 0 && palette_zero_ignores_fields {
        return Some([0; TILE_CONFIGURATION_BYTES]);
    }
    let names = if palette == 0 { 0 } else { TILES };
    let zero = |range: core::ops::Range<usize>| bytes[range].iter().all(|byte| *byte == 0);
    if !zero(START_ROW + 1..ROW_BYTES_FIELD) || !zero(ROW_BYTES_FIELD + 2 * names..ROWS_FIELD) || !zero(ROWS_FIELD + names..TILE_CONFIGURATION_BYTES) {
        return None;
    }
    for tile in 0..names {
        let shape = read_shape(bytes, tile);
        if shape.row_bytes > ROW_BYTES || shape.rows > TILE_ROWS || (shape.rows == 0) != (shape.row_bytes == 0) {
            return None;
        }
    }
    Some(if palette == 0 { [0; TILE_CONFIGURATION_BYTES] } else { *bytes })
}

/// What a form of this family does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmxOperation {
    /// LDTILECFG: TILECFG from 64 bytes of memory, which zeroes TILEDATA.
    LoadConfiguration,
    /// STTILECFG: TILECFG to 64 bytes of memory, zeros where no tiles are configured.
    StoreConfiguration,
    /// TILELOADD and TILELOADDT1: the tile's rows from memory, from the start row on, a row at the base plus its
    /// number times the index register's value scaled; each row's bytes past the tile's and each row past its rows
    /// zeroed.
    LoadTile,
    /// TILESTORED: the tile's rows to memory as TILELOADD reads them.
    StoreTile,
    /// TILERELEASE: TILECFG and TILEDATA to their initial configuration.
    Release,
    /// TILEZERO: every byte of the tile zeroed.
    ZeroTile,
    /// TDPBSSD, TDPBSUD, TDPBUSD and TDPBUUD: each doubleword of the destination plus, for each doubleword of a row
    /// of the first source and of a column of the second, the products of their four bytes, signed where named so.
    DotBytes { first_signed: bool, second_signed: bool },
    /// TDPBF16PS: each single of the destination plus the sum of two sums over a row of the first source and a column
    /// of the second, of the products of their lower bfloat16 halves and of their upper ones; every step a fused
    /// multiply-add or an addition rounded to nearest with denormals read and written as zeros, MXCSR neither read
    /// nor written.
    DotBfloat16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AmxForm {
    pub instruction: Instruction,
    pub operation: AmxOperation,
    pub code: Code,
}

impl AmxForm {
    /// Whether the form uses TILEDATA, so that XFD makes it raise #NM.
    pub const fn uses_tile_data(&self) -> bool {
        !matches!(self.operation, AmxOperation::LoadConfiguration | AmxOperation::StoreConfiguration | AmxOperation::Release)
    }

    pub const fn has_memory_operand(&self) -> bool {
        matches!(
            self.operation,
            AmxOperation::LoadConfiguration | AmxOperation::StoreConfiguration | AmxOperation::LoadTile | AmxOperation::StoreTile
        )
    }

    pub const fn writes_memory(&self) -> bool {
        matches!(self.operation, AmxOperation::StoreConfiguration | AmxOperation::StoreTile)
    }

    /// The tiles the form names: TDP's destination and two sources, a load's or store's one, TILEZERO's one.
    pub const fn count_tiles(&self) -> usize {
        match self.operation {
            AmxOperation::DotBytes { .. } | AmxOperation::DotBfloat16 => 3,
            AmxOperation::LoadTile | AmxOperation::StoreTile | AmxOperation::ZeroTile => 1,
            _ => 0,
        }
    }
}

pub fn find_amx_form(instruction: Instruction) -> Option<&'static AmxForm> {
    AMX_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn amx(instruction: Instruction, operation: AmxOperation, code: Code) -> AmxForm {
    AmxForm { instruction, operation, code }
}

const fn dot(first_signed: bool, second_signed: bool) -> AmxOperation {
    AmxOperation::DotBytes { first_signed, second_signed }
}

/// Every form of this family modeled: the AMX-TILE, AMX-INT8 and AMX-BF16 forms. This chip lacks AMX-FP16's
/// TDPFP16PS and AMX-COMPLEX's TCMMIMFP16PS and TCMMRLFP16PS, which keep the behavior that raises #UD alone.
pub static AMX_FORMS: &[AmxForm] = &[
    amx(Instruction::LdtilecfgMem, AmxOperation::LoadConfiguration, Code::VEX_Ldtilecfg_m512),
    amx(Instruction::SttilecfgMem, AmxOperation::StoreConfiguration, Code::VEX_Sttilecfg_m512),
    amx(Instruction::TileloaddTmmu32Memu32, AmxOperation::LoadTile, Code::VEX_Tileloadd_tmm_sibmem),
    amx(Instruction::Tileloaddt1Tmmu32Memu32, AmxOperation::LoadTile, Code::VEX_Tileloaddt1_tmm_sibmem),
    amx(Instruction::TilestoredMemu32Tmmu32, AmxOperation::StoreTile, Code::VEX_Tilestored_sibmem_tmm),
    amx(Instruction::Tilerelease, AmxOperation::Release, Code::VEX_Tilerelease),
    amx(Instruction::TilezeroTmmu32, AmxOperation::ZeroTile, Code::VEX_Tilezero_tmm),
    amx(Instruction::TdpbssdTmmi32Tmm4i8Tmm4i8, dot(true, true), Code::VEX_Tdpbssd_tmm_tmm_tmm),
    amx(Instruction::TdpbsudTmmi32Tmm4i8Tmm4u8, dot(true, false), Code::VEX_Tdpbsud_tmm_tmm_tmm),
    amx(Instruction::TdpbusdTmmi32Tmm4u8Tmm4i8, dot(false, true), Code::VEX_Tdpbusd_tmm_tmm_tmm),
    amx(Instruction::TdpbuudTmmu32Tmm4u8Tmm4u8, dot(false, false), Code::VEX_Tdpbuud_tmm_tmm_tmm),
    amx(Instruction::Tdpbf16psTmmf32Tmm2bf16Tmm2bf16, AmxOperation::DotBfloat16, Code::VEX_Tdpbf16ps_tmm_tmm_tmm),
];

/// Where a form that uses TILEDATA checks that XFD disables it, which the manual's list of exceptions does not order:
/// before every check of TILECFG, or after those of the tiles' configuration and shape and before that of the start
/// row, which only a tile load or store makes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::amx)
)]
pub enum DisabledDataCheck {
    BeforeConfiguration,
    BeforeStartRow,
}

/// How LDTILECFG reads palette 0, which the manual's pseudocode leaves unclear: as the initial configuration whatever
/// the other bytes hold, or with every byte but the start row's checked for zero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::amx)
)]
pub enum PaletteZero {
    IgnoresFields,
    ChecksReservedBytes,
}

/// A form's behavior where the manuals leave a choice: what a store that faults part-way writes; what XINUSE says of
/// the TILEDATA LDTILECFG zeroes; which exception comes first where XFD disables TILEDATA; how palette 0 is read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::amx)
)]
pub struct AmxBehavior {
    pub across_fault: StoreAcrossFault,
    pub zeroed: ZeroedTracking,
    pub disabled: DisabledDataCheck,
    pub palette_zero: PaletteZero,
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];
const ZEROED: [ZeroedTracking; 2] = [ZeroedTracking::TrackedInUse, ZeroedTracking::TrackedInitial];
const DISABLED: [DisabledDataCheck; 2] = [DisabledDataCheck::BeforeConfiguration, DisabledDataCheck::BeforeStartRow];
const PALETTE_ZERO: [PaletteZero; 2] = [PaletteZero::ChecksReservedBytes, PaletteZero::IgnoresFields];

/// All the choices where the form leaves them open, else the first.
fn take<T>(all: &[T], open: bool) -> &[T] {
    if open { all } else { &all[..1] }
}

/// Every combination of the choices the form leaves open, the first of each where it has none.
pub(crate) fn list_amx_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_amx_form(instruction) else {
        return Vec::new();
    };
    let configuration = form.operation == AmxOperation::LoadConfiguration;
    let mut behaviors = Vec::new();
    for &across_fault in take(&ACROSS_FAULT, form.writes_memory()) {
        for &zeroed in take(&ZEROED, configuration) {
            for &disabled in take(&DISABLED, form.uses_tile_data()) {
                for &palette_zero in take(&PALETTE_ZERO, configuration) {
                    let behavior = AmxBehavior { across_fault, zeroed, disabled, palette_zero };
                    behaviors.push(PreciseInstruction::Amx { instruction, behavior });
                }
            }
        }
    }
    behaviors
}
