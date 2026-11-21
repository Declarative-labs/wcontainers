//! The IPC namespace's sysctls (`ipc/ipc_sysctl.c`, `ipc/mq_sysctl.c`), the limits they set, and
//! the integer parsing of a sysctl's write (`kernel/sysctl.c`).

use alloc::string::String;
use core::ops::RangeInclusive;

use wcontainers_abi::PAGE_SIZE;

use crate::Errno;
use crate::fs::PATH_SEPARATOR;
use crate::linux_extra::{
    HARD_MSGMAX, HARD_MSGSIZEMAX, IPCMNI, MIN_MSGMAX, MIN_MSGSIZEMAX, MQ_MSG_DEFAULT, MQ_MSG_MAX,
    MQ_MSGSIZE_DEFAULT, MQ_MSGSIZE_MAX, MQ_QUEUES_MAX, MSGMAX, MSGMNB, MSGMNI, SEMMNI, SEMMNS,
    SEMMSL, SEMOPM, SHMALL, SHMMAX, SHMMNI,
};

/// The directory under `/proc` that holds the sysctls; a key names its file with each dot made a
/// slash, as runc writes it.
pub(crate) const SYSCTL_ROOT: &str = "sys";
const KEY_SEPARATOR: char = '.';

pub(crate) const MSGMAX_PATH: &str = "sys/kernel/msgmax";
pub(crate) const MSGMNB_PATH: &str = "sys/kernel/msgmnb";
pub(crate) const MSGMNI_PATH: &str = "sys/kernel/msgmni";
pub(crate) const SEM_PATH: &str = "sys/kernel/sem";
pub(crate) const SHMALL_PATH: &str = "sys/kernel/shmall";
pub(crate) const SHMMAX_PATH: &str = "sys/kernel/shmmax";
pub(crate) const SHMMNI_PATH: &str = "sys/kernel/shmmni";
pub(crate) const MQ_MSG_DEFAULT_PATH: &str = "sys/fs/mqueue/msg_default";
pub(crate) const MQ_MSG_MAX_PATH: &str = "sys/fs/mqueue/msg_max";
pub(crate) const MQ_MSGSIZE_DEFAULT_PATH: &str = "sys/fs/mqueue/msgsize_default";
pub(crate) const MQ_MSGSIZE_MAX_PATH: &str = "sys/fs/mqueue/msgsize_max";
pub(crate) const MQ_QUEUES_MAX_PATH: &str = "sys/fs/mqueue/queues_max";

/// What parts the values of a sysctl holding several, as a read shows them.
pub(crate) const VALUE_SEPARATOR: char = '\t';

/// The separators a number may end at (`proc_wspace_sep`).
const NUMBER_ENDS: &[u8] = b" \t\n";
/// The longest number a write may hold, with its sign (`TMPBUFLEN`, less its terminator).
const NUMBER_TEXT: usize = 21;

/// Where `kernel.sem` holds each of its values.
pub(crate) const SEMMSL_INDEX: usize = 0;
pub(crate) const SEMMNS_INDEX: usize = 1;
pub(crate) const SEMOPM_INDEX: usize = 2;
pub(crate) const SEMMNI_INDEX: usize = 3;
pub(crate) const SEM_VALUES: usize = SEMMNI_INDEX + 1;

/// The limits of the container's IPC namespace (`struct ipc_namespace`), in the sysctls' types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IpcLimits {
    pub msg_max: i32,
    pub msg_mnb: i32,
    pub msg_mni: i32,
    /// SEMMSL, SEMMNS, SEMOPM, SEMMNI.
    pub sem: [i32; SEM_VALUES],
    pub shm_max: u64,
    pub shm_all: u64,
    pub shm_mni: i32,
    pub mq_queues_max: i32,
    pub mq_msg_max: i32,
    pub mq_msgsize_max: i32,
    pub mq_msg_default: i32,
    pub mq_msgsize_default: i32,
}

impl Default for IpcLimits {
    fn default() -> Self {
        Self {
            msg_max: MSGMAX as i32,
            msg_mnb: MSGMNB as i32,
            msg_mni: MSGMNI as i32,
            sem: [SEMMSL, SEMMNS as i32, SEMOPM as i32, SEMMNI as i32],
            shm_max: SHMMAX,
            shm_all: SHMALL,
            shm_mni: SHMMNI as i32,
            mq_queues_max: MQ_QUEUES_MAX as i32,
            mq_msg_max: MQ_MSG_MAX as i32,
            mq_msgsize_max: MQ_MSGSIZE_MAX as i32,
            mq_msg_default: MQ_MSG_DEFAULT as i32,
            mq_msgsize_default: MQ_MSGSIZE_DEFAULT as i32,
        }
    }
}

