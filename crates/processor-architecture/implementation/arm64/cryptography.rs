//! The cryptography instructions of Advanced SIMD registers: AES, SHA-1, SHA-256, SHA-512 and SHA-3, each a
//! transcription of Arm's pseudocode on its page (linked at each operation).

use core::convert::Infallible;

use crate::encoding::{Field, RD, RM, RN};
use crate::{Exception, Instruction, Processor};

/// The third source of EOR3 and BCAX.
const RA: Field = Field::new(10, 5);
/// The rotation of XAR.
const ROTATION: Field = Field::new(10, 6);
const WORD_BITS: u32 = u32::BITS;
const DOUBLEWORD_BITS: u32 = u64::BITS;
/// The bytes of an AES state, four columns of four.
const AES_STATE_BYTES: usize = 16;
const AES_COLUMN_BYTES: usize = 4;
/// The polynomial x^8 + x^4 + x^3 + x + 1 of AES's field, without its x^8.
const AES_REDUCTION: u8 = 0x1B;
/// The constant the S-box adds after its affine map.
const AES_AFFINE_CONSTANT: u8 = 0x63;

/// What one cryptography instruction computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CryptographyOperation {
    /// [AESE](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/AESE--AES-single-round-encryption-)
    AesEncrypt,
    /// [AESD](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/AESD--AES-single-round-decryption-)
    AesDecrypt,
    /// [AESMC](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/AESMC--AES-mix-columns-)
    AesMixColumns,
    /// [AESIMC](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/AESIMC--AES-inverse-mix-columns-)
    AesInverseMixColumns,
    /// [SHA1C](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA1C--SHA1-hash-update--choose--),
    /// SHA1P and SHA1M: four rounds with the choose, parity or majority function.
    Sha1Hash(Sha1Function),
    /// [SHA1H](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA1H--SHA1-fixed-rotate-)
    Sha1FixedRotate,
    /// [SHA1SU0](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA1SU0--SHA1-schedule-update-0-)
    Sha1ScheduleUpdate0,
    /// [SHA1SU1](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA1SU1--SHA1-schedule-update-1-)
    Sha1ScheduleUpdate1,
    /// [SHA256H](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA256H--SHA256-hash-update--part-1--)
    /// and SHA256H2, which keeps the other half of the state.
    Sha256Hash { first_part: bool },
    /// [SHA256SU0](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA256SU0--SHA256-schedule-update-0-)
    Sha256ScheduleUpdate0,
    /// [SHA256SU1](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA256SU1--SHA256-schedule-update-1-)
    Sha256ScheduleUpdate1,
    /// [SHA512H](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA512H--SHA512-hash-update-part-1-)
    Sha512Hash,
    /// [SHA512H2](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA512H2--SHA512-hash-update-part-2-)
    Sha512Hash2,
    /// [SHA512SU0](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA512SU0--SHA512-schedule-update-0-)
    Sha512ScheduleUpdate0,
    /// [SHA512SU1](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SHA512SU1--SHA512-schedule-update-1-)
    Sha512ScheduleUpdate1,
    /// [RAX1](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/RAX1--Rotate-and-exclusive-OR-)
    RotateAndExclusiveOr,
    /// [EOR3](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/EOR3--Three-way-exclusive-OR-)
    ThreeWayExclusiveOr,
    /// [BCAX](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/BCAX--Bit-clear-and-exclusive-OR-)
    BitClearAndExclusiveOr,
    /// [XAR](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/XAR--Exclusive-OR-and-rotate-)
    ExclusiveOrAndRotate,
}

/// The function of three words each SHA-1 hash round applies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sha1Function {
    Choose,
    Parity,
    Majority,
}

