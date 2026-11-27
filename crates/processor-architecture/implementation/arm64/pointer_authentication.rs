//! Pointer authentication at EL0 as the executor's guest configures it: SCTLR_EL1's EnIA, EnIB, EnDA and EnDB clear, so
//! that every instruction that adds or checks a code leaves its pointer as it is, as [AddPAC](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-pac#:~:text=AddPACIA)
//! and Auth return it when the key is not enabled; the generic key's PACGA, which no enable bit covers, traps to the
//! hypervisor, which makes it undefined; and XPACI and XPACD strip the code whatever the keys. XPACLRI, which strips
//! the link register, is left out: the generated states hold zero there, which no strip changes.

use core::convert::Infallible;

use crate::encoding::RD;
use crate::integer::low_ones;
use crate::operand::RegisterThirtyOne;
use crate::registers::{read_register, write_register};
use crate::{Instruction, Processor, VIRTUAL_ADDRESS_BITS};

/// The bit that tells an upper-half address from a lower-half one, which a stripped code is made of copies of.
const HALF_BIT: u32 = 55;
/// Where the top byte starts, which TBI0 leaves out of a lower-half address.
const TOP_BYTE_SHIFT: u32 = u64::BITS - u8::BITS;

/// What one pointer authentication instruction does with its keys off.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerAuthenticationOperation {
    /// Adds or checks a code: with the key off, nothing changes.
    KeyDisabled,
    /// Strips the code from the destination, alike for a data address (XPACD) and an instruction address (XPACI).
    Strip,
    /// PACGA, which the hypervisor traps.
    Undefined,
}

/// The operation of each modeled pointer authentication instruction, none for every other instruction.
pub const fn to_pointer_authentication_operation(
    instruction: Instruction<Infallible>,
) -> Option<PointerAuthenticationOperation> {
    use Instruction as I;
    use PointerAuthenticationOperation as O;
    Some(match instruction {
        I::Pacia
        | I::Pacib
        | I::Pacda
        | I::Pacdb
        | I::Paciza
        | I::Pacizb
        | I::Pacdza
        | I::Pacdzb
        | I::Autia
        | I::Autib
        | I::Autda
        | I::Autdb
        | I::Autiza
        | I::Autizb
        | I::Autdza
        | I::Autdzb
        | I::Pacia1716
        | I::Pacib1716
        | I::Autia1716
        | I::Autib1716
        | I::Paciaz
        | I::Paciasp
        | I::Pacibz
        | I::Pacibsp
        | I::Autiaz
        | I::Autiasp
        | I::Autibz
        | I::Autibsp => O::KeyDisabled,
        I::Xpaci | I::Xpacd => O::Strip,
        I::Pacga => O::Undefined,
        _ => return None,
    })
}

/// [Strip](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-pac#:~:text=Strip):
/// the code bits, from the address's 48th up, become copies of bit 55, but the top byte of a lower-half address,
/// which TBI0 leaves out of the address, stays; TBI1 is off, and TBID0 too, so instruction and data addresses alike.
pub fn strip(pointer: u64) -> u64 {
    let address = pointer & low_ones(VIRTUAL_ADDRESS_BITS);
    if crate::planted::is(477) && (pointer >> HALF_BIT) & 1 == 0 {
        return pointer;
    }
    if crate::planted::is(478) && (pointer >> HALF_BIT) & 1 != 0 {
        return (pointer & !low_ones(TOP_BYTE_SHIFT))
            | low_ones(TOP_BYTE_SHIFT) & !low_ones(VIRTUAL_ADDRESS_BITS)
            | address;
    }
    if (pointer >> HALF_BIT) & 1 != 0 {
        !low_ones(VIRTUAL_ADDRESS_BITS) | address
    } else {
        (pointer & !low_ones(TOP_BYTE_SHIFT)) | address
    }
}

pub fn simulate_pointer_authentication<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: PointerAuthenticationOperation,
) -> Result<(), crate::Exception> {
    use PointerAuthenticationOperation as O;
    let number = match operation {
        O::KeyDisabled if crate::planted::is(479) => RD.read(word),
        O::KeyDisabled => return Ok(()),
        O::Undefined if crate::planted::is(480) => {
            write_register(processor, RD.read(word), 0, RegisterThirtyOne::ZeroRegister);
            return Ok(());
        }
        O::Undefined => return Err(crate::Exception::UndefinedInstruction),
        O::Strip => RD.read(word),
    };
    let pointer = read_register(processor, number, RegisterThirtyOne::ZeroRegister);
    write_register(
        processor,
        number,
        strip(pointer),
        RegisterThirtyOne::ZeroRegister,
    );
    Ok(())
}
