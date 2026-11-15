//! Every A64 instruction form, read from `arm/a64-forms.txt` (taken from Arm's specification), and which of
//! them the CPU we present has.

use std::sync::OnceLock;

/// The optional features the CPU we present has, by Arm's names. They match the `AT_HWCAP` bits the kernel
/// reports (`wcontainers_abi::HWCAP`).
pub const OFFERED_FEATURES: [&str; 4] = ["FEAT_FP", "FEAT_AdvSIMD", "FEAT_CRC32", "FEAT_LSE"];

/// One encoding of one instruction.
#[derive(Clone, Debug)]
pub struct Form {
    pub name: String,
    pub group: String,
    /// The condition under which the form exists: Arm's expression over `FEAT_` names and fields, or `true`.
    pub features: String,
    pub mask: u32,
    pub value: u32,
    /// Bits the specification says should hold a fixed value but which decode either way.
    pub should_mask: u32,
    pub should_value: u32,
    /// Named fields: name, lowest bit, width.
    pub fields: Vec<(String, u32, u32)>,
}

impl Form {
    pub fn field(&self, word: u32, name: &str) -> Option<u32> {
        self.fields
            .iter()
            .find(|(field, ..)| field == name)
            .map(|&(_, low, width)| (word >> low) & ((1 << width) - 1))
    }

    /// A word of this form with the given bits in every free position.
    pub fn word(&self, free: u32) -> u32 {
        (self.value & self.mask)
            | (self.should_value & self.should_mask)
            | (free & !self.mask & !self.should_mask)
    }

    /// Whether this word is this form on the CPU we present: its fixed bits match and its condition holds.
    pub fn presented(&self, word: u32) -> bool {
        word & self.mask == self.value && Condition { form: self, word }.holds()
    }

    /// Whether this form's words are an instruction of the CPU we present even without the form's feature,
    /// because the form was taken from a hint space that does nothing.
    pub fn hint_without_feature(&self) -> bool {
        HINT_CARVE_OUTS.contains(&self.name.as_str())
    }
}

/// Every form, read once.
pub fn all() -> &'static [Form] {
    static FORMS: OnceLock<Vec<Form>> = OnceLock::new();
    FORMS.get_or_init(read)
}

/// Forms Arm placed in a hint space, which a CPU without their feature runs as that hint: RPRFM takes
/// PRFM (register) prefetch operations that Arm reserved, and reserved prefetch operations do nothing.
const HINT_CARVE_OUTS: [&str; 1] = ["RPRFM_R_ldst_regoff"];

/// Whether a word is an instruction of the CPU we present.
pub fn presented(word: u32) -> bool {
    candidates(word).iter().any(|&i| {
        let form = &all()[usize::from(i)];
        form.presented(word) || (form.hint_without_feature() && word & form.mask == form.value)
    })
}

/// The bits of a word that pick its bucket of candidate forms: bits 31 to 21.
const KEY_LOW: u32 = 21;
const KEY_BUCKETS: u32 = 1 << (32 - KEY_LOW);

/// The forms whose fixed bits in the key agree with the word's.
fn candidates(word: u32) -> &'static [u16] {
    static INDEX: OnceLock<Vec<Vec<u16>>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        (0..KEY_BUCKETS)
            .map(|key| {
                let word = key << KEY_LOW;
                let key_mask = u32::MAX << KEY_LOW;
                (0..all().len())
                    .filter(|&i| (word ^ all()[i].value) & all()[i].mask & key_mask == 0)
                    .map(|i| u16::try_from(i).expect("fewer than 65536 forms"))
                    .collect()
            })
            .collect()
    });
    &index[(word >> KEY_LOW) as usize]
}