/// The operation of each modeled cryptography instruction, none for every other instruction.
pub const fn to_cryptography_operation(
    instruction: Instruction<Infallible>,
) -> Option<CryptographyOperation> {
    use CryptographyOperation as O;
    use Instruction as I;
    Some(match instruction {
        I::AeseVectorRr => O::AesEncrypt,
        I::AesdVectorRr => O::AesDecrypt,
        I::AesmcVectorRr => O::AesMixColumns,
        I::AesimcVectorRr => O::AesInverseMixColumns,
        I::Sha1c => O::Sha1Hash(Sha1Function::Choose),
        I::Sha1p => O::Sha1Hash(Sha1Function::Parity),
        I::Sha1m => O::Sha1Hash(Sha1Function::Majority),
        I::Sha1h => O::Sha1FixedRotate,
        I::Sha1su0 => O::Sha1ScheduleUpdate0,
        I::Sha1su1 => O::Sha1ScheduleUpdate1,
        I::Sha256h => O::Sha256Hash { first_part: true },
        I::Sha256h2 => O::Sha256Hash { first_part: false },
        I::Sha256su0 => O::Sha256ScheduleUpdate0,
        I::Sha256su1 => O::Sha256ScheduleUpdate1,
        I::Sha512h => O::Sha512Hash,
        I::Sha512h2 => O::Sha512Hash2,
        I::Sha512su0 => O::Sha512ScheduleUpdate0,
        I::Sha512su1 => O::Sha512ScheduleUpdate1,
        I::Rax1Vector => O::RotateAndExclusiveOr,
        I::Eor3Vector => O::ThreeWayExclusiveOr,
        I::BcaxVector => O::BitClearAndExclusiveOr,
        I::XarVector => O::ExclusiveOrAndRotate,
        _ => return None,
    })
}

/// Applies the operation to the registers the word names; every form writes its destination whole.
pub fn simulate_cryptography<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: CryptographyOperation,
) -> Result<(), Exception> {
    use CryptographyOperation as O;
    let register = |field: Field| processor.vector_registers[field.read(word) as usize];
    let (d, n, m) = (register(RD), register(RN), register(RM));
    let result = match operation {
        O::AesEncrypt => {
            let state = if crate::planted::is(701) { d } else { d ^ n };
            let shifted = if crate::planted::is(702) {
                state
            } else {
                shift_aes_rows(state, false)
            };
            map_aes_bytes(shifted, |byte| substitute_aes_byte(byte, false))
        }
        O::AesDecrypt => map_aes_bytes(shift_aes_rows(d ^ n, true), |byte| {
            substitute_aes_byte(byte, !crate::planted::is(703))
        }),
        O::AesMixColumns => mix_aes_columns(n, false),
        O::AesInverseMixColumns => mix_aes_columns(n, true),
        O::Sha1Hash(function) => {
            let function = if crate::planted::is(704) && function == Sha1Function::Choose {
                Sha1Function::Parity
            } else {
                function
            };
            hash_sha1(d, n as u32, m, function)
        }
        O::Sha1FixedRotate => u128::from((n as u32).rotate_left(30)),
        O::Sha1ScheduleUpdate0 => {
            let joined = (d >> DOUBLEWORD_BITS) | (n << DOUBLEWORD_BITS);
            joined ^ d ^ m
        }
        O::Sha1ScheduleUpdate1 => {
            let t = d ^ (n >> WORD_BITS);
            let mut result = map_words(t, |word| word.rotate_left(1));
            let top = to_word(t, 3).rotate_left(1) ^ to_word(t, 0).rotate_left(2);
            result = with_word(result, 3, top);
            result
        }
        O::Sha256Hash { first_part: true } => hash_sha256(d, n, m, true),
        O::Sha256Hash { first_part: false } => hash_sha256(n, d, m, false),
        O::Sha256ScheduleUpdate0 => {
            let t = (d >> WORD_BITS) | (u128::from(to_word(n, 0)) << (3 * WORD_BITS));
            let mut result = 0;
            for element in 0..4 {
                let value = to_word(t, element);
                let sigma = value.rotate_right(7) ^ value.rotate_right(18) ^ (value >> 3);
                result = with_word(result, element, sigma.wrapping_add(to_word(d, element)));
            }
            result
        }
        O::Sha256ScheduleUpdate1 => schedule_sha256(d, n, m),
        O::Sha512Hash => hash_sha512(n, m, d),
        O::Sha512Hash2 => hash_sha512_second(n, m, d),
        O::Sha512ScheduleUpdate0 => {
            let (w_low, w_high) = to_doublewords(d);
            let (x_low, _) = to_doublewords(n);
            let sigma = |value: u64| value.rotate_right(1) ^ value.rotate_right(8) ^ (value >> 7);
            from_doublewords(
                w_low.wrapping_add(sigma(w_high)),
                w_high.wrapping_add(sigma(x_low)),
            )
        }
        O::Sha512ScheduleUpdate1 => {
            let (w_low, w_high) = to_doublewords(d);
            let (x_low, x_high) = to_doublewords(n);
            let (y_low, y_high) = to_doublewords(m);
            let shift = if crate::planted::is(705) { 7 } else { 6 };
            let sigma =
                |value: u64| value.rotate_right(19) ^ value.rotate_right(61) ^ (value >> shift);
            from_doublewords(
                w_low.wrapping_add(sigma(x_low)).wrapping_add(y_low),
                w_high.wrapping_add(sigma(x_high)).wrapping_add(y_high),
            )
        }
        O::RotateAndExclusiveOr => {
            let (n_low, n_high) = to_doublewords(n);
            let (m_low, m_high) = to_doublewords(m);
            from_doublewords(n_low ^ m_low.rotate_left(1), n_high ^ m_high.rotate_left(1))
        }
        O::ThreeWayExclusiveOr => {
            let a = if crate::planted::is(706) {
                0
            } else {
                register(RA)
            };
            n ^ m ^ a
        }
        O::BitClearAndExclusiveOr => n ^ (m & !register(RA)),
        O::ExclusiveOrAndRotate => {
            let rotation = ROTATION.read(word);
            let (low, high) = to_doublewords(n ^ m);
            let rotate = |value: u64| {
                if crate::planted::is(707) {
                    value.rotate_left(rotation)
                } else {
                    value.rotate_right(rotation)
                }
            };
            from_doublewords(rotate(low), rotate(high))
        }
    };
    processor.vector_registers[RD.read(word) as usize] = result;
    Ok(())
}

