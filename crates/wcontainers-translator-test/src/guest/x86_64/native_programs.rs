//! Layer 2 of the native output (docs/x86-native-output.md section 8): a program's pieces translated for a host
//! process, linked to each other and to the lookup's second level, and run on the reference machine in a blob of
//! their own. The blob maps the program at its own address, the thread area and the lookup's two levels, and
//! points the inside's entries at its own routines: a system call it makes for the guest, recording the count
//! there on standard error, and a miss (an exit or a lookup to a piece not made yet) it records before ending.
//! A run that ends on a miss is made again with that piece. A fault of the guest's ends the blob by the same
//! signal, as on the reference.

use super::native::{
    ALTERNATE_STACK, ALTERNATE_STACK_BYTES, FRAME_GENERAL, GENERAL_REGISTERS, LOAD, LOWER_REGIONS,
    PAGE, PROLOGUE, REGION_BYTES, SECOND_LEVEL, STOP_SIGNALS, assembled, copy, general_register,
    map, number, protect, restorer, system_call,
};
use super::*;
use crate::programs::{Blob, Ran, STACK_TOP};
use crate::progress::{Counted, Position};
use crate::reference::{BlobRequest, Reference};
use iced_x86::IcedError;
use iced_x86::code_asm::{
    AsmRegister64, CodeAssembler, CodeLabel, qword_ptr, r10, r11, r12, r13, r14, rax, rbx, rcx,
    rdi, rdx, rsi, rsp,
};
use linux_raw_sys::general::{SA_ONSTACK, SA_RESTORER, SA_SIGINFO, kernel_sigaction, sigaltstack};
use std::collections::BTreeMap;
use wcontainers_abi::WORD;
use wcontainers_guest_cpu::position::{Resume, Side};
use wcontainers_guest_cpu::{Guest, HostCode, InsideCall};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_x86::area::{
    ENTRIES, FIRST_LEVEL, FIXED, FixedPart, RegisterArea, WITHHELD_VALUES,
};
use wcontainers_x86::carry::{COUNT, T, U, WITHHELD, retarget};
use wcontainers_x86::native::{Target, Way};

/// The blob is loaded at `LOAD`, far from the program and its stack; its parts follow at fixed offsets.
const PROLOGUE_ROOM: u64 = 2 * PAGE;
const HANDLER: u64 = PROLOGUE + PROLOGUE_ROOM;
const RESTORER: u64 = HANDLER + PAGE;
const BLOCK: u64 = RESTORER + PAGE;
/// The pieces, then the routines for the inside's calls, the second level's words, the program's bytes and the
/// thread area, whose first level the prologue maps after the blob.
const PIECES: u64 = BLOCK + PAGE;
/// Pieces start on this boundary, as a loader places them.
const PIECE_ALIGN: usize = 16;
/// The byte between pieces: `int3`.
const BETWEEN_PIECES: u8 = 0xcc;
/// A run made again more often than this, each time for one more piece, is taken as never ending.
const MOST_RUNS: u32 = 10_000;

/// The block's parts: the handler's action, the default action (all zero) and the alternate stack.
const ACTION_AT: u64 = 0;
const DEFAULT_ACTION_AT: u64 = ACTION_AT + size_of::<kernel_sigaction>() as u64;
const ALTERNATE_AT: u64 = DEFAULT_ACTION_AT + size_of::<kernel_sigaction>() as u64;

/// Words the routines keep in the register area's page, after the register area itself: no translated code
/// reaches past it.
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Scratch {
    count: u64,
    rax: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    r11: u64,
    /// The guest's rcx while its call site is found.
    dispatch: u64,
    record: [u64; RECORD_WORDS],
}
const SCRATCH: usize = size_of::<RegisterArea>().next_multiple_of(WORD as usize);
const _: () = assert!(SCRATCH + size_of::<Scratch>() <= FIXED);
const fn scratch(field: usize) -> u64 {
    (SCRATCH + field) as u64
}
const RECORD_WORDS: usize = 3;
const RECORD_BYTES: usize = RECORD_WORDS * WORD as usize;

/// What a record on standard error says, its first word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
enum Said {
    /// A system call made for the guest: the call site's number and the count register there.
    SystemCall = 1,
    /// An exit to a piece not made yet: its return point.
    ExitMiss,
    /// A lookup that found no piece: the target and the host pc (zero).
    LookupMiss,
    /// A call into the inside the blob does not answer: its code and return point.
    OtherCall,
    /// A fault outside the pieces, the blob's own: the host pc and the address.
    OwnFault,
    /// A repeated string instruction run for the guest, which the count takes pass by pass: the call site's
    /// number and rcx before it.
    StringBefore,
    /// After it: rcx.
    StringAfter,
    /// Before it and after it: rsi and rdi.
    StringPointers,
}

impl Said {
    const ALL: [Said; 8] = [
        Said::SystemCall,
        Said::ExitMiss,
        Said::LookupMiss,
        Said::OtherCall,
        Said::OwnFault,
        Said::StringBefore,
        Said::StringAfter,
        Said::StringPointers,
    ];
}

