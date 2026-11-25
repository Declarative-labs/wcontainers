//! The fuzzer's random numbers: a seeded generator, and the edge values every field is drawn from.

/// The 32-bit edges of what the calls take: limits, one past each, powers of two near them, and values that wrap.
const EDGES_32: &[u32] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    12,
    15,
    16,
    17,
    29,
    30,
    31,
    32,
    33,
    63,
    64,
    65,
    127,
    128,
    129,
    255,
    256,
    341,
    342,
    511,
    1023,
    1024,
    1025,
    2047,
    2048,
    2049,
    4095,
    4096,
    8192,
    16383,
    16384,
    16385,
    32767,
    32768,
    32769,
    65535,
    65536,
    1 << 24,
    (1 << 24) + 1,
    1 << 30,
    (1 << 31) - 1,
    1 << 31,
    (1 << 31) + 1,
    u32::MAX - 15,
    u32::MAX - 7,
    u32::MAX - 3,
    u32::MAX - 1,
    u32::MAX,
];

/// The 64-bit edges of what the calls take, as for `EDGES_32`.
///
/// None makes a buffer of more than a few megabytes: the largest buffer Metal makes is far below 2^40 bytes.
const EDGES_64: &[u64] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    7,
    8,
    12,
    15,
    16,
    17,
    31,
    32,
    63,
    64,
    65,
    255,
    256,
    257,
    1023,
    1024,
    4092,
    4095,
    4096,
    4100,
    65532,
    65536,
    262_136,
    262_144,
    262_152,
    1 << 20,
    (1 << 20) + 4,
    1 << 40,
    1 << 62,
    1 << 63,
    (1 << 63) + 64,
    u64::MAX / 2 + 1,
    u64::MAX - 63,
    u64::MAX - 15,
    u64::MAX - 7,
    u64::MAX - 3,
    u64::MAX - 1,
    u64::MAX,
];

/// The floats at the edges of what the calls take.
const EDGES_FLOAT: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    0.25,
    2.5,
    1e-30,
    1e30,
    255.0,
    256.0,
    3.5,
    f64::MAX,
    f64::MIN,
    f64::MIN_POSITIVE,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
];

/// A generator of random numbers, seeded, the same for the same seed and batch on every run (SplitMix64).
///
/// https://prng.di.unimi.it/splitmix64.c
pub struct Random(u64);

impl Random {
    /// Returns the generator of one batch of one seed.
    pub fn new(seed: u64, batch: u64) -> Random {
        let mut random = Random(seed ^ batch.rotate_left(32) ^ 0x5EED_F022_0000_0000);
        random.next();
        random
    }

    /// Returns the next 64 random bits.
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a number below `n`, zero when `n` is zero.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next().checked_rem(n).unwrap_or(0)
    }

    /// Returns an index below `n`.
    pub fn index(&mut self, n: usize) -> usize {
        usize::try_from(self.below(n as u64)).unwrap_or(0)
    }

    /// Returns true `percent` times in a hundred.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// Returns one of `items`.
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.index(items.len())]
    }

    /// Returns a 32-bit value: a third of the time an edge, otherwise one of `usual` or a small number.
    pub fn u32(&mut self, usual: &[u32]) -> u32 {
        match self.below(6) {
            0 | 1 => self.pick(EDGES_32),
            2 => u32::try_from(self.below(9)).unwrap_or(0),
            _ if !usual.is_empty() => self.pick(usual),
            _ => u32::try_from(self.below(65)).unwrap_or(0),
        }
    }

    /// Returns a 64-bit value: a third of the time an edge, otherwise one of `usual` or a small multiple of four.
    pub fn u64(&mut self, usual: &[u64]) -> u64 {
        match self.below(6) {
            0 | 1 => self.pick(EDGES_64),
            2 => self.below(65).wrapping_mul(4),
            _ if !usual.is_empty() => self.pick(usual),
            _ => self.below(257),
        }
    }

    /// Returns a signed 32-bit value, at an edge or small.
    pub fn i32(&mut self) -> i32 {
        match self.below(4) {
            0 => self.pick(&[
                i32::MIN,
                i32::MIN + 1,
                -65536,
                -3,
                -1,
                i32::MAX - 1,
                i32::MAX,
            ]),
            _ => i32::try_from(self.below(9)).unwrap_or(0).wrapping_sub(4),
        }
    }

    /// Returns a float: a third of the time an edge, otherwise one of `usual`.
    pub fn f64(&mut self, usual: &[f64]) -> f64 {
        if usual.is_empty() || self.below(3) == 0 {
            self.pick(EDGES_FLOAT)
        } else {
            self.pick(usual)
        }
    }

    /// Returns a 32-bit float as `f64` returns it.
    pub fn f32(&mut self, usual: &[f64]) -> f32 {
        self.f64(usual) as f32
    }

    /// Returns bytes of a length from `lengths`, random.
    pub fn bytes(&mut self, lengths: &[usize]) -> Vec<u8> {
        let length = self.pick(lengths);
        (0..length).map(|_| self.next() as u8).collect()
    }
}
impl Random {
    /// Returns which of `fields` a call breaks, or none: most calls are valid but for one field.
    pub fn broken(&mut self, fields: u64) -> Option<u64> {
        self.chance(65).then(|| self.below(fields))
    }

    /// Returns a value past `most`, for a field whose sum with `other` must stay within it.
    ///
    /// It is one past, the largest, a power of two, or one whose sum with `other` wraps to a small number.
    pub fn past(&mut self, most: u32, other: u32) -> u32 {
        match self.below(5) {
            0 => most.saturating_add(1),
            1 => u32::MAX,
            // Only a sum with a field past zero wraps.
            2 if other > 0 => 0u32
                .wrapping_sub(other)
                .wrapping_add(self.below(u64::from(other).min(3)) as u32),
            2 => most.saturating_add(2),
            3 => most.saturating_add(other).saturating_add(1),
            _ => 1u32 << (31u64.saturating_sub(self.below(6)) as u32),
        }
    }

    /// Returns a 64-bit value past `most`, as `past` does.
    pub fn past_64(&mut self, most: u64, other: u64) -> u64 {
        match self.below(5) {
            0 => most.saturating_add(1),
            1 => u64::MAX,
            2 if other > 0 => 0u64
                .wrapping_sub(other)
                .wrapping_add(self.below(other.min(3))),
            2 => most.saturating_add(8),
            3 => most.saturating_add(other).saturating_add(4),
            _ => 1u64 << 63u64.saturating_sub(self.below(4)),
        }
    }
}
