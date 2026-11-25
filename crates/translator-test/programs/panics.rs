// Rust panics: caught at every depth with their messages, unwinding drops on the way. The stand-in kernel
// answers only five system calls, so the C library calls `entry` as its `main` (the build names it to the
// linker; the standard library's own start asks for more calls), and the program allocates from a static area
// (the stand-in has no mmap, and the standard library allocates each panic's message and exception).

use std::alloc::{GlobalAlloc, Layout};
use std::cell::{Cell, UnsafeCell};
use std::panic;

struct Area {
    bytes: UnsafeCell<[u8; 1 << 20]>,
    used: Cell<usize>,
}

// One thread only.
unsafe impl Sync for Area {}

unsafe impl GlobalAlloc for Area {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = self.bytes.get() as usize;
        // Aligned as malloc aligns everything (C's max_align_t, 16 bytes like u128): the unwinder writes its
        // exception objects, which Rust allocates with a smaller alignment, with aligned vector stores.
        let align = layout.align().max(align_of::<u128>());
        let start = (base + self.used.get()).next_multiple_of(align);
        let end = start + layout.size();
        if end > base + (1 << 20) {
            return std::ptr::null_mut();
        }
        self.used.set(end - base);
        start as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static AREA: Area = Area {
    bytes: UnsafeCell::new([0; 1 << 20]),
    used: Cell::new(0),
};

thread_local!(static DROPPED: Cell<u32> = const { Cell::new(0) });

struct Guard(u32);

impl Drop for Guard {
    fn drop(&mut self) {
        DROPPED.with(|d| d.set(d.get() + self.0));
    }
}

#[inline(never)]
fn dive(n: u32) -> u32 {
    let _guard = Guard(n);
    if n == 0 {
        panic!("bottom {}", std::hint::black_box(n));
    }
    dive(n - 1) + 1
}

/// The program, called by the C library with the arguments' count and values.
pub extern "C" fn entry(_: i32, _: *const *const u8) -> i32 {
    panic::set_hook(Box::new(|_| {}));
    let mut caught = 0;
    for i in 0..32 {
        match panic::catch_unwind(|| dive(i % 9)) {
            Ok(v) => println!("returned {v}"),
            Err(e) => {
                caught += 1;
                let message = e.downcast_ref::<String>().cloned().unwrap_or_default();
                println!("{message} at {i}");
            }
        }
    }
    let dropped = DROPPED.with(Cell::get);
    println!("caught {caught}, dropped {dropped}");
    0
}