/// How a program ran natively.
#[derive(Debug)]
pub struct NativeRun {
    /// Its output and status, as the reference's are compared.
    pub ran: Ran,
    /// The position after each system call, the count read as the inside reads it.
    pub syscalls: Vec<Position>,
    /// How many times it was run, one more for each piece a miss asked for.
    pub runs: u32,
    pub pieces: usize,
}

/// The program's pieces made so far: none where no instruction starts at the address.
struct Pieces<'a> {
    blob: &'a Blob,
    look_ahead: LookAhead,
    counted: bool,
    target: Target,
    made: BTreeMap<u64, Option<HostCode>>,
}

impl Pieces<'_> {
    /// Makes the piece at `from` and every piece reached from it directly: the targets of its exits and the
    /// return points of its calls.
    fn reach(&mut self, from: u64) {
        let mut work = vec![from];
        while let Some(at) = work.pop() {
            if self.made.contains_key(&at) {
                continue;
            }
            let code = self.translate(at);
            if let Some(code) = &code {
                work.extend(code.exits.iter().map(|e| e.target));
                for span in &code.spans {
                    if let Some((i, length)) = self.guest_instruction(span.guest)
                        && i.iced.mnemonic() == Mnemonic::Call
                    {
                        work.push(span.guest + length as u64);
                    }
                }
            }
            self.made.insert(at, code);
        }
    }

    fn code_from(&self, at: u64) -> Option<&[u8]> {
        let offset = usize::try_from(at.checked_sub(self.blob.load)?).ok()?;
        self.blob.bytes[..self.blob.code_bytes as usize].get(offset..)
    }

    fn guest_instruction(&self, at: u64) -> Option<(wcontainers_x86::Instruction, usize)> {
        <X86_64 as Guest>::decode(self.code_from(at)?).map(|(i, length)| (i, length.into()))
    }

    fn translate(&self, at: u64) -> Option<HostCode> {
        translate_native_for::<X86_64>(
            self.code_from(at)?,
            at,
            self.look_ahead,
            self.counted,
            &Way::Process,
            &self.target,
        )
    }
}

/// A call site of the inside's answer: where it returns and the guest's next pc.
struct Site {
    t: u64,
    next: u64,
    /// How far the count register is ahead of the guest's count there.
    lead: u32,
    answer: Answer,
}

/// The instructions the blob answers for the inside.
enum Answer {
    SystemCall,
    /// A repeated string instruction under the count, run here from its own bytes.
    String {
        bytes: Vec<u8>,
        repeated: wcontainers_x86::strings::Repeated,
    },
}

/// The pieces laid out from `PIECES`, linked, with what the blob's routines need of them.
struct Laid {
    bytes: Vec<u8>,
    /// The second level's words: slot address, piece address.
    slots: Vec<(u64, u64)>,
    entry: u64,
    sites: Vec<Site>,
    /// Each exit's return point, with the guest address it goes to.
    exits: BTreeMap<u64, u64>,
}

fn lay_out(pieces: &Pieces) -> Result<Laid, String> {
    let mut bytes = Vec::new();
    let mut host = BTreeMap::new();
    for (&guest, code) in &pieces.made {
        let Some(code) = code else { continue };
        bytes.resize(bytes.len().next_multiple_of(PIECE_ALIGN), BETWEEN_PIECES);
        host.insert(guest, PIECES + bytes.len() as u64);
        bytes.extend_from_slice(&code.bytes);
    }
    let mut laid = Laid {
        bytes,
        slots: Vec::new(),
        entry: *host
            .get(&pieces.blob.load)
            .ok_or("no piece at the program's start")?,
        sites: Vec::new(),
        exits: BTreeMap::new(),
    };
    for (&guest, code) in &pieces.made {
        let Some(code) = code else { continue };
        if guest >> u32::BITS != 0 {
            return Err(format!("a piece at {guest:#x}, past the first region"));
        }
        let at = host[&guest];
        laid.slots.push((SECOND_LEVEL + WORD * guest, at));
        let offset = (at - PIECES) as usize;
        let piece = &mut laid.bytes[offset..offset + code.bytes.len()];
        for exit in &code.exits {
            let Some(&to) = host.get(&exit.target) else {
                continue;
            };
            let distance = to as i64 - (at + u64::from(exit.start)) as i64;
            let patch = retarget(piece, exit, distance).ok_or("an exit out of reach")?;
            let from = patch.at as usize;
            piece[from..from + patch.written().len()].copy_from_slice(patch.written());
        }
        let piece = laid.bytes[offset..offset + code.bytes.len()].to_vec();
        calls(pieces, code, &piece, at, &mut laid)?;
    }
    Ok(laid)
}