const fn to_word(vector: u128, element: u32) -> u32 {
    (vector >> (element * WORD_BITS)) as u32
}

const fn with_word(vector: u128, element: u32, value: u32) -> u128 {
    let shift = element * WORD_BITS;
    (vector & !((u32::MAX as u128) << shift)) | ((value as u128) << shift)
}

fn map_words(vector: u128, map: impl Fn(u32) -> u32) -> u128 {
    (0..4).fold(0, |result, element| {
        with_word(result, element, map(to_word(vector, element)))
    })
}

const fn to_doublewords(vector: u128) -> (u64, u64) {
    (vector as u64, (vector >> DOUBLEWORD_BITS) as u64)
}

const fn from_doublewords(low: u64, high: u64) -> u128 {
    (low as u128) | ((high as u128) << DOUBLEWORD_BITS)
}

/// [SHAchoose](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-crypto#impl-shared.SHAchoose.3), SHAparity and SHAmajority.
const fn apply_sha1_function(function: Sha1Function, x: u32, y: u32, z: u32) -> u32 {
    match function {
        Sha1Function::Choose => ((y ^ z) & x) ^ z,
        Sha1Function::Parity => x ^ y ^ z,
        Sha1Function::Majority => (x & y) | ((x | y) & z),
    }
}

/// The four rounds of SHA1C, SHA1P and SHA1M on the state X (Qd), Y (Sn) and the schedule W (Vm).
fn hash_sha1(state: u128, mut y: u32, schedule: u128, function: Sha1Function) -> u128 {
    let mut x = state;
    for element in 0..4 {
        let t = apply_sha1_function(function, to_word(x, 1), to_word(x, 2), to_word(x, 3));
        y = y
            .wrapping_add(to_word(x, 0).rotate_left(5))
            .wrapping_add(t)
            .wrapping_add(to_word(schedule, element));
        x = with_word(x, 1, to_word(x, 1).rotate_left(30));
        // <Y, X> = ROL(Y : X, 32): Y takes X's top word, and X moves up a word over Y's old value.
        let top = to_word(x, 3);
        x = (x << WORD_BITS) | u128::from(y);
        y = top;
    }
    x
}