impl IpcLimits {
    /// A write of `text` at the start of the sysctl at `path` (below `/proc`), as its handler
    /// takes it; none where that sysctl is not one of these.
    pub(crate) fn write(&mut self, path: &str, text: &[u8]) -> Option<Result<(), Errno>> {
        let ipc_mni = 0..=IPCMNI as i32;
        let any = 0..=i32::MAX;
        let messages = MIN_MSGMAX..=HARD_MSGMAX as i32;
        let message_size = MIN_MSGSIZEMAX..=HARD_MSGSIZEMAX as i32;
        Some(match path {
            MSGMAX_PATH => ints(core::slice::from_mut(&mut self.msg_max), text, any),
            MSGMNB_PATH => ints(core::slice::from_mut(&mut self.msg_mnb), text, any),
            MSGMNI_PATH => ints(core::slice::from_mut(&mut self.msg_mni), text, ipc_mni),
            SHMMNI_PATH => ints(core::slice::from_mut(&mut self.shm_mni), text, ipc_mni),
            SHMMAX_PATH => unsigned_long(&mut self.shm_max, text),
            SHMALL_PATH => unsigned_long(&mut self.shm_all, text),
            // `proc_ipc_sem_dointvec`: every value an int, then SEMMNI checked and put back if
            // it is out of range.
            SEM_PATH => {
                let semmni = self.sem[SEMMNI_INDEX];
                ints(&mut self.sem, text, i32::MIN..=i32::MAX).and_then(|()| {
                    if ipc_mni.contains(&self.sem[SEMMNI_INDEX]) {
                        Ok(())
                    } else {
                        self.sem[SEMMNI_INDEX] = semmni;
                        Err(Errno::ERANGE)
                    }
                })
            }
            MQ_QUEUES_MAX_PATH => ints(
                core::slice::from_mut(&mut self.mq_queues_max),
                text,
                i32::MIN..=i32::MAX,
            ),
            MQ_MSG_MAX_PATH => ints(core::slice::from_mut(&mut self.mq_msg_max), text, messages),
            MQ_MSG_DEFAULT_PATH => ints(
                core::slice::from_mut(&mut self.mq_msg_default),
                text,
                messages,
            ),
            MQ_MSGSIZE_MAX_PATH => ints(
                core::slice::from_mut(&mut self.mq_msgsize_max),
                text,
                message_size,
            ),
            MQ_MSGSIZE_DEFAULT_PATH => ints(
                core::slice::from_mut(&mut self.mq_msgsize_default),
                text,
                message_size,
            ),
            _ => return None,
        })
    }
}

/// The file below `/proc` that a sysctl's key names.
pub(crate) fn path_of(key: &str) -> String {
    let mut path = String::from(SYSCTL_ROOT);
    let separator = char::from(PATH_SEPARATOR);
    path.push(separator);
    path.extend(key.chars().map(|character| {
        if character == KEY_SEPARATOR {
            separator
        } else {
            character
        }
    }));
    path
}

/// The part of a write a sysctl reads (`proc_sys_call_handler`'s buffer, less its terminator).
fn readable(text: &[u8]) -> &[u8] {
    &text[..text.len().min(PAGE_SIZE as usize - 1)]
}

fn skip_spaces(text: &[u8]) -> &[u8] {
    let start = text
        .iter()
        .position(|byte| !byte.is_ascii_whitespace() && *byte != VERTICAL_TAB)
        .unwrap_or(text.len());
    &text[start..]
}

/// The one space C's `isspace` has that Rust's ASCII whitespace does not.
const VERTICAL_TAB: u8 = 0x0b;

/// One number at the start of `text` (`proc_get_long`): its sign, its magnitude in the base its
/// prefix gives (`0x` hexadecimal, `0` octal), and what follows it.
fn number(text: &[u8]) -> Result<(bool, u64, &[u8]), Errno> {
    let window = &text[..text.len().min(NUMBER_TEXT)];
    let (negative, digits) = match window.split_first() {
        Some((&MINUS_SIGN, rest)) if text.len() > 1 => (true, rest),
        _ => (false, window),
    };
    if !digits.first().is_some_and(u8::is_ascii_digit) {
        return Err(Errno::EINVAL);
    }
    let (radix, prefix) = match digits {
        [ZERO, x, next, ..]
            if x.eq_ignore_ascii_case(&HEXADECIMAL_MARK) && next.is_ascii_hexdigit() =>
        {
            (HEXADECIMAL, HEXADECIMAL_PREFIX)
        }
        [ZERO, ..] => (OCTAL, 0),
        _ => (DECIMAL, 0),
    };
    let body = &digits[prefix..];
    let length = body
        .iter()
        .position(|byte| !char::from(*byte).is_digit(radix))
        .unwrap_or(body.len());
    let mut magnitude: u64 = 0;
    for byte in &body[..length] {
        let digit = char::from(*byte)
            .to_digit(radix)
            .expect("a digit of the base");
        magnitude = magnitude
            .checked_mul(radix.into())
            .and_then(|value| value.checked_add(digit.into()))
            .ok_or(Errno::EINVAL)?;
    }
    let used = window.len() - digits.len() + prefix + length;
    if used == NUMBER_TEXT {
        return Err(Errno::EINVAL);
    }
    let rest = &text[used..];
    if rest.first().is_some_and(|byte| !NUMBER_ENDS.contains(byte)) {
        return Err(Errno::EINVAL);
    }
    Ok((negative, magnitude, rest))
}

