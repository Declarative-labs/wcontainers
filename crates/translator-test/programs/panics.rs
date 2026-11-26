// Rust panics: caught at every depth with their messages, unwinding drops on the way. The C library calls
// `entry` as `main` (named so by the build), and the standard library allocates each panic's message and
// exception through it, from pages the stand-in kernel maps.

use std::cell::Cell;
use std::panic;

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

// The program, called by the C library with the arguments' count and values.
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