/// [SHA256hash](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-crypto#impl-shared.SHA256hash.4): four rounds, giving X for part 1 and Y for part 2.
fn hash_sha256(mut x: u128, mut y: u128, schedule: u128, first_part: bool) -> u128 {
    for element in 0..4 {
        let choose = apply_sha1_function(
            Sha1Function::Choose,
            to_word(y, 0),
            to_word(y, 1),
            to_word(y, 2),
        );
        let majority = apply_sha1_function(
            Sha1Function::Majority,
            to_word(x, 0),
            to_word(x, 1),
            to_word(x, 2),
        );
        let y0 = to_word(y, 0);
        let sigma1 = y0.rotate_right(6) ^ y0.rotate_right(11) ^ y0.rotate_right(25);
        let t = to_word(y, 3)
            .wrapping_add(sigma1)
            .wrapping_add(choose)
            .wrapping_add(to_word(schedule, element));
        x = with_word(x, 3, t.wrapping_add(to_word(x, 3)));
        let x0 = to_word(x, 0);
        let sigma0 = x0.rotate_right(2) ^ x0.rotate_right(13) ^ x0.rotate_right(22);
        y = with_word(y, 3, t.wrapping_add(sigma0).wrapping_add(majority));
        // <Y, X> = ROL(Y : X, 32) over 256 bits.
        let x_top = to_word(x, 3);
        let y_top = to_word(y, 3);
        x = (x << WORD_BITS) | u128::from(y_top);
        y = (y << WORD_BITS) | u128::from(x_top);
    }
    if first_part { x } else { y }
}

/// SHA256SU1 on Vd, Vn and Vm.
fn schedule_sha256(d: u128, n: u128, m: u128) -> u128 {
    let t0 = (n >> WORD_BITS) | (u128::from(to_word(m, 0)) << (3 * WORD_BITS));
    let sigma = |value: u32| value.rotate_right(17) ^ value.rotate_right(19) ^ (value >> 10);
    let mut result = 0;
    for element in 0..2 {
        let value = sigma(to_word(m, element + 2))
            .wrapping_add(to_word(d, element))
            .wrapping_add(to_word(t0, element));
        result = with_word(result, element, value);
    }
    for element in 2..4 {
        let value = sigma(to_word(result, element - 2))
            .wrapping_add(to_word(d, element))
            .wrapping_add(to_word(t0, element));
        result = with_word(result, element, value);
    }
    result
}

/// SHA512H with X = Vn, Y = Vm and W = Vd.
fn hash_sha512(x: u128, y: u128, w: u128) -> u128 {
    let (x_low, x_high) = to_doublewords(x);
    let (y_low, y_high) = to_doublewords(y);
    let (w_low, w_high) = to_doublewords(w);
    let sigma1 =
        |value: u64| value.rotate_right(14) ^ value.rotate_right(18) ^ value.rotate_right(41);
    let high = ((y_high & x_low) ^ (!y_high & x_high))
        .wrapping_add(sigma1(y_high))
        .wrapping_add(w_high);
    let tmp = high.wrapping_add(y_low);
    let low = ((tmp & y_high) ^ (!tmp & x_low))
        .wrapping_add(sigma1(tmp))
        .wrapping_add(w_low);
    from_doublewords(low, high)
}

/// SHA512H2 with X = Vn, Y = Vm and W = Vd.
fn hash_sha512_second(x: u128, y: u128, w: u128) -> u128 {
    let (x_low, _) = to_doublewords(x);
    let (y_low, y_high) = to_doublewords(y);
    let (w_low, w_high) = to_doublewords(w);
    let sigma0 =
        |value: u64| value.rotate_right(28) ^ value.rotate_right(34) ^ value.rotate_right(39);
    let high = ((x_low & y_high) ^ (x_low & y_low) ^ (y_high & y_low))
        .wrapping_add(sigma0(y_low))
        .wrapping_add(w_high);
    let low = ((high & y_low) ^ (high & y_high) ^ (y_high & y_low))
        .wrapping_add(sigma0(high))
        .wrapping_add(w_low);
    from_doublewords(low, high)
}

