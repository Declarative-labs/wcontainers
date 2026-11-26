//! What an exception taken from translated code comes to for the guest (docs/native.md section 4): the
//! virtual machine's exception syndrome read into what [`wcontainers_executor_arm64_stop`] resolves in every way.
//! The inside resolves its guest threads' stops with this, and the tests that run pieces on the floor resolve
//! theirs with the same code.

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_arm64::Way;
use wcontainers_arm64::thread_state::RegisterPage;
pub use wcontainers_executor_arm64_stop::*;
use wcontainers_processor_architecture_arm64::syndrome::WRITE_NOT_READ;

use crate::syndrome::{ALIGNMENT_FAULT, CACHE_MAINTENANCE, FAULT_STATUS};

/// The exception that does not resolve: one translated code never raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unexpected {
    pub syndrome: u64,
    pub fault: u64,
    pub pc: u64,
}

/// One exception taken from translated code at the guest's level.
pub struct Exception<'a> {
    pub syndrome: u64,
    /// FAR_EL1.
    pub fault: u64,
    pub hardware: &'a Interrupted,
    pub page: &'a RegisterPage,
}

/// What the syndrome says the exception was.
fn taken(syndrome: u64, fault: u64) -> Taken {
    let class = ESR_EL1::EC.read(syndrome);
    if class == ESR_EL1::EC::Value::Brk64 as u64 {
        Taken::Call(ESR_EL1::ISS.read(syndrome) as u16)
    } else if class == ESR_EL1::EC::Value::PCAlignmentFault as u64 {
        Taken::JumpAlignment
    } else if class == ESR_EL1::EC::Value::SPAlignmentFault as u64 {
        Taken::StackAlignment
    } else if class == ESR_EL1::EC::Value::DataAbortLowerEL as u64 {
        Taken::Access {
            alignment: ESR_EL1::ISS.read(syndrome) & FAULT_STATUS == ALIGNMENT_FAULT,
            fault,
            write: syndrome & (WRITE_NOT_READ | CACHE_MAINTENANCE) == WRITE_NOT_READ,
        }
    } else {
        Taken::Other
    }
}

/// What an exception taken from `piece` comes to. `memory` reads guest memory (for a jump's fix-ups).
pub fn resolve(
    piece: &Piece<'_>,
    exception: &Exception<'_>,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Result<Resolved, Unexpected> {
    let h = exception.hardware;
    let taken = taken(exception.syndrome, exception.fault);
    wcontainers_executor_arm64_stop::resolve(
        Way::Machine,
        piece,
        taken,
        h,
        exception.page,
        float,
        memory,
    )
    .ok_or(Unexpected {
        syndrome: exception.syndrome,
        fault: exception.fault,
        pc: h.pc,
    })
}

/// The guest's state at an interrupt that landed at the hardware's pc while `piece` ran (see
/// [`wcontainers_executor_arm64_stop::interrupted`]).
pub fn interrupted(
    piece: &Piece<'_>,
    h: &Interrupted,
    page: &RegisterPage,
    float: &mut FloatStatus,
    memory: &mut dyn FnMut(u64, &mut [u8]),
) -> Option<GuestState> {
    wcontainers_executor_arm64_stop::interrupted(Way::Machine, piece, h, page, float, memory)
}
