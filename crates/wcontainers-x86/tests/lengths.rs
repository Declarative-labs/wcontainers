//! Every instruction length the reference CPU counted in the sweep `lengths.txt` (made by the test crate's
//! `x86-lengths`) is the decoder's: every opcode of every map, valid or not, after the prefixes and VEX and
//! EVEX fields that can change a length.

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn lengths_are_the_reference_cpus() {
    let sweep = include_str!("lengths.txt");
    let (mut checked, mut wrong) = (0, Vec::new());
    for line in sweep.lines().filter(|l| !l.starts_with('#')) {
        let [escape, filler, lengths] = line.split(' ').collect::<Vec<_>>()[..] else {
            panic!("a line of three fields: {line}");
        };
        let name = format!("{escape} {filler}");
        let (escape, filler) = (bytes(escape), bytes(filler));
        for (opcode, digit) in lengths.chars().enumerate() {
            let Some(expected) = digit.to_digit(16) else {
                continue;
            };
            let mut probe = escape.clone();
            probe.push(opcode as u8);
            probe.extend(&filler);
            probe.resize(16, 0xc0);
            let ours = wcontainers_x86::decode(&probe).map(|(_, length)| u32::from(length));
            checked += 1;
            if ours != Some(expected) {
                wrong.push(format!(
                    "{name} opcode {opcode:02x}: the CPU counts {expected}, we {ours:?}"
                ));
            }
        }
    }
    eprintln!("{checked} lengths checked, {} wrong", wrong.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(checked > 70_000);
}

/// Where the sweep has no length (an instruction that faults however short it is), the table counts what iced
/// counts: a lock prefix makes nearly every instruction invalid, so its length comes from the table, and it
/// must be one more than iced's length without the prefix.
#[test]
fn the_table_counts_valid_instructions_as_iced_does() {
    use iced_x86::{Decoder, DecoderOptions};
    let mut checked = 0;
    for context in [&[][..], &[0x66], &[0x48], &[0x67], &[0x66, 0x48]] {
        for map in [&[][..], &[0x0f], &[0x0f, 0x38], &[0x0f, 0x3a]] {
            for opcode in 0..=255u8 {
                for filler in [&[0xc0][..], &[0x04, 0x05], &[0xf8], &[0x40]] {
                    let mut plain = [context, map, &[opcode], filler].concat();
                    plain.resize(16, 0xc0);
                    let instruction =
                        Decoder::with_ip(64, &plain, 0, DecoderOptions::FORCE_RESERVED_NOP)
                            .decode();
                    if instruction.is_invalid()
                        || wcontainers_x86::claim_group(instruction.code())
                            == wcontainers_x86::claims::GROUP_NEVER
                    {
                        continue;
                    }
                    let mut locked = vec![0xf0];
                    locked.extend(&plain[..15]);
                    let ours = wcontainers_x86::decode(&locked).map(|(_, n)| usize::from(n));
                    assert_eq!(ours, Some(instruction.len() + 1), "f0 {plain:02x?}");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 9_000, "{checked}");
}
