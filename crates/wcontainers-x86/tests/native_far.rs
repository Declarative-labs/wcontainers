//! The inside's answer to a near transfer whose target is at or above 2^47 (docs/x86-native-output.md section
//! 3.3), from code at the top of user space, where a rel32 or rel8 reaches past it: the guest's 4-level rule as
//! the reference applies it: general protection leaving the registers as they were, a call's return address
//! written first (the Xeon's `call rax` to 2^63 writes it, as the meaning does).

use wcontainers_abi::PAGE_SIZE;
use wcontainers_x86::answer::{
    Consts, Fault, Memory, NearTransfer, Question, answer, near_transfer,
};
use wcontainers_x86::native::{far_transfer, question};
use wcontainers_x86::{
    ARITHMETIC_FLAGS, Offer, RCX, RSP, USER_END, USER_TOP, V3, X86Block, decode, is_canonical,
};
use zerocopy::FromZeros;

/// One page of stack below `STACK`, the only memory there is.
const STACK: u64 = USER_TOP;
const STACK_BYTES: usize = PAGE_SIZE as usize;
/// Where the instructions are: the last bytes below the end of user space.
const AT: u64 = USER_END - 0x40;
/// A target in the upper canonical half, and code near zero whose rel32 reaches it.
const UPPER: u64 = !(USER_END - 1) + PAGE_SIZE;
const LOW: u64 = PAGE_SIZE;
const LOW_REACHES: u64 = 0u64.wrapping_sub(PAGE_SIZE);

struct Stack(Vec<u8>);

impl Stack {
    fn slot(&mut self, at: u64, bytes: usize) -> Option<&mut [u8]> {
        let from = usize::try_from(at.checked_sub(STACK - STACK_BYTES as u64)?).ok()?;
        self.0.get_mut(from..from.checked_add(bytes)?)
    }
}

impl Memory for Stack {
    fn read(&mut self, at: u64, into: &mut [u8]) -> Result<(), u64> {
        into.copy_from_slice(self.slot(at, into.len()).ok_or(at)?);
        Ok(())
    }

    fn write(&mut self, at: u64, bytes: &[u8]) -> Result<(), u64> {
        self.slot(at, bytes.len()).ok_or(at)?.copy_from_slice(bytes);
        Ok(())
    }

    fn check(&mut self, at: u64, bytes: u64, _: bool) -> Result<(), u64> {
        self.slot(at, bytes as usize).map(|_| ()).ok_or(at)
    }
}

impl wcontainers_x86::answer::Clock for Stack {
    fn nanoseconds(&mut self) -> u64 {
        0
    }
}

fn block(rcx: u64, flags: u64, rsp: u64) -> X86Block {
    let mut block = X86Block::new_zeroed();
    block.rip = AT;
    block.gpr[usize::from(RCX)] = rcx;
    block.gpr[usize::from(RSP)] = rsp;
    block.flags = (flags & ARITHMETIC_FLAGS) as u32;
    block
}

/// The instruction `code` at `AT` answered on `start`: the result, the block after and the stack after.
fn answered(code: &[u8], start: X86Block) -> (Result<bool, Fault>, X86Block, Stack) {
    let mut bytes = code.to_vec();
    bytes.resize(16, 0);
    let (i, _) = decode(&bytes).expect("decodes");
    let q = question(&i).expect("a question for a direct transfer");
    assert!(
        matches!(q, Question::NearBranch { .. }),
        "{code:02x?} gives {q:?}"
    );
    let mut block = start;
    let mut stack = Stack(vec![0; STACK_BYTES]);
    let mut constants = Consts::new().expect("the constants cache");
    let offer = Offer::new(V3).expect("the v3 offer");
    let result = answer(q, &mut block, offer, &mut constants, &mut stack).map(|n| n.transferred);
    (result, block, stack)
}

/// `code` at `AT` with a rel32 reaching `target`.
fn rel32(opcode: &[u8], target: u64) -> Vec<u8> {
    rel32_at(AT, opcode, target)
}

/// `code` at `from` with a rel32 reaching `target`.
fn rel32_at(from: u64, opcode: &[u8], target: u64) -> Vec<u8> {
    let end = from + (opcode.len() + wcontainers_x86::carry::REL32) as u64;
    [opcode, &(target.wrapping_sub(end) as u32).to_le_bytes()].concat()
}

#[test]
fn a_direct_transfer_past_user_space_is_general_protection_and_changes_nothing() {
    let start = block(5, ARITHMETIC_FLAGS, STACK);
    for code in [
        rel32(&[0xe9], USER_END),
        rel32(&[0xe8], USER_END + 0x10),
        rel32(&[0x0f, 0x84], USER_END),
        vec![0xeb, 0x7f],
        vec![0xe2, 0x7f],
        vec![0xe1, 0x7f],
        vec![0x67, 0xe3, 0x7f],
    ] {
        let mut start = start;
        if code.ends_with(&[0xe3, 0x7f]) {
            // jecxz: ecx zero, rcx not.
            start.gpr[usize::from(RCX)] = 1 << u32::BITS;
        }
        let (result, after, mut stack) = answered(&code, start);
        assert_eq!(result, Err(Fault::GeneralProtection), "{code:02x?}");
        assert_eq!(after, start, "{code:02x?} changed the block");
        let mut slot = [0; 8];
        stack.read(STACK - 8, &mut slot).expect("the slot");
        let pushed = if code[0] == 0xe8 {
            AT + code.len() as u64
        } else {
            0
        };
        assert_eq!(u64::from_le_bytes(slot), pushed, "{code:02x?}'s stack");
    }
}

