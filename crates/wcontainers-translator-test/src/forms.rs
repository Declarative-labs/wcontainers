//! Every A64 instruction form, read from `arm/a64-forms.txt` (taken from Arm's specification).

/// One encoding of one instruction.
#[derive(Clone, Debug)]
pub struct Form {
    pub name: String,
    pub group: String,
    /// The features the form needs, or `true`.
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
}

pub fn all() -> Vec<Form> {
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
