//! The skeleton of the AMX forms of [`crate::amx`]: XCR0's check, the checks of the tiles each form names in the
//! order the behavior gives, the memory accesses row by row, and the tiles' results.

use iced_x86::{Instruction as Decoded, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_access, expect_code, locate, read_register, store};
use crate::amx::{
    AmxBehavior, AmxForm, AmxOperation, DisabledDataCheck, PaletteZero, ROW_BYTES, TILE_COMPONENTS, TILE_CONFIGURATION_BYTES,
    TILE_CONFIGURATION_COMPONENT, TILE_DATA_COMPONENT, TILE_ROWS, TileData, TileShape, check_configuration, find_amx_form,
    is_configured, read_shape, read_start_row,
};
use crate::evex::fused::multiply_add;
use crate::float::{DENORMALS_ARE_ZEROS, FLAGS, FLUSH_TO_ZERO, MASK_SHIFT, Precision};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::xsave::ZeroedTracking;
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

/// TDPBF16PS's control: round to nearest, denormals read and written as zeros, every exception masked.
const DOT_CONTROL: u32 = FLAGS << MASK_SHIFT | DENORMALS_ARE_ZEROS | FLUSH_TO_ZERO;
/// One as a single, which turns a fused multiply-add into an addition rounded once.
const SINGLE_ONE: u64 = 0x3F80_0000;
/// The order a fused step takes NaNs in: the first multiplicand's, the second's, the addend's.
const NAN_ORDER: [usize; 3] = [0, 1, 2];

pub(super) fn execute_amx(
    instruction: Instruction,
    behavior: AmxBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_amx_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no AMX form"));
    expect_code(decoded, &[form.code]);
    if processor.xcr0 & TILE_COMPONENTS != TILE_COMPONENTS && !is_active(Bug::TilesWithoutExtendedControl) {
        return Err(Exception::InvalidOpcode);
    }
    let tiles = list_named_tiles(form, decoded);
    check_tiles(form, behavior, processor, &tiles)?;
    match form.operation {
        AmxOperation::LoadConfiguration => load_configuration(behavior, processor, memory, decoded),
        AmxOperation::StoreConfiguration => store_configuration(behavior, processor, memory, decoded, on_memory_mutated),
        AmxOperation::LoadTile => load_tile(tiles[0], processor, memory, decoded),
        AmxOperation::StoreTile => store_tile(behavior, tiles[0], processor, memory, decoded, on_memory_mutated),
        AmxOperation::Release => {
            processor.tile_configuration = [0; TILE_CONFIGURATION_BYTES];
            processor.tile_data = TileData::ZERO;
            if !is_active(Bug::TileReleaseKeepsInUse) {
                processor.xinuse &= !TILE_COMPONENTS;
            }
            Ok(())
        }
        AmxOperation::ZeroTile => {
            let rows = if is_active(Bug::TileZeroOnlyConfiguredRows) {
                read_shape(&processor.tile_configuration, tiles[0]).rows
            } else {
                TILE_ROWS
            };
            for row in 0..rows {
                processor.tile_data.write_row(tiles[0], row).fill(0);
            }
            finish_tile_write(processor);
            Ok(())
        }
        AmxOperation::DotBytes { first_signed, second_signed } => {
            compute_dot(processor, tiles, |sum, first, second| {
                let extend = |byte: u8, signed: bool| if signed { i32::from(byte as i8) } else { i32::from(byte) };
                let (first_signed, second_signed) =
                    if is_active(Bug::TileDotSignsSwapped) { (second_signed, first_signed) } else { (first_signed, second_signed) };
                let products = (0..4).map(|index| {
                    let shift = 8 * index;
                    extend((first >> shift) as u8, first_signed).wrapping_mul(extend((second >> shift) as u8, second_signed))
                });
                products.fold(sum as i32, i32::wrapping_add) as u32
            });
            Ok(())
        }
        AmxOperation::DotBfloat16 => {
            compute_dot_bfloat16(processor, tiles);
            Ok(())
        }
    }
}

/// The tiles the instruction names, in Intel's operand order: TDP's destination, first and second source.
fn list_named_tiles(form: &AmxForm, decoded: &Decoded) -> [usize; 3] {
    let number = |operand: u32| {
        let register = decoded.op_register(operand);
        (register as usize).wrapping_sub(Register::TMM0 as usize)
    };
    match form.operation {
        AmxOperation::DotBytes { .. } | AmxOperation::DotBfloat16 => [number(0), number(1), number(2)],
        AmxOperation::LoadTile | AmxOperation::ZeroTile => [number(0), 0, 0],
        AmxOperation::StoreTile => [number(1), 0, 0],
        _ => [0; 3],
    }
}