/// Finds the calls into the inside of the piece at host `at`: its system calls and its exits.
fn calls(
    pieces: &Pieces,
    code: &HostCode,
    piece: &[u8],
    at: u64,
    laid: &mut Laid,
) -> Result<(), String> {
    let entries = ENTRIES as u64..(ENTRIES + InsideCall::ALL.len() * WORD as usize) as u64;
    let mut decoder = Decoder::with_ip(wcontainers_x86::MODE_BITS, piece, at, DecoderOptions::NONE);
    for host in decoder.iter() {
        let through_area = host.mnemonic() == Mnemonic::Jmp
            && host.op0_kind() == OpKind::Memory
            && host.memory_segment() == Register::GS
            && host.memory_base() == Register::None
            && entries.contains(&host.memory_displacement64());
        if !through_area {
            continue;
        }
        let kind =
            InsideCall::from_code(((host.memory_displacement64() - entries.start) / WORD) as u16)
                .ok_or("an entry no call has")?;
        let t = host.next_ip();
        let span = code
            .span((t - at - 1) as u32)
            .ok_or("a call outside every span")?;
        match kind {
            InsideCall::Answer => {
                let (i, length) = pieces
                    .guest_instruction(span.guest)
                    .ok_or("an answered call at no instruction")?;
                let m = i.iced.mnemonic();
                let answer = if m == Mnemonic::Syscall {
                    Answer::SystemCall
                } else if i.iced.is_string_instruction()
                    && (i.iced.has_rep_prefix() || i.iced.has_repne_prefix())
                {
                    let bytes = pieces
                        .code_from(span.guest)
                        .and_then(|b| b.get(..length))
                        .ok_or("an instruction past the code")?
                        .to_vec();
                    Answer::String {
                        bytes,
                        repeated: wcontainers_x86::strings::Repeated {
                            kind: wcontainers_x86::strings::StringKind::of(m),
                            element: i.iced.memory_size().size() as u8,
                            address32: i.address32,
                            rflags: 0,
                            count_before: 0,
                            count_after: 0,
                            distance: 0,
                        },
                    }
                } else {
                    return Err(format!(
                        "the inside answers {} at {:#x}, which this blob does not",
                        i.iced, span.guest
                    ));
                };
                laid.sites.push(Site {
                    t,
                    next: span.guest + length as u64,
                    lead: span.lead,
                    answer,
                });
            }
            InsideCall::Exit => {
                let target = match (span.side, span.resume) {
                    (Side::Before, _) => span.guest,
                    (Side::After, Resume::At(target)) => target,
                    (_, resume) => return Err(format!("an exit resuming {resume:?}")),
                };
                laid.exits.insert(t, target);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Runs `blob`'s program natively as [`run`] does and compares it with its run on the CPU, `cpu`, and when
/// `counting` its count after each system call with the WebAssembly output's, `counted`.
pub fn compare(
    reference: &mut Reference,
    blob: &Blob,
    look_ahead: LookAhead,
    counting: bool,
    cpu: &Ran,
    counted: &Counted,
) -> Result<NativeRun, String> {
    let what = format!("natively at {look_ahead:?}, counted {counting}");
    let ours = run(reference, blob, look_ahead, counting).map_err(|e| format!("{what}: {e}"))?;
    if ours.ran != *cpu {
        return Err(format!(
            "{what}: output {:?} status {}, the CPU's {:?} status {}",
            String::from_utf8_lossy(&ours.ran.stdout),
            ours.ran.status,
            String::from_utf8_lossy(&cpu.stdout),
            cpu.status
        ));
    }
    if counting {
        // An ended program's last call is its exit, whose position is the end.
        let (calls, end) = match ours.syscalls.split_last() {
            Some((last, before)) if cpu.status == libc::EXIT_SUCCESS => (before, Some(last.count)),
            _ => (&ours.syscalls[..], None),
        };
        if calls != counted.syscalls {
            let at = calls
                .iter()
                .zip(&counted.syscalls)
                .position(|(a, b)| a != b);
            return Err(format!(
                "{what}: the positions after {} system calls differ from the WebAssembly output's {}, first at call {at:?}: {:?} against {:?}",
                calls.len(),
                counted.syscalls.len(),
                at.and_then(|k| calls.get(k)),
                at.and_then(|k| counted.syscalls.get(k)),
            ));
        }
        if let Some(end) = end
            && end != counted.end
        {
            return Err(format!(
                "{what}: the count at the end is {end}, the WebAssembly output's {}",
                counted.end
            ));
        }
    }
    Ok(ours)
}

/// Runs `blob`'s program natively at `look_ahead`, with the count when `counted`.
pub fn run(
    reference: &mut Reference,
    blob: &Blob,
    look_ahead: LookAhead,
    counted: bool,
) -> Result<NativeRun, String> {
    run_timed(reference, blob, look_ahead, counted, 0).map(|(run, _)| run)
}

/// [`run`], then the blob with every piece it reached run `timed` times more, and the best of their times in
/// seconds, from before the reference makes its process until it has ended, as it times a blob.
pub fn run_timed(
    reference: &mut Reference,
    blob: &Blob,
    look_ahead: LookAhead,
    counted: bool,
    timed: u32,
) -> Result<(NativeRun, f64), String> {
    let mut pieces = Pieces {
        blob,
        look_ahead,
        counted,
        target: Target {
            host: super::native::REFERENCE_HOST,
            offer: wcontainers_x86::Offer::default(),
        },
        made: BTreeMap::new(),
    };
    pieces.reach(blob.load);
    for runs in 1..=MOST_RUNS {
        let laid = lay_out(&pieces)?;
        let bytes = harness(blob, &laid).map_err(|e| format!("the blob does not assemble: {e}"))?;
        let request = |runs: u32| BlobRequest {
            bytes: &bytes,
            load: LOAD,
            stack_top: STACK_TOP,
            stack_bytes: blob.stack_bytes,
            headers: blob.headers_in_memory(),
            auxiliary: <X86_64 as GuestTest>::AUXILIARY,
            stdin: &[],
            runs,
        };
        let ran = reference.blob(&request(0))?;
        if ran.stderr.len() % RECORD_BYTES != 0 {
            return Err(format!(
                "standard error of {} bytes: {}",
                ran.stderr.len(),
                String::from_utf8_lossy(&ran.stderr)
            ));
        }
        let records: Vec<(Said, u64, u64)> = ran
            .stderr
            .as_chunks::<RECORD_BYTES>()
            .0
            .iter()
            .map(|r| {
                let word = |k: usize| {
                    let at = k * WORD as usize;
                    u64::from_le_bytes(r[at..at + WORD as usize].try_into().unwrap())
                };
                let said = Said::ALL
                    .into_iter()
                    .find(|&s| s as u64 == word(0))
                    .ok_or_else(|| format!("a record saying {}", word(0)))?;
                Ok((said, word(1), word(2)))
            })
            .collect::<Result<_, String>>()?;
        let missed = match records.last() {
            Some(&(Said::ExitMiss, t, _)) => {
                *laid.exits.get(&t).ok_or("an exit miss at no exit")?
            }
            Some(&(Said::LookupMiss, target, _)) => target,
            Some(&(Said::OtherCall, kind, t)) => {
                return Err(format!(
                    "a call {:?} returning to {t:#x}",
                    InsideCall::from_code(kind as u16)
                ));
            }
            Some(&(Said::OwnFault, pc, address)) => {
                return Err(format!(
                    "the blob faulted at {pc:#x} on {address:#x}, status {}",
                    ran.status
                ));
            }
            _ => {
                let syscalls = positions(&laid, &records)?;
                let seconds = if timed > 0 {
                    reference.blob(&request(timed))?.seconds
                } else {
                    ran.seconds
                };
                let run = NativeRun {
                    ran: Ran {
                        stdout: ran.stdout,
                        stderr: Vec::new(),
                        status: ran.status,
                    },
                    syscalls,
                    runs,
                    pieces: pieces.made.values().flatten().count(),
                };
                return Ok((run, seconds));
            }
        };
        if matches!(pieces.made.get(&missed), Some(Some(_))) {
            return Err(format!("a miss at {missed:#x}, whose piece is made"));
        }
        let image = blob.load..blob.load + (blob.bytes.len() as u64).next_multiple_of(PAGE);
        if !image.contains(&missed) {
            // Nothing is mapped there: the inside gives the guest the fetch's page fault, which ends it as Linux
            // ends a program that jumps there.
            let syscalls = positions(&laid, &records[..records.len() - 1])?;
            let run = NativeRun {
                ran: Ran {
                    stdout: ran.stdout,
                    stderr: Vec::new(),
                    status: crate::layout::SIGNALLED + libc::SIGSEGV,
                },
                syscalls,
                runs,
                pieces: pieces.made.values().flatten().count(),
            };
            return Ok((run, ran.seconds));
        }
        if matches!(pieces.made.get(&missed), Some(None)) || pieces.code_from(missed).is_none() {
            return Err(format!("the guest goes to {missed:#x}, where no code is"));
        }
        pieces.reach(missed);
    }
    Err(format!("{MOST_RUNS} runs, each missing a piece"))
}

/// The position after each system call of a run's records, read as the inside reads the count: the count register
/// less the site's lead, with what the inside adds for each repeated string it runs (its passes by the meaning's
/// rule, less the one event the piece's add counted for it).
fn positions(laid: &Laid, records: &[(Said, u64, u64)]) -> Result<Vec<Position>, String> {
    let mut added = 0u64;
    let mut positions = Vec::new();
    let mut records = records.iter();
    while let Some(&(said, first, second)) = records.next() {
        let site = laid
            .sites
            .get(first as usize)
            .ok_or("a record of no site")?;
        match (said, &site.answer) {
            (Said::SystemCall, Answer::SystemCall) => positions.push(Position {
                count: second.wrapping_sub(site.lead.into()) + added,
                pc: site.next,
            }),
            (Said::StringBefore, Answer::String { repeated, .. }) => {
                // A run ended in the instruction by a fault has no records after it.
                let (
                    Some(&(Said::StringPointers, _, start)),
                    Some(&(Said::StringAfter, after, _)),
                    Some(&(Said::StringPointers, source, destination)),
                ) = (records.next(), records.next(), records.next())
                else {
                    break;
                };
                // The flags the passes depend on: DF, which way rdi went (a host process keeps TF off). A string
                // with no element is one pass either way; lods, which leaves rdi, is never bulk.
                let backward = (destination.wrapping_sub(start) as i64) < 0;
                let rflags = if backward { wcontainers_x86::DF } else { 0 };
                let width = |v: u64| {
                    if repeated.address32 {
                        v & u64::from(u32::MAX)
                    } else {
                        v
                    }
                };
                let passes =
                    wcontainers_x86::strings::string_passes(&wcontainers_x86::strings::Repeated {
                        rflags,
                        count_before: width(second),
                        count_after: width(after),
                        distance: destination.wrapping_sub(source),
                        ..*repeated
                    });
                added += passes - 1;
            }
            other => {
                return Err(format!(
                    "a record {other:?} of site {first}",
                    other = other.0
                ));
            }
        }
    }
    Ok(positions)
}

/// Where each part of the blob is, from the pieces' size on.
struct Places {
    routines: u64,
    slots: u64,
    image: u64,
    area: u64,
    end: u64,
}

fn places(blob: &Blob, laid: &Laid, routines: usize) -> Places {
    let routines_at = PIECES + laid.bytes.len() as u64;
    let slots = (routines_at + routines as u64).next_multiple_of(WORD);
    let image = (slots + (laid.slots.len() * 2) as u64 * WORD).next_multiple_of(PAGE);
    let area = (image + blob.bytes.len() as u64).next_multiple_of(PAGE);
    Places {
        routines: routines_at,
        slots,
        image,
        area,
        end: area + FIRST_LEVEL as u64,
    }
}

/// The whole blob.
fn harness(blob: &Blob, laid: &Laid) -> Result<Vec<u8>, IcedError> {
    // The routines' size is known once they are assembled somewhere; their code does not depend on where.
    let size = routines(laid, PIECES + laid.bytes.len() as u64, 0)?
        .bytes
        .len();
    let at = places(blob, laid, size);
    let routines = routines(laid, at.routines, at.area)?;
    assert_eq!(
        routines.bytes.len(),
        size,
        "the routines' size does not depend on the area"
    );
    let mut bytes = vec![0u8; (at.end - LOAD) as usize];
    let mut put = |to: u64, part: &[u8]| {
        let to = (to - LOAD) as usize;
        bytes[to..to + part.len()].copy_from_slice(part);
    };
    let prologue = prologue(blob, laid, &at)?;
    assert!(prologue.len() as u64 <= PROLOGUE_ROOM, "the prologue fits");
    put(PROLOGUE, &prologue);
    let handler = handler(laid, &routines.copies, at.area)?;
    assert!(handler.len() as u64 <= PAGE, "the handler fits its page");
    put(HANDLER, &handler);
    put(RESTORER, &restorer(RESTORER)?);
    let flags = u64::from(SA_SIGINFO | SA_ONSTACK | SA_RESTORER);
    for (field, value) in [
        (
            core::mem::offset_of!(kernel_sigaction, sa_handler_kernel),
            HANDLER,
        ),
        (core::mem::offset_of!(kernel_sigaction, sa_flags), flags),
        (
            core::mem::offset_of!(kernel_sigaction, sa_restorer),
            RESTORER,
        ),
    ] {
        put(BLOCK + ACTION_AT + field as u64, &value.to_le_bytes());
    }
    for (field, value) in [
        (core::mem::offset_of!(sigaltstack, ss_sp), ALTERNATE_STACK),
        (
            core::mem::offset_of!(sigaltstack, ss_size),
            ALTERNATE_STACK_BYTES,
        ),
    ] {
        put(BLOCK + ALTERNATE_AT + field as u64, &value.to_le_bytes());
    }
    put(PIECES, &laid.bytes);
    put(at.routines, &routines.bytes);
    let slots: Vec<u8> = laid
        .slots
        .iter()
        .flat_map(|&(slot, to)| [slot, to])
        .flat_map(u64::to_le_bytes)
        .collect();
    put(at.slots, &slots);
    put(at.image, &blob.bytes);
    let fixed = FixedPart {
        entries: routines.entries,
    };
    put(at.area + FIXED as u64, fixed.as_bytes());
    Ok(bytes)
}

/// Writes a record from `kind` and the two registers, then ends the blob. Clobbers what it likes.
fn record_and_end(
    a: &mut CodeAssembler,
    area: u64,
    said: Said,
    first: AsmRegister64,
    second: AsmRegister64,
) -> Result<(), IcedError> {
    let record = area + scratch(core::mem::offset_of!(Scratch, record));
    a.mov(rbx, record)?;
    a.mov(qword_ptr(rbx + WORD as i32), first)?;
    a.mov(qword_ptr(rbx + 2 * WORD as i32), second)?;
    a.mov(rax, said as u64)?;
    a.mov(qword_ptr(rbx), rax)?;
    system_call(
        a,
        libc::SYS_write,
        &[libc::STDERR_FILENO as u64, record, RECORD_BYTES as u64],
    )?;
    // Any status: the record says how the blob ended.
    system_call(a, libc::SYS_exit_group, &[1])
}

/// The routines the area's entries point at.
struct Routines {
    bytes: Vec<u8>,
    entries: [u64; InsideCall::ALL.len()],
    /// Where the guest's repeated string instructions run, whose faults are the guest's.
    copies: Vec<u64>,
}

/// A word of a record.
#[derive(Clone, Copy)]
enum Word {
    Register(AsmRegister64),
    Constant(i32),
}

/// Writes a record of `said` and two words to standard error, every register and flag left as it was.
fn keep_record(
    a: &mut CodeAssembler,
    area: u64,
    said: Said,
    first: Word,
    second: Word,
) -> Result<(), IcedError> {
    let word = |field: usize| qword_ptr(scratch(field)).gs();
    let kept = [
        (core::mem::offset_of!(Scratch, rax), rax),
        (core::mem::offset_of!(Scratch, rdi), rdi),
        (core::mem::offset_of!(Scratch, rsi), rsi),
        (core::mem::offset_of!(Scratch, rdx), rdx),
        (core::mem::offset_of!(Scratch, rcx), rcx),
        (core::mem::offset_of!(Scratch, r11), r11),
    ];
    for (field, r) in kept {
        a.mov(word(field), r)?;
    }
    let record = core::mem::offset_of!(Scratch, record);
    a.mov(word(record), said as i32)?;
    for (n, value) in [first, second].into_iter().enumerate() {
        let at = word(record + (n + 1) * WORD as usize);
        match value {
            Word::Register(r) => a.mov(at, r)?,
            Word::Constant(c) => a.mov(at, c)?,
        }
    }
    system_call(
        a,
        libc::SYS_write,
        &[
            libc::STDERR_FILENO as u64,
            area + scratch(record),
            RECORD_BYTES as u64,
        ],
    )?;
    for (field, r) in kept {
        a.mov(r, word(field))?;
    }
    Ok(())
}

/// The routines the area's entries point at, assembled at `at` for the area at `area`.
fn routines(laid: &Laid, at: u64, area: u64) -> Result<Routines, IcedError> {
    let mut a = CodeAssembler::new(wcontainers_x86::MODE_BITS)?;
    let gs_word = |offset: u64| qword_ptr(offset).gs();
    let withheld = |number: u8| {
        let k = WITHHELD.iter().position(|&w| w == number)?;
        Some((WITHHELD_VALUES + k * WORD as usize) as u64)
    };
    let (count, t) = (assembled(COUNT), assembled(T));
    let mut entry_labels: Vec<CodeLabel> =
        InsideCall::ALL.iter().map(|_| a.create_label()).collect();
    let mut copies = Vec::new();
    // The answered instruction: found by its return point, compared flag-free (`lea` and `jrcxz`) with the guest's
    // rcx kept aside.
    let answer = InsideCall::Answer.code() as usize;
    a.set_label(&mut entry_labels[answer])?;
    let dispatch = gs_word(scratch(core::mem::offset_of!(Scratch, dispatch)));
    a.mov(dispatch, rcx)?;
    let mut site_labels: Vec<CodeLabel> = laid.sites.iter().map(|_| a.create_label()).collect();
    for (site, label) in laid.sites.iter().zip(&site_labels) {
        let mut hit = a.create_label();
        let mut next = a.create_label();
        a.mov(rcx, site.t.wrapping_neg())?;
        a.lea(rcx, qword_ptr(rcx + t))?;
        a.jrcxz(hit)?;
        a.jmp(next)?;
        a.set_label(&mut hit)?;
        a.mov(rcx, dispatch)?;
        a.jmp(*label)?;
        a.set_label(&mut next)?;
    }
    let code = rcx_code(&mut a, InsideCall::Answer)?;
    record_and_end(&mut a, area, Said::OtherCall, code, t)?;
    let scratch_word = |field: usize| gs_word(scratch(field));
    for (k, (site, label)) in laid.sites.iter().zip(&mut site_labels).enumerate() {
        a.set_label(label)?;
        let k = Word::Constant(i32::try_from(k).expect("a site's number"));
        match &site.answer {
            Answer::SystemCall => {
                keep_record(&mut a, area, Said::SystemCall, k, Word::Register(count))?;
                // The guest's own call, its r10 from the area where withheld; then its r11 is the flags, its
                // rcx the next pc.
                a.mov(scratch_word(core::mem::offset_of!(Scratch, count)), count)?;
                if let Some(slot) = withheld(wcontainers_x86::R10) {
                    a.mov(r10, gs_word(slot))?;
                }
                a.syscall()?;
                if let Some(slot) = withheld(wcontainers_x86::R11) {
                    a.mov(gs_word(slot), r11)?;
                }
                a.mov(count, scratch_word(core::mem::offset_of!(Scratch, count)))?;
                a.mov(rcx, site.next)?;
            }
            Answer::String { bytes, .. } => {
                keep_record(&mut a, area, Said::StringBefore, k, Word::Register(rcx))?;
                keep_record(
                    &mut a,
                    area,
                    Said::StringPointers,
                    Word::Register(rsi),
                    Word::Register(rdi),
                )?;
                let mut copy = a.create_label();
                a.set_label(&mut copy)?;
                a.db(bytes)?;
                copies.push(copy);
                keep_record(
                    &mut a,
                    area,
                    Said::StringAfter,
                    Word::Register(rcx),
                    Word::Constant(0),
                )?;
                keep_record(
                    &mut a,
                    area,
                    Said::StringPointers,
                    Word::Register(rsi),
                    Word::Register(rdi),
                )?;
            }
        }
        a.mov(t, site.t)?;
        a.jmp(t)?;
    }
    let exit = InsideCall::Exit.code() as usize;
    a.set_label(&mut entry_labels[exit])?;
    record_and_end(&mut a, area, Said::ExitMiss, t, t)?;
    for call in InsideCall::ALL {
        if matches!(call, InsideCall::Answer | InsideCall::Exit) {
            continue;
        }
        a.set_label(&mut entry_labels[call.code() as usize])?;
        let code = rcx_code(&mut a, call)?;
        record_and_end(&mut a, area, Said::OtherCall, code, t)?;
    }
    let result = a.assemble_options(
        at,
        iced_x86::BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS,
    )?;
    let entries = core::array::from_fn(|k| {
        result
            .label_ip(&entry_labels[k])
            .expect("every entry is placed")
    });
    let copies = copies
        .iter()
        .map(|c| result.label_ip(c).expect("every copy is placed"))
        .collect();
    Ok(Routines {
        bytes: result.inner.code_buffer,
        entries,
        copies,
    })
}

/// `mov ecx, code` of `call`: rcx.
fn rcx_code(a: &mut CodeAssembler, call: InsideCall) -> Result<AsmRegister64, IcedError> {
    a.mov(rcx, u64::from(call.code()))?;
    Ok(rcx)
}

/// The handler of every signal a stop raises: a lookup's miss (the jump to an empty second-level word, host pc
/// zero, the target in t) is recorded; a fault in the pieces is the guest's, which ends the blob by the same
/// signal (the default action put back, the instruction run again); any other is the blob's own.
fn handler(laid: &Laid, copies: &[u64], area: u64) -> Result<Vec<u8>, IcedError> {
    use linux_raw_sys::general::{__sifields__bindgen_ty_5, siginfo__bindgen_ty_1__bindgen_ty_1};
    let mut a = CodeAssembler::new(wcontainers_x86::MODE_BITS)?;
    let context = core::mem::offset_of!(libc::ucontext_t, uc_mcontext)
        + core::mem::offset_of!(libc::mcontext_t, gregs);
    let frame =
        |register: libc::c_int| (context + register as usize * size_of::<libc::greg_t>()) as i32;
    let info_address = (core::mem::offset_of!(siginfo__bindgen_ty_1__bindgen_ty_1, _sifields)
        + core::mem::offset_of!(__sifields__bindgen_ty_5, _addr)) as i32;
    let mut lookup = a.create_label();
    let mut own = a.create_label();
    let mut guest = a.create_label();
    a.mov(r12, qword_ptr(rdx + frame(libc::REG_RIP)))?;
    a.mov(
        r13,
        qword_ptr(rdx + frame(FRAME_GENERAL[usize::from(number(T))])),
    )?;
    a.mov(r14, qword_ptr(rsi + info_address))?;
    a.test(r12, r12)?;
    a.jz(lookup)?;
    for &copy in copies {
        a.mov(rax, copy)?;
        a.cmp(r12, rax)?;
        a.je(guest)?;
    }
    a.mov(rax, r12)?;
    a.mov(rcx, PIECES)?;
    a.sub(rax, rcx)?;
    a.mov(rcx, laid.bytes.len() as u64)?;
    a.cmp(rax, rcx)?;
    a.jae(own)?;
    a.set_label(&mut guest)?;
    // rdi still holds the signal.
    a.mov(rsi, BLOCK + DEFAULT_ACTION_AT)?;
    a.mov(rdx, 0u64)?;
    a.mov(
        r10,
        size_of::<linux_raw_sys::general::kernel_sigset_t>() as u64,
    )?;
    a.mov(rax, libc::SYS_rt_sigaction as u64)?;
    a.syscall()?;
    a.ret()?;
    a.set_label(&mut lookup)?;
    record_and_end(&mut a, area, Said::LookupMiss, r13, r12)?;
    a.set_label(&mut own)?;
    record_and_end(&mut a, area, Said::OwnFault, r12, r14)?;
    a.assemble(HANDLER)
}

/// The code that sets the blob up and enters the program's first piece with the state the reference starts a
/// blob with: its registers and flags as they are here, which it keeps aside first.
fn prologue(blob: &Blob, laid: &Laid, at: &Places) -> Result<Vec<u8>, IcedError> {
    let mut a = CodeAssembler::new(wcontainers_x86::MODE_BITS)?;
    let mut saved: Vec<CodeLabel> = (0..GENERAL_REGISTERS).map(|_| a.create_label()).collect();
    let mut flags = a.create_label();
    let mut below = a.create_label();
    let mut start = a.create_label();
    a.jmp(start)?;
    for label in saved.iter_mut().chain([&mut flags, &mut below]) {
        a.set_label(label)?;
        a.dq(&[0])?;
    }
    a.set_label(&mut start)?;
    // The word below sp is the program's stack: pushing the flags must leave it as it was.
    a.mov(qword_ptr(saved[0]), rax)?;
    a.mov(rax, qword_ptr(rsp - WORD as i32))?;
    a.mov(qword_ptr(below), rax)?;
    a.pushfq()?;
    a.pop(rax)?;
    a.mov(qword_ptr(flags), rax)?;
    a.mov(rax, qword_ptr(below))?;
    a.mov(qword_ptr(rsp - WORD as i32), rax)?;
    for (n, &label) in saved.iter().enumerate().skip(1) {
        a.mov(qword_ptr(label), general_register(n))?;
    }
    let read_write = libc::PROT_READ | libc::PROT_WRITE;
    // No core dump where the guest's fault ends the blob: the status is the signal's alone, and the host's crash
    // reporter is not run.
    system_call(
        &mut a,
        libc::SYS_prctl,
        &[libc::PR_SET_DUMPABLE as u64, u64::from(false)],
    )?;
    // The program at its own address.
    let image_bytes = (blob.bytes.len() as u64).next_multiple_of(PAGE);
    map(&mut a, blob.load, image_bytes, read_write)?;
    copy(&mut a, blob.load, at.image, blob.bytes.len() as u64)?;
    // The first level: every lower region's word the second level less that region's table offset.
    let first = at.area + FIRST_LEVEL as u64;
    let first_bytes = LOWER_REGIONS * WORD;
    map(&mut a, first, first_bytes, read_write)?;
    a.mov(rdi, first)?;
    a.mov(rax, SECOND_LEVEL)?;
    a.mov(rdx, REGION_BYTES)?;
    a.mov(rcx, LOWER_REGIONS)?;
    let mut fill = a.create_label();
    a.set_label(&mut fill)?;
    a.mov(qword_ptr(rdi), rax)?;
    a.sub(rax, rdx)?;
    a.add(rdi, WORD as i32)?;
    a.dec(rcx)?;
    a.jnz(fill)?;
    protect(&mut a, first, first_bytes, libc::PROT_READ)?;
    protect(&mut a, at.area + FIXED as u64, PAGE, libc::PROT_READ)?;
    // The second level, every word empty but the pieces'.
    let flags_reserved =
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE | libc::MAP_NORESERVE;
    system_call(
        &mut a,
        libc::SYS_mmap,
        &[
            SECOND_LEVEL,
            REGION_BYTES,
            read_write as u64,
            flags_reserved as u64,
            u64::MAX,
            0,
        ],
    )?;
    a.mov(rsi, at.slots)?;
    a.mov(rcx, laid.slots.len() as u64)?;
    let mut slot = a.create_label();
    a.set_label(&mut slot)?;
    a.mov(rax, qword_ptr(rsi))?;
    a.mov(rdx, qword_ptr(rsi + WORD as i32))?;
    a.mov(qword_ptr(rax), rdx)?;
    a.add(rsi, 2 * WORD as i32)?;
    a.dec(rcx)?;
    a.jnz(slot)?;
    protect(&mut a, SECOND_LEVEL, REGION_BYTES, libc::PROT_READ)?;
    // The handler, on a stack of its own.
    map(&mut a, ALTERNATE_STACK, ALTERNATE_STACK_BYTES, read_write)?;
    system_call(&mut a, libc::SYS_sigaltstack, &[BLOCK + ALTERNATE_AT, 0])?;
    for signal in STOP_SIGNALS {
        system_call(
            &mut a,
            libc::SYS_rt_sigaction,
            &[
                signal.into(),
                BLOCK + ACTION_AT,
                0,
                size_of::<linux_raw_sys::general::kernel_sigset_t>() as u64,
            ],
        )?;
    }
    // The thread area on GS, the withheld registers' guest values in it.
    a.mov(rax, at.area)?;
    a.wrgsbase(rax)?;
    for (k, &w) in WITHHELD.iter().enumerate() {
        a.mov(rax, qword_ptr(saved[usize::from(w)]))?;
        a.mov(
            qword_ptr(WITHHELD_VALUES as u64 + k as u64 * WORD).gs(),
            rax,
        )?;
    }
    // The flags (the word below sp kept), then every other register as it was.
    let (count, t, u) = (assembled(COUNT), assembled(T), assembled(U));
    a.mov(u, qword_ptr(rsp - WORD as i32))?;
    a.push(qword_ptr(flags))?;
    a.popfq()?;
    a.mov(qword_ptr(rsp - WORD as i32), u)?;
    for (n, &label) in saved.iter().enumerate() {
        if n == RSP || WITHHELD.contains(&(n as u8)) {
            continue;
        }
        a.mov(general_register(n), qword_ptr(label))?;
    }
    a.mov(count, 0u64)?;
    a.mov(u, 0u64)?;
    a.mov(t, laid.entry)?;
    a.jmp(t)?;
    a.assemble(PROLOGUE)
}