/// The bytes of an AES state, the lowest first: byte `row + 4 * column`.
const fn to_aes_bytes(state: u128) -> [u8; AES_STATE_BYTES] {
    state.to_le_bytes()
}

fn map_aes_bytes(state: u128, map: impl Fn(u8) -> u8) -> u128 {
    u128::from_le_bytes(to_aes_bytes(state).map(map))
}

/// [AESShiftRows](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-crypto#impl-shared.AESShiftRows.1) and AESInvShiftRows: row `r` moves left by `r` columns, or right for the inverse.
fn shift_aes_rows(state: u128, inverse: bool) -> u128 {
    let bytes = to_aes_bytes(state);
    let mut shifted = [0; AES_STATE_BYTES];
    for column in 0..AES_COLUMN_BYTES {
        for row in 0..AES_COLUMN_BYTES {
            let moved = if inverse {
                (column + AES_COLUMN_BYTES - row) % AES_COLUMN_BYTES
            } else {
                (column + row) % AES_COLUMN_BYTES
            };
            shifted[row + AES_COLUMN_BYTES * column] = bytes[row + AES_COLUMN_BYTES * moved];
        }
    }
    u128::from_le_bytes(shifted)
}

/// The product of two elements of AES's field.
const fn multiply_aes(mut first: u8, mut second: u8) -> u8 {
    let mut product = 0;
    while second != 0 {
        if second & 1 != 0 {
            product ^= first;
        }
        let carry = first & 0x80 != 0;
        first <<= 1;
        if carry {
            first ^= AES_REDUCTION;
        }
        second >>= 1;
    }
    product
}

/// The multiplicative inverse in AES's field, zero for zero: the element to the 254th power.
const fn invert_aes(byte: u8) -> u8 {
    let mut result = 1;
    let mut power = 0;
    while power < 254 {
        result = multiply_aes(result, byte);
        power += 1;
    }
    result
}

/// The affine map of the S-box: each bit and the four above it, cyclically, with the constant.
const fn map_aes_affine(byte: u8) -> u8 {
    byte ^ byte.rotate_left(1)
        ^ byte.rotate_left(2)
        ^ byte.rotate_left(3)
        ^ byte.rotate_left(4)
        ^ AES_AFFINE_CONSTANT
}

/// The inverse of the affine map.
const fn unmap_aes_affine(byte: u8) -> u8 {
    byte.rotate_left(1) ^ byte.rotate_left(3) ^ byte.rotate_left(6) ^ 0x05
}

/// The S-box of [AESSubBytes](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-crypto#impl-shared.AESSubBytes.1), or of AESInvSubBytes, from its definition in FIPS 197.
const fn substitute_aes_byte(byte: u8, inverse: bool) -> u8 {
    if inverse {
        invert_aes(unmap_aes_affine(byte))
    } else {
        map_aes_affine(invert_aes(byte))
    }
}

/// [AESMixColumns](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-crypto#impl-shared.AESMixColumns.1) and AESInvMixColumns: each column times a fixed polynomial.
fn mix_aes_columns(state: u128, inverse: bool) -> u128 {
    let coefficients: [u8; AES_COLUMN_BYTES] = if inverse {
        [0x0E, 0x0B, 0x0D, 0x09]
    } else {
        [0x02, 0x03, 0x01, 0x01]
    };
    let bytes = to_aes_bytes(state);
    let mut mixed = [0; AES_STATE_BYTES];
    for column in 0..AES_COLUMN_BYTES {
        for row in 0..AES_COLUMN_BYTES {
            mixed[row + AES_COLUMN_BYTES * column] = (0..AES_COLUMN_BYTES).fold(0, |sum, place| {
                let coefficient = coefficients[(place + AES_COLUMN_BYTES - row) % AES_COLUMN_BYTES];
                sum ^ multiply_aes(coefficient, bytes[place + AES_COLUMN_BYTES * column])
            });
        }
    }
    u128::from_le_bytes(mixed)
}
