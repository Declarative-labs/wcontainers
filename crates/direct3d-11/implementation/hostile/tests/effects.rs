//! The effects judgments (src/api.rs `effects`), tested. Each judgment says what none of the methods of a group of
//! interfaces (or exports) can do to a fact the marks rely on. For each group, every method of it is called through both
//! glues on an object of every kind, after a buffer's memory was lent by Map, and then every fact a test can see is
//! checked: every call returned (no exception, no callback ran anything that did not return); no method a row keeps
//! from the system reached the implementation (protection, the device lock, deferred contexts, discards, the reference
//! rasterizer, mapped surfaces and the rest); the memory lent is intact unless the group's own Unmap ended it, and then
//! it reads and writes nothing; the objects held before still answer as they did.

use std::time::Duration;

use wcontainers_direct3d_11::enumerations::{Map, Usage};
use wcontainers_direct3d_11::structures::BufferDesc;
use wcontainers_direct3d_11::{Buffer as _, Device as _, DeviceContext as _, ResourceRef};
use wcontainers_direct3d_11_hostile::{
    self as hostile, ANSWERED, EFFECTS, LyingBytes, NEVER_REACHED, Objects, drive_where, make_all,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// How long one group's drive may take before it counts as a call that never returned.
const DRIVE_WITHIN: Duration = Duration::from_secs(120);

/// The bytes a write-discard Map lends of the buffer.
const LENT_BYTES: usize = 4096;

/// How many times Map is asked before the test gives up.
const MAP_TRIES: usize = 64;

/// D3D11_MAP_WRITE_DISCARD.
const WRITE_DISCARD: Map = Map(4);

fn honest() -> LyingBytes {
    LyingBytes::lengths(&[64])
}

/// A dynamic buffer the CPU writes, of the bytes `LENT_BYTES` says.
fn buffer_desc() -> BufferDesc {
    BufferDesc {
        byte_width: 4096,
        usage: Usage(2),
        bind_flags: 1,
        cpu_access_flags: 0x1_0000,
        misc_flags: 0,
        structure_byte_stride: 0,
    }
}

/// Ends the process when `body` takes longer than `limit`: a call that never returns must not pass for a slow one.
fn within<R>(what: &str, limit: Duration, body: impl FnOnce() -> R) -> R {
    let (done, finished) = std::sync::mpsc::channel::<()>();
    let what = what.to_string();
    let watchdog = std::thread::spawn(move || {
        if finished.recv_timeout(limit).is_err() {
            println!("EFFECTS HANG: {what} did not end within {limit:?}");
            std::process::exit(3);
        }
    });
    let answer = body();
    let _ = done.send(());
    let _ = watchdog.join();
    answer
}

#[test]
fn every_group_of_interfaces_keeps_the_facts_the_marks_rely_on() {
    hostile::plan(None);
    let mut checked = 0;
    for (interfaces, why) in EFFECTS {
        let mut objects = Objects::new();
        make_all(&mut objects);
        let (Some(device), Some(context)) =
            (objects.device().cloned(), objects.device_context().cloned())
        else {
            panic!("no device or context to drive {interfaces:?} on");
        };
        let buffer = device
            .create_buffer(&buffer_desc(), None)
            .expect("a dynamic buffer");
        objects.put_buffer(buffer.clone());
        make_all(&mut objects);
        // The hostile implementation's Map answers lying pitches and lengths, which the glue refuses: asked until it
        // answers honestly.
        let lent = (0..MAP_TRIES)
            .find_map(|_| {
                context
                    .map(ResourceRef::Buffer(&buffer), 0, WRITE_DISCARD, 0)
                    .ok()
            })
            .expect("the buffer mapped");
        // The glue lends the description's bytes raised to the pitches the system answered (lengths.rs).
        let length = usize::try_from(lent.data.len()).unwrap_or(0);
        assert!(
            length >= LENT_BYTES,
            "the memory lent is shorter than the buffer"
        );
        let desc_before = buffer.get_desc();
        let reached_before: Vec<u64> = NEVER_REACHED
            .iter()
            .map(|place| hostile::reached(place))
            .collect();
        let otherwise_before: Vec<u64> = ANSWERED
            .iter()
            .map(|(place, _)| hostile::reached_otherwise(place))
            .collect();
        let unmaps_before = hostile::reached("ID3D11DeviceContext::Unmap");
        let wanted = |interface: &str| interfaces.contains(&interface);
        // The drive's own panics (a method the calling glue does not carry) are counted, not printed.
        let shown = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let driven = within(&format!("{interfaces:?}"), DRIVE_WITHIN, || {
            drive_where(&objects, &honest, &wanted)
        });
        std::panic::set_hook(shown);
        // Every call returned to its caller: none panicked back (a callback or an exception would have).
        assert_eq!(driven.panicked, 0, "{interfaces:?}: a call panicked back");
        // No method a row keeps from the system reached the implementation.
        for (place, before) in NEVER_REACHED.iter().zip(&reached_before) {
            assert_eq!(
                hostile::reached(place),
                *before,
                "{interfaces:?} reached {place}, which a row keeps from the system"
            );
        }
        // A method a row answers reached the implementation only with the answer's own arguments (the glue's own call).
        for ((place, value), before) in ANSWERED.iter().zip(&otherwise_before) {
            assert_eq!(
                hostile::reached_otherwise(place),
                *before,
                "{interfaces:?} reached {place} with other than {value}, which a row answers"
            );
        }
        // The memory lent lives until the glue's own Unmap: intact, or, after the group's Unmap, reaching nothing.
        let pattern = vec![0x3c_u8; length];
        let written = lent.data.write(&pattern);
        let mut back = vec![0_u8; length];
        let read = lent.data.read(&mut back);
        if hostile::reached("ID3D11DeviceContext::Unmap") == unmaps_before {
            assert_eq!(
                (written, read),
                (length, length),
                "{interfaces:?}: the memory lent was ended by no Unmap"
            );
            assert_eq!(
                back, pattern,
                "{interfaces:?}: the memory lent changed under its view"
            );
        } else {
            assert!(
                written == 0 || written == length,
                "{interfaces:?}: the memory lent was cut short"
            );
        }
        // The objects held before still answer as they did.
        assert_eq!(
            buffer.get_desc(),
            desc_before,
            "{interfaces:?}: the buffer's description changed"
        );
        context.unmap(ResourceRef::Buffer(&buffer), 0);
        println!(
            "EFFECTS {interfaces:?}: called {}, skipped {}; facts held ({why})",
            driven.called, driven.skipped
        );
        checked += 1;
        drop(lent);
        drop(objects);
    }
    assert_eq!(checked, EFFECTS.len());
}