#[test]
fn a_direct_transfer_to_the_upper_half_is_made_and_rip_is_its_target() {
    let mut start = block(5, 0, STACK);
    start.rip = LOW;
    let call = rel32_at(LOW, &[0xe8], LOW_REACHES);
    let (result, after, mut stack) = answered(&call, start);
    assert_eq!(result, Ok(true));
    assert_eq!(after.rip, LOW_REACHES);
    let pushed = STACK - size_of::<u64>() as u64;
    assert_eq!(after.gpr[usize::from(RSP)], pushed);
    let mut back = [0; 8];
    stack.read(pushed, &mut back).expect("the slot");
    assert_eq!(u64::from_le_bytes(back), LOW + call.len() as u64);
    // A call whose push cannot be made faults there, changing nothing.
    let mut unmapped = block(5, 0, STACK - STACK_BYTES as u64);
    unmapped.rip = LOW;
    let (result, after, _) = answered(&call, unmapped);
    assert_eq!(
        result,
        Err(Fault::Memory(unmapped.gpr[usize::from(RSP)] - 8))
    );
    assert_eq!(after, unmapped);
}

#[test]
fn a_branch_not_taken_goes_on_and_a_loop_counts() {
    // je with ZF clear, loop with rcx 1, loope with ZF clear, jrcxz with rcx not zero: not taken.
    for (code, rcx, flags, left) in [
        (rel32(&[0x0f, 0x84], USER_END), 5, 0, 5),
        (vec![0xe2, 0x7f], 1, 0, 0),
        (vec![0xe1, 0x7f], 5, 0, 4),
        (vec![0x67, 0xe2, 0x7f], 1 << u32::BITS | 1, 0, 0),
        (vec![0xe3, 0x7f], 1 << u32::BITS, 0, 1 << u32::BITS),
    ] {
        let (result, after, _) = answered(&code, block(rcx, flags, STACK));
        assert_eq!(result, Ok(false), "{code:02x?}");
        assert_eq!(after.gpr[usize::from(RCX)], left, "{code:02x?}");
        assert_eq!(after.rip, AT, "{code:02x?}: the caller moves rip past it");
    }
    // loopne with ZF clear and rcx 2 to the upper half, from code near zero: taken, the count taken with it.
    let mut start = block(2, 0, STACK);
    start.rip = 0x10;
    let mut bytes = vec![0xe0, 0x80];
    bytes.resize(16, 0);
    let (i, _) = decode(&bytes).expect("decodes");
    let mut stack = Stack(vec![0; STACK_BYTES]);
    let mut constants = Consts::new().expect("the constants cache");
    let offer = Offer::new(V3).expect("the v3 offer");
    let q = question(&i).expect("a question");
    let notes = answer(q, &mut start, offer, &mut constants, &mut stack).expect("taken");
    assert!(notes.transferred);
    assert_eq!(start.rip, 0x12u64.wrapping_sub(0x80));
    assert!(is_canonical(start.rip));
    assert_eq!(start.gpr[usize::from(RCX)], 1);
}

#[test]
fn a_far_target_through_a_register_or_the_stack_follows_the_same_rule() {
    let transfer = |hex: &[u8]| {
        let mut bytes = hex.to_vec();
        bytes.resize(16, 0);
        far_transfer(&decode(&bytes).expect("decodes").0).expect("a near transfer through t")
    };
    assert_eq!(transfer(&[0xff, 0xe0]), NearTransfer::Jump);
    assert_eq!(
        transfer(&[0x41, 0xff, 0xd0]),
        NearTransfer::Call { length: 3 }
    );
    assert_eq!(
        transfer(&[0xc2, 0x10, 0x00]),
        NearTransfer::Return { release: 0x10 }
    );
    let start = block(0, 0, STACK - 8);
    for t in [
        transfer(&[0xff, 0xe0]),
        transfer(&[0xff, 0xd0]),
        transfer(&[0xc3]),
    ] {
        let mut after = start;
        let mut stack = Stack(vec![0; STACK_BYTES]);
        assert_eq!(
            near_transfer(t, 1 << 63, &mut after, &mut stack),
            Err(Fault::GeneralProtection)
        );
        assert_eq!(after, start, "{t:?}");
        let mut slot = [0; 8];
        stack.read(STACK - 16, &mut slot).expect("the slot");
        let call = matches!(t, NearTransfer::Call { .. });
        assert_eq!(u64::from_le_bytes(slot) != 0, call, "{t:?}'s stack");
        near_transfer(t, UPPER, &mut after, &mut stack).expect("made");
        assert_eq!(after.rip, UPPER);
        let rsp = after.gpr[usize::from(RSP)];
        match t {
            NearTransfer::Jump => assert_eq!(rsp, STACK - 8),
            NearTransfer::Call { .. } => assert_eq!(rsp, STACK - 16),
            NearTransfer::Return { .. } => assert_eq!(rsp, STACK),
        }
    }
}
