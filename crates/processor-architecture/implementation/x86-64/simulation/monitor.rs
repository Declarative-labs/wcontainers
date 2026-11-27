//! UMONITOR as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (vol. 2, UMONITOR) gives it, which this chip does not follow: the address the register holds raises #GP(0) when it
//! is not canonical and the page fault of a read of one byte otherwise.

use iced_x86::{Code, Instruction as Decoded};

use super::{Memory, MemoryOperand, check_access, expect_code, read_register};
use crate::{Access, Exception, Processor};

pub(super) fn check_monitored_address(
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    expect_code(decoded, &[Code::Umonitor_r32, Code::Umonitor_r64]);
    let operand = MemoryOperand { address: read_register(processor, decoded.op0_register()), stack: false };
    check_access(processor.flags, memory, operand, 1, Access::Read)
}