/// #NM where XFD disables TILEDATA for a form that uses it, which loads IA32_XFD_ERR, and #UD where the tiles it
/// names are not configured as it needs them, in the behavior's order.
fn check_tiles(form: &AmxForm, behavior: AmxBehavior, processor: &mut Processor, tiles: &[usize; 3]) -> Result<(), Exception> {
    let disabled = form.uses_tile_data()
        && processor.xfd & processor.xcr0 & TILE_DATA_COMPONENT != 0
        && !is_active(Bug::TilesIgnoreFeatureDisable);
    if behavior.disabled == DisabledDataCheck::BeforeConfiguration && disabled {
        return Err(raise_disabled(processor));
    }
    check_shapes(form, processor, tiles)?;
    if disabled {
        return Err(raise_disabled(processor));
    }
    check_start_row(form, processor, tiles[0])?;
    if is_active(Bug::TilesClearFeatureDisableError) {
        processor.xfd_err = 0;
    }
    Ok(())
}

/// The #NM XFD raises for TILEDATA, which loads IA32_XFD_ERR with XFD's TILEDATA bit.
fn raise_disabled(processor: &mut Processor) -> Exception {
    if !is_active(Bug::TileFaultKeepsFeatureDisableError) {
        processor.xfd_err = processor.xfd & TILE_DATA_COMPONENT;
    }
    Exception::DeviceNotAvailable
}

/// #UD where a tile load or store starts at or past its tile's rows.
fn check_start_row(form: &AmxForm, processor: &Processor, tile: usize) -> Result<(), Exception> {
    let moves = matches!(form.operation, AmxOperation::LoadTile | AmxOperation::StoreTile);
    let configuration = &processor.tile_configuration;
    if moves && !is_active(Bug::TileStartRowUnchecked) && read_start_row(configuration) >= read_shape(configuration, tile).rows {
        return Err(Exception::InvalidOpcode);
    }
    Ok(())
}

/// #UD where the form's tiles do not fit it, as the manual's exception classes AMX-E3 to AMX-E5 list.
fn check_shapes(form: &AmxForm, processor: &Processor, tiles: &[usize; 3]) -> Result<(), Exception> {
    let configuration = &processor.tile_configuration;
    let count = form.count_tiles();
    if count == 0 {
        return Ok(());
    }
    let undefined = Err(Exception::InvalidOpcode);
    if !is_configured(configuration) {
        return undefined;
    }
    let shapes: [TileShape; 3] = tiles.map(|tile| read_shape(configuration, tile));
    let named = &shapes[..count];
    if named.iter().any(|shape| !shape.is_valid()) {
        return undefined;
    }
    match form.operation {
        AmxOperation::LoadTile | AmxOperation::StoreTile if !shapes[0].row_bytes.is_multiple_of(4) => return undefined,
        AmxOperation::DotBytes { .. } | AmxOperation::DotBfloat16 => {
            let [destination, first, second] = shapes;
            let fits = destination.row_bytes == second.row_bytes
                && destination.rows == first.rows
                && first.row_bytes / 4 == second.rows;
            let dwords = shapes.iter().all(|shape| shape.row_bytes.is_multiple_of(4));
            // A tile named twice does not decode as TDP, so fetching it raises #UD already.
            if !(fits || is_active(Bug::TileDotShapesUnchecked)) || !dwords {
                return undefined;
            }
        }
        _ => {}
    }
    Ok(())
}