const MINUS_SIGN: u8 = b'-';
const ZERO: u8 = b'0';
const HEXADECIMAL_MARK: u8 = b'x';
const DECIMAL: u32 = 10;
const OCTAL: u32 = 8;
const HEXADECIMAL: u32 = 16;
const HEXADECIMAL_PREFIX: usize = 2;

/// `__do_proc_dointvec` with `do_proc_dointvec_minmax_conv`: up to one int per slot, each in
/// `range`; the first that is not stops it with EINVAL, keeping those before it.
fn ints(slots: &mut [i32], text: &[u8], range: RangeInclusive<i32>) -> Result<(), Errno> {
    let mut rest = readable(text);
    let mut first = true;
    for slot in slots {
        rest = skip_spaces(rest);
        if rest.is_empty() {
            break;
        }
        let (negative, magnitude, after) = number(rest)?;
        let value = if negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| range.contains(value))
        .ok_or(Errno::EINVAL)?;
        *slot = value;
        first = false;
        rest = after;
    }
    if first { Err(Errno::EINVAL) } else { Ok(()) }
}

/// `__do_proc_doulongvec_minmax` for one value with no bounds: a sign is EINVAL.
fn unsigned_long(slot: &mut u64, text: &[u8]) -> Result<(), Errno> {
    let rest = skip_spaces(readable(text));
    if rest.is_empty() {
        return Err(Errno::EINVAL);
    }
    match number(rest)? {
        (false, magnitude, _) => {
            *slot = magnitude;
            Ok(())
        }
        (true, ..) => Err(Errno::EINVAL),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn writes_parse_as_linux_parses_them() {
        let mut limits = IpcLimits::default();
        assert_eq!(limits.write(MSGMAX_PATH, b"  4000\n"), Some(Ok(())));
        assert_eq!(limits.msg_max, 4000);
        assert_eq!(limits.write(MSGMAX_PATH, b"0x20"), Some(Ok(())));
        assert_eq!(limits.msg_max, 32);
        assert_eq!(limits.write(MSGMAX_PATH, b"010"), Some(Ok(())));
        assert_eq!(limits.msg_max, 8);
        assert_eq!(limits.write(MSGMAX_PATH, b"-1"), Some(Err(Errno::EINVAL)));
        assert_eq!(limits.write(MSGMAX_PATH, b"12a"), Some(Err(Errno::EINVAL)));
        assert_eq!(limits.write(MSGMAX_PATH, b""), Some(Err(Errno::EINVAL)));
        let past_int = (i64::from(i32::MAX) + 1).to_string();
        assert_eq!(
            limits.write(MSGMAX_PATH, past_int.as_bytes()),
            Some(Err(Errno::EINVAL))
        );
        assert_eq!(limits.msg_max, 8);
        let past_mni = (IPCMNI + 1).to_string();
        assert_eq!(
            limits.write(MSGMNI_PATH, past_mni.as_bytes()),
            Some(Err(Errno::EINVAL))
        );
        let mni = IPCMNI.to_string();
        assert_eq!(limits.write(MSGMNI_PATH, mni.as_bytes()), Some(Ok(())));
        let most = u64::MAX.to_string();
        assert_eq!(limits.write(SHMMAX_PATH, most.as_bytes()), Some(Ok(())));
        assert_eq!(limits.shm_max, u64::MAX);
        let past_most = (u128::from(u64::MAX) + 1).to_string();
        assert_eq!(
            limits.write(SHMMAX_PATH, past_most.as_bytes()),
            Some(Err(Errno::EINVAL))
        );
        assert_eq!(limits.write(SHMALL_PATH, b"-5"), Some(Err(Errno::EINVAL)));
        assert_eq!(limits.write(SEM_PATH, b"250 64000 32 128"), Some(Ok(())));
        assert_eq!(limits.sem, [250, 64000, 32, 128]);
        assert_eq!(
            limits.write(SEM_PATH, b"1 2 3 40000"),
            Some(Err(Errno::ERANGE))
        );
        assert_eq!(limits.sem, [1, 2, 3, 128]);
        assert_eq!(limits.write(SEM_PATH, b"7"), Some(Ok(())));
        assert_eq!(limits.sem, [7, 2, 3, 128]);
        assert_eq!(
            limits.write(MQ_MSG_MAX_PATH, b"0"),
            Some(Err(Errno::EINVAL))
        );
        assert_eq!(
            limits.write(MQ_MSGSIZE_MAX_PATH, b"127"),
            Some(Err(Errno::EINVAL))
        );
        assert_eq!(limits.write(MQ_QUEUES_MAX_PATH, b"-3"), Some(Ok(())));
        assert_eq!(limits.mq_queues_max, -3);
        assert_eq!(limits.write(SYSCTL_ROOT, b"1"), None);
    }

    #[test]
    fn keys_name_their_files_below_proc() {
        assert_eq!(path_of("kernel.shmmax"), SHMMAX_PATH);
        assert_eq!(path_of("fs.mqueue.msg_max"), MQ_MSG_MAX_PATH);
    }
}
