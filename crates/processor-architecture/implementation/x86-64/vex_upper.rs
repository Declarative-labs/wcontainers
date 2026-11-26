//! What every VEX form does above bit 255: a write of an XMM or YMM register zeroes the register up to the widest
//! vector, ZMM's bits 256 to 511, and VZEROUPPER and VZEROALL reach ZMM0 to ZMM15 alone, as the
//! [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) volume 1, section
//! 15.5, has it. No VEX form reads those bits or names ZMM16 to ZMM31.

use iced_x86::{Code, EncodingKind, Instruction as Decoded, InstructionInfoFactory, OpAccess};

use crate::planted::{Bug, is_active};
use crate::{Processor, VECTOR_REGISTERS};

/// Zeroes bits 256 to 511 of each vector register a VEX instruction writes; leaves any other instruction alone.
pub(super) fn zero_written_upper_bits(processor: &mut Processor, decoded: &Decoded) {
    if decoded.encoding() != EncodingKind::VEX || is_active(Bug::VexKeepsZmmUpperBits) {
        return;
    }
    match decoded.code() {
        Code::VEX_Vzeroupper if is_active(Bug::ZeroUpperKeepsZmmUpperBits) => {}
        Code::VEX_Vzeroupper | Code::VEX_Vzeroall => {
            processor.zmm_upper_halves = [[0; 2]; VECTOR_REGISTERS];
            if is_active(Bug::ZeroAllClearsHighRegisters) {
                processor.high_vector_registers = [[0; 4]; VECTOR_REGISTERS];
            }
        }
        _ => {
            let mut factory = InstructionInfoFactory::new();
            let used_registers = factory.info(decoded).used_registers();
            let is_vector = |register: iced_x86::Register| register.is_xmm() || register.is_ymm() || register.is_zmm();
            let read = |number: usize| {
                used_registers.iter().any(|used| {
                    is_vector(used.register())
                        && used.register().number() == number
                        && matches!(used.access(), OpAccess::Read | OpAccess::ReadWrite | OpAccess::CondRead | OpAccess::ReadCondWrite)
                })
            };
            for used in used_registers {
                let register = used.register();
                let written = matches!(used.access(), OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite);
                if is_active(Bug::VexReadDestinationKeepsZmmUpperBits) && written && read(register.number()) {
                    continue;
                }
                if written && is_vector(register) && register.number() < VECTOR_REGISTERS {
                    processor.zmm_upper_halves[register.number()] = [0; 2];
                }
            }
        }
    }
}

/// Zeroes bits 256 to 511 of the mask of a VEX gather that faults, which writes its mask whole and leaves its
/// destination's bits above the vector, as this chip does.
pub(super) fn zero_gather_mask_upper_bits(processor: &mut Processor, decoded: &Decoded) {
    if !is_active(Bug::VexKeepsZmmUpperBits) {
        processor.zmm_upper_halves[decoded.op2_register().number()] = [0; 2];
    }
}