/// LDTILECFG: TILECFG from memory, TILEDATA zeroed and marked in use or not as the behavior tracks it.
fn load_configuration(behavior: AmxBehavior, processor: &mut Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Result<(), Exception> {
    let source = locate(processor, decoded);
    // No AMX form checks alignment.
    check_access(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, source, TILE_CONFIGURATION_BYTES as u64, Access::Read)?;
    let bytes: [u8; TILE_CONFIGURATION_BYTES] = read_bytes(memory, source.address);
    let ignores = behavior.palette_zero == PaletteZero::IgnoresFields;
    let Some(configuration) = check_configuration(&bytes, ignores) else {
        return Err(Exception::GeneralProtection { error_code: 0 });
    };
    processor.tile_configuration = configuration;
    processor.tile_data = TileData::ZERO;
    processor.xinuse &= !TILE_COMPONENTS;
    if is_configured(&configuration) {
        processor.xinuse |= TILE_CONFIGURATION_COMPONENT;
        if behavior.zeroed == ZeroedTracking::TrackedInUse {
            processor.xinuse |= TILE_DATA_COMPONENT;
        }
    }
    Ok(())
}

/// STTILECFG: TILECFG to memory, zeros where no tiles are configured.
fn store_configuration(
    behavior: AmxBehavior,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let destination = locate(processor, decoded);
    let mut bytes = processor.tile_configuration;
    if is_active(Bug::TileConfigurationStoreDropsStartRow) {
        bytes[1] = 0;
    }
    store(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, destination, &bytes, behavior.across_fault, on_memory_mutated)
}

/// The address of the row of a tile load or store: the base and displacement plus the row's number times the index
/// register's value, scaled.
fn locate_row(processor: &Processor, decoded: &Decoded, row: usize) -> MemoryOperand {
    let operand = locate(processor, decoded);
    let index = decoded.memory_index();
    if index == Register::None {
        return operand;
    }
    let value = read_register(processor, index);
    let scale = u64::from(decoded.memory_index_scale());
    let start = operand.address.wrapping_sub(value.wrapping_mul(scale));
    let stride = if is_active(Bug::TileRowStrideUnscaled) { value } else { value.wrapping_mul(scale) };
    MemoryOperand { address: start.wrapping_add(stride.wrapping_mul(row as u64)), stack: operand.stack }
}

/// TILELOADD: the rows from the start row on zeroed, then each loaded in turn; a row that faults leaves the start
/// row at it.
fn load_tile(tile: usize, processor: &mut Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Result<(), Exception> {
    let shape = read_shape(&processor.tile_configuration, tile);
    let start = read_start_row(&processor.tile_configuration);
    let zeroed_from = if is_active(Bug::TileLoadKeepsUpperRows) { shape.rows } else { start };
    for row in zeroed_from..TILE_ROWS {
        processor.tile_data.write_row(tile, row).fill(0);
    }
    processor.xinuse |= TILE_DATA_COMPONENT;
    for row in start..shape.rows {
        let operand = locate_row(processor, decoded, row);
        if let Err(exception) =
            check_access(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, operand, shape.row_bytes as u64, Access::Read)
        {
            processor.tile_configuration[1] = row as u8;
            return Err(exception);
        }
        let bytes: [u8; ROW_BYTES] = read_bytes(memory, operand.address);
        let row_bytes = if is_active(Bug::TileLoadWholeRows) { ROW_BYTES } else { shape.row_bytes };
        let written = processor.tile_data.write_row(tile, row);
        written[..row_bytes].copy_from_slice(&bytes[..row_bytes]);
        written[row_bytes..].fill(0);
    }
    finish_tile_write(processor);
    Ok(())
}

/// TILESTORED: each row from the start row on stored in turn; a row that faults leaves the start row at it.
fn store_tile(
    behavior: AmxBehavior,
    tile: usize,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let shape = read_shape(&processor.tile_configuration, tile);
    let start = read_start_row(&processor.tile_configuration);
    for row in start..shape.rows {
        let operand = locate_row(processor, decoded, row);
        let bytes: [u8; ROW_BYTES] = processor.tile_data.read_row(tile, row).try_into().expect("a row");
        let flags = processor.flags.difference(Flags::ALIGNMENT_CHECK);
        if let Err(exception) = store(flags, memory, operand, &bytes[..shape.row_bytes], behavior.across_fault, &mut on_memory_mutated) {
            processor.tile_configuration[1] = row as u8;
            return Err(exception);
        }
    }
    if !is_active(Bug::TileStoreKeepsStartRow) {
        processor.tile_configuration[1] = 0;
    }
    Ok(())
}

/// What every form that writes a tile does once it completes: the start row back to 0, TILEDATA in use.
fn finish_tile_write(processor: &mut Processor) {
    if !is_active(Bug::TileStoreKeepsStartRow) {
        processor.tile_configuration[1] = 0;
    }
    processor.xinuse |= TILE_DATA_COMPONENT;
}

/// A doubleword of a tile's row.
fn read_dword(data: &TileData, tile: usize, row: usize, index: usize) -> u32 {
    let bytes = data.read_row(tile, row);
    u32::from_le_bytes(bytes[4 * index..4 * index + 4].try_into().expect("four bytes"))
}

/// TDP's skeleton: each destination doubleword of its rows and columns from itself and a row of the first source and a
/// column of the second, the rest of each row and every row past them zeroed.
fn compute_dot(processor: &mut Processor, [destination, first, second]: [usize; 3], step: impl Fn(u32, u32, u32) -> u32) {
    let configuration = processor.tile_configuration;
    let shape = read_shape(&configuration, destination);
    let depth = read_shape(&configuration, first).row_bytes / 4;
    let columns = shape.row_bytes / 4;
    let data = processor.tile_data.clone();
    for row in 0..shape.rows {
        let mut result = [0u8; ROW_BYTES];
        for column in 0..columns {
            let mut sum = read_dword(&data, destination, row, column);
            for inner in 0..depth {
                sum = step(sum, read_dword(&data, first, row, inner), read_dword(&data, second, inner, column));
            }
            result[4 * column..4 * column + 4].copy_from_slice(&sum.to_le_bytes());
        }
        processor.tile_data.write_row(destination, row).copy_from_slice(&result);
    }
    for row in shape.rows..TILE_ROWS {
        processor.tile_data.write_row(destination, row).fill(0);
    }
    finish_tile_write(processor);
}

/// TDPBF16PS: for each element the products of the lower halves summed in one single and of the upper halves in
/// another, each step fused, then their sum added to the destination.
fn compute_dot_bfloat16(processor: &mut Processor, [destination, first, second]: [usize; 3]) {
    let configuration = processor.tile_configuration;
    let shape = read_shape(&configuration, destination);
    let depth = read_shape(&configuration, first).row_bytes / 4;
    let columns = shape.row_bytes / 4;
    let data = processor.tile_data.clone();
    let fused = |a: u64, b: u64, c: u64| multiply_add(a, b, c, false, false, Precision::Single, DOT_CONTROL, NAN_ORDER).bits;
    let widen = |pair: u32, upper: bool| u64::from(if upper { pair & 0xFFFF_0000 } else { pair << 16 });
    for row in 0..shape.rows {
        let mut result = [0u8; ROW_BYTES];
        for column in 0..columns {
            let (mut lower, mut upper) = (0, 0);
            for inner in 0..depth {
                let a = read_dword(&data, first, row, inner);
                let b = read_dword(&data, second, inner, column);
                lower = fused(widen(a, false), widen(b, false), lower);
                upper = fused(widen(a, true), widen(b, true), upper);
            }
            let addend = u64::from(read_dword(&data, destination, row, column));
            let pair = if is_active(Bug::TileBfloat16PairsSummedIntoDestination) {
                fused(upper, SINGLE_ONE, fused(lower, SINGLE_ONE, addend))
            } else {
                fused(addend, SINGLE_ONE, fused(lower, SINGLE_ONE, upper))
            };
            result[4 * column..4 * column + 4].copy_from_slice(&(pair as u32).to_le_bytes());
        }
        processor.tile_data.write_row(destination, row).copy_from_slice(&result);
    }
    for row in shape.rows..TILE_ROWS {
        processor.tile_data.write_row(destination, row).fill(0);
    }
    finish_tile_write(processor);
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::vec;

    use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator};

    use crate::amx::{TILE_COMPONENTS, TILE_DATA_COMPONENT, TILES, TileShape, write_configuration};
    use crate::{
        DataAccess, Exception, Instruction, InstructionFetch, PagePermission, PreciseInstruction, Processor, X86_64, ZERO_PROCESSOR,
    };

    const CODE: u64 = 0x1000;
    /// TILEZERO tmm0.
    const TILE_ZERO: [u8; 5] = [0xC4, 0xE2, 0x7B, 0x49, 0xC0];

    #[test]
    fn every_behavior_of_tilezero_loads_the_error_where_xfd_disables_tiledata() {
        let whole = TileShape { rows: 16, row_bytes: 64 };
        let processor = Processor {
            instruction_pointer: CODE,
            xcr0: ZERO_PROCESSOR.xcr0 | TILE_COMPONENTS,
            xfd: TILE_DATA_COMPONENT,
            tile_configuration: write_configuration([whole; TILES], 0),
            ..ZERO_PROCESSOR
        };
        let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
        for behavior in X86_64::list_known_behaviors(Instruction::TilezeroTmmu32) {
            let state = ExecutionState {
                processor: processor.clone(),
                memory_mutations: vec![
                    MemoryDelta::PermissionAssignment { addresses: CODE..CODE + 0x1000, permission: executable },
                    MemoryDelta::ValueAssignment { start: CODE, bytes: Cow::Borrowed(&TILE_ZERO) },
                ],
            };
            if matches!(behavior, PreciseInstruction::Undefined(_)) {
                continue;
            }
            let Ok(after) = behavior.simulate_state(state);
            assert_eq!(after.processor.exception, Some(Exception::DeviceNotAvailable), "{behavior:?}");
            assert_eq!(after.processor.xfd_err, TILE_DATA_COMPONENT, "{behavior:?}");
            assert_eq!(after.processor.instruction_pointer, CODE, "{behavior:?}");
        }
    }
}
