//! The inside's float state routines and its whole TLB invalidation, made by the encoder and written into the frame the inside names.

use wcontainers_abi::PAGE_SIZE;
use wcontainers_arm64::carry::LINK;
use wcontainers_arm64::encode::{self, FloatStatusMoves, Register, form};
use wcontainers_executor_hardware_virtualization_inside::link::Link;
use wcontainers_executor_hardware_virtualization_inside::routines::{Placed, Routines, Vectors};

use crate::Memory;

/// The registers the routines take their arguments in and give their results in, as the C calling convention
/// has them.
const FIRST: Register = 0;
const SECOND: Register = 1;

/// Bytes of one vector register.
const VECTOR_BYTES: u64 = size_of::<[u64; 2]>() as u64;

/// How many routines there are: one start each in [`Placed`].
const ROUTINES: usize = size_of::<Placed>() / size_of::<u64>();

/// The routines' words and where each starts, in words, in [`Placed`]'s order.
fn routines() -> (Vec<u32>, [usize; ROUTINES]) {
    let first = FloatStatusMoves::with(FIRST);
    let second = FloatStatusMoves::with(SECOND);
    let ret = encode::branch_register(form::RET, LINK);
    let vectors = Vectors::default().len() as u8;
    let each = |name: encode::Form| -> Vec<u32> {
        (0..vectors)
            .map(|v| {
                encode::load_store(name, v, FIRST, u64::from(v) * VECTOR_BYTES, VECTOR_BYTES)
                    .expect("every vector register's place fits the offset")
            })
            .chain([ret])
            .collect()
    };
    let parts = [
        vec![first.read_fpsr, second.read_fpcr, ret],
        vec![first.write_fpsr, second.write_fpcr, ret],
        each(form::STR_Q),
        each(form::LDR_Q),
        vec![encode::invalidate_whole_tlb(), ret],
    ];
    let mut words = Vec::new();
    let mut starts = [0; ROUTINES];
    for (start, part) in starts.iter_mut().zip(parts) {
        *start = words.len();
        words.extend(part);
    }
    (words, starts)
}

/// Answers a [`Routines`] request: writes the routines into guest memory and says where each starts.
pub fn answer(link: &mut Link, memory: &Memory) {
    let request: Routines = link.get();
    let (words, starts) = routines();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    assert!(bytes.len() as u64 <= PAGE_SIZE, "the routines fit one frame");
    let host = memory.host_bytes(request.at, bytes.len() as u64);
    // SAFETY: the bytes lie in guest memory, in the frame the inside named for them.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), host, bytes.len()) };
    let [
        read_float_control,
        write_float_control,
        save_vectors,
        load_vectors,
        invalidate_whole_tlb,
    ] = starts.map(|start| (start * size_of::<u32>()) as u64);
    link.put(&Placed {
        read_float_control,
        write_float_control,
        save_vectors,
        load_vectors,
        invalidate_whole_tlb,
    });
}

#[cfg(test)]
mod tests {
    use wcontainers_arm64::encode::FloatStatusMoves;

    #[test]
    fn the_moves_are_the_float_status_registers() {
        let moves = FloatStatusMoves::with(super::FIRST);
        // mrs x0, fpsr; mrs x0, fpcr; msr fpsr, x0; msr fpcr, x0
        assert_eq!(moves.read_fpsr, 0xd53b_4420);
        assert_eq!(moves.read_fpcr, 0xd53b_4400);
        assert_eq!(moves.write_fpsr, 0xd51b_4420);
        assert_eq!(moves.write_fpcr, 0xd51b_4400);
    }

    #[test]
    fn the_invalidation_is_tlbi_vmalle1is() {
        assert_eq!(
            wcontainers_arm64::encode::invalidate_whole_tlb(),
            0xd508_831f
        );
    }

    #[test]
    fn the_routines_fit_a_page() {
        let (words, _) = super::routines();
        assert!((words.len() * size_of::<u32>()) as u64 <= wcontainers_abi::PAGE_SIZE);
    }
}
