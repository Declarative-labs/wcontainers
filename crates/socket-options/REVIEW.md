# The socket options primitive, for review

## The trait (`interface.rs`)

```rust
pub trait SocketOptions {
    fn set_option(&self, socket: BorrowedFd<'_>, option: PlainOption, value: &[u8]) -> Result<(), Errno>;
    fn get_option(&self, socket: BorrowedFd<'_>, option: PlainOption, into: &mut [u8]) -> Result<usize, Errno>;
    fn count_queued(&self, socket: BorrowedFd<'_>, count: QueuedCount) -> Result<c_int, Errno>;
    fn get_authentication_key(&self, socket: BorrowedFd<'_>, query: &mut [u8; AUTHENTICATION_KEY_QUERY]) -> Result<(), Errno>;
    fn set_source_filter(&self, socket: BorrowedFd<'_>, filter: &SourceFilterOptions<'_>) -> Result<(), Errno>;
    fn get_multipath_full_information<const CONNECTION: usize, Subflow: FromBytes, Information: FromBytes>(
        &self, socket: BorrowedFd<'_>, connection: &mut [u8; CONNECTION],
        subflows: &mut [Subflow], information: &mut [Information]) -> Result<MultipathCounts, Errno>;
}
```

`PlainOption` is a closed list of 81 options: 51 4-byte integers, 7 only read, 15 structures, 7 byte strings, 1 64-bit number.

## The three unsafe lines (`implementation/linux/implementation.rs`)

```rust
unsafe { foreign::setsockopt(socket, level, name, value.address(0), length) }
unsafe { foreign::getsockopt(socket, level, name, into.address(0), &mut written) }
unsafe { foreign::ioctl(socket, request as libc::Ioctl, into.address(0)) }
```

Each is sound for the same reason: the only address the kernel gets is the start of a region of the primitive's own scratch file, of the value's length, with no Rust reference over it. Whatever the descriptor is by the time of the call, the kernel's accesses within 4 GiB of that address land in the scratch file or fail with `EFAULT`. `written` is a local `socklen_t`. The bytes in a region hold no address the kernel follows: a read is given zeros, a set gets a value of its entry's kind, and the multipath request's two addresses point into the same region.

## How the guard works (`implementation/linux/scratch.rs`)

1. Each thread has a sealed memfd of 4 GiB, then the data, mapped with memmap2's safe `map_raw` plus 4 GiB past the file's end.
2. No page backs the part past the end, so any access there fails with `EFAULT`.
3. A region is the last bytes of the data, so going forward from it the kernel finds only the region, then the guard.
4. The 4 GiB before the data are never written, so a reach back lands in the same file (new this round).
5. A region is cleared by punching it out of the file, and bytes go in and out with `pwrite` and `pread`.

## What is contained by construction

- Overruns and over-reads past the length, at any length: lengths over `u32` are refused, and `pwrite` stops at 2 GiB.
- Reaches back from the address, up to 4 GiB.
- A swapped descriptor's memory access: the region is the same whatever the descriptor now is.
- A read cannot carry an address: the kernel is given zeros.

## What rests on review

- The 23 entries whose value is a structure, a byte string or a 64-bit number. Each carries `read_in(kernel, file, function, "reads N bytes, follows no address")`, read at v7.0. A wrong kind lets the kernel follow an address the caller wrote, as the third check showed.
- The kernel acting on a swapped descriptor in ways other than memory, for example setting an option on another socket of the process.

## Assumptions, each written where it applies

- No handler reaches 4 GiB or more past the address (`scratch.rs`, `GUARD`).
- No handler reaches back from it: now true by construction up to 4 GiB (`scratch.rs`, `BEFORE`).
- Every handler a swapped descriptor can reach at a listed level and name reads the declared kind (`set_through`).
- Drivers treat the ioctl argument as user memory within 4 GiB of the address, or as a plain number (`ioctl_through`).

## What each check covers (x86-64, kernel 7.0.0-1018-aws, as root)

- `tests/addresses.rs`, 2 tests: no structure on the list has a field of 8 bytes or more except 6 named ones, and each of the 23 entries carries its record with the right byte count.
- Conformance suite, 11 tests, about 40 s: every option in the state that fills it, and 375,429,130 reads at every length with 0 bytes outside.
- `tests/attacks.rs`, 13 tests, about 5 s: the first two checks' attacks.
- `tests/containment.rs`, 4 tests, about 18 s: kernel page faults traced while every method runs at scratch sizes from 4 KiB to 1 MiB and at lengths up to 2^32; addresses planted at every 4-byte offset of every settable value in five bases, including a leading count of 1, on 11 sockets (108,482 calls); fd 0 swapped across 70 kinds of descriptor (20,000 calls each); and a check that a region is lent as zeros.
- Kani, 7 harnesses, and Miri, 3 tests: the region given to the kernel is exactly the value's length, and the built requests are right. Both run against a model of the scratch, so the guard itself is covered only by the suites.

## What three rounds of independent checks found

1. Two listed options, `TCP_AO_GET_KEYS` and `MCAST_MSFILTER`, took a count from inside the value and overflowed the heap from safe code. Closed by removing them and adding typed methods that set the count themselves.
2. A raw socket passed as TCP, and a descriptor swapped after the check let the kernel read 40 bytes out of a 4-byte value. Closed by the scratch and its guard: the kernel never sees the caller's memory, and the descriptor check became a courtesy.
3. The list and the guard held, but a wrong entry whose value carries an address let the kernel copy a program out of our memory. This round adds the records and their test, puts the third check's tests in the suite, and writes down the assumptions. With the wrong entry added under a false record, the record test passes and the suite's sweep fails with 20 follows.

## The question for you

Which options may a guest reach at all? The guest never names an option; the host socket crates map its typed settings to entries, and each of the 81 is named at some call site there. So the list is as wide as the settings the host offers, and dropping a setting is what would shorten it.