fn read() -> Vec<Form> {
    include_str!("../arm/a64-forms.txt")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let mut parts = line.split(' ');
            let mut next = || parts.next().expect("a complete line");
            let name = next().to_string();
            let group = next().to_string();
            let features = next().to_string();
            let mut hex = || u32::from_str_radix(&next()[2..], 16).expect("hexadecimal");
            let (mask, value, should_mask, should_value) = (hex(), hex(), hex(), hex());
            let fields = parts
                .map(|field| {
                    let mut pieces = field.split(':');
                    let name = pieces.next().unwrap().to_string();
                    let low = pieces.next().unwrap().parse().unwrap();
                    let width = pieces.next().unwrap().parse().unwrap();
                    (name, low, width)
                })
                .collect();
            Form {
                name,
                group,
                features,
                mask,
                value,
                should_mask,
                should_value,
                fields,
            }
        })
        .collect()
}

/// Evaluates a form's condition for one word. The grammar Arm uses there: `!`, `&&`, `||`, brackets,
/// `true`, `FEAT_` names, and `Field=='bits'`, `Field!='bits'`, `FieldIN{'bits',...}` with `x` for any bit.
struct Condition<'a> {
    form: &'a Form,
    word: u32,
}

impl Condition<'_> {
    fn holds(&self) -> bool {
        let mut rest = self.form.features.as_str();
        let value = self.or(&mut rest);
        assert!(rest.is_empty(), "unparsed condition {}", self.form.features);
        value
    }

    fn or(&self, rest: &mut &str) -> bool {
        let mut value = self.and(rest);
        while let Some(after) = rest.strip_prefix("||") {
            *rest = after;
            value |= self.and(rest);
        }
        value
    }

    fn and(&self, rest: &mut &str) -> bool {
        let mut value = self.not(rest);
        while let Some(after) = rest.strip_prefix("&&") {
            *rest = after;
            value &= self.not(rest);
        }
        value
    }

    fn not(&self, rest: &mut &str) -> bool {
        if let Some(after) = rest.strip_prefix('!').filter(|a| !a.starts_with('=')) {
            *rest = after;
            return !self.not(rest);
        }
        if let Some(after) = rest.strip_prefix('(') {
            *rest = after;
            let value = self.or(rest);
            *rest = rest.strip_prefix(')').expect("a closing bracket");
            return value;
        }
        self.atom(rest)
    }

    fn atom(&self, rest: &mut &str) -> bool {
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let (mut name, after) = rest.split_at(end);
        *rest = after;
        // `FieldIN{` reads as one name.
        if rest.starts_with('{') {
            name = name.strip_suffix("IN").expect("IN before a set");
            *rest = &rest[1..];
        }
        if name == "true" {
            return true;
        }
        if name.starts_with("FEAT_") {
            return OFFERED_FEATURES.contains(&name);
        }
        let field = self
            .form
            .field(self.word, name)
            .unwrap_or_else(|| panic!("no field {name} in {}", self.form.name));
        let matches = |pattern: &str| {
            pattern
                .chars()
                .rev()
                .enumerate()
                .all(|(bit, c)| c == 'x' || u32::from(c == '1') == (field >> bit) & 1)
        };
        if let Some(after) = rest.strip_prefix("==") {
            *rest = after;
            matches(pattern(rest))
        } else if let Some(after) = rest.strip_prefix("!=") {
            *rest = after;
            !matches(pattern(rest))
        } else if after.starts_with('{') {
            let mut any = false;
            loop {
                any |= matches(pattern(rest));
                match rest.strip_prefix(',') {
                    Some(after) => *rest = after,
                    None => break,
                }
            }
            *rest = rest.strip_prefix('}').expect("a closing brace");
            any
        } else {
            panic!("unknown condition at {rest} in {}", self.form.features)
        }
    }
}

/// Take one quoted bit pattern.
fn pattern<'a>(rest: &mut &'a str) -> &'a str {
    let quoted = rest.strip_prefix('\'').expect("a quoted pattern");
    let close = quoted.find('\'').expect("a closing quote");
    *rest = &quoted[close + 1..];
    &quoted[..close]
}
