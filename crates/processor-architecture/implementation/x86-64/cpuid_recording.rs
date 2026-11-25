//! Written by the `record_cpuid` example of wcontainers-behavior-source-kvm-single-step-x86-64.

use crate::CpuidLeaf;

pub static LEAVES: [CpuidLeaf; 13] = [
    CpuidLeaf { leaf: 0x0, subleaf: None, registers: [0x00000020, 0x756e6547, 0x6c65746e, 0x49656e69] },
    CpuidLeaf { leaf: 0x1, subleaf: None, registers: [0x000806f8, 0x00800800, 0xf7fab227, 0x0fabfbff] },
    CpuidLeaf { leaf: 0x7, subleaf: Some(0x0), registers: [0x00000002, 0xf1bf27eb, 0x1b415fee, 0xbfd14410] },
    CpuidLeaf { leaf: 0x7, subleaf: Some(0x1), registers: [0x00001c30, 0x00000000, 0x00000000, 0x00000000] },
    CpuidLeaf { leaf: 0x7, subleaf: Some(0xffffffff), registers: [0x00000000, 0x00000000, 0x00000000, 0x00000000] },
    CpuidLeaf { leaf: 0xd, subleaf: Some(0x0), registers: [0x000002e7, 0x00000240, 0x00000a88, 0x00000000] },
    CpuidLeaf { leaf: 0xd, subleaf: Some(0x1), registers: [0x0000001f, 0x00000240, 0x00001800, 0x00000000] },
    CpuidLeaf { leaf: 0xd, subleaf: Some(0xffffffff), registers: [0x00000000, 0x00000000, 0x00000000, 0x00000000] },
    CpuidLeaf { leaf: 0x20, subleaf: None, registers: [0x00000000, 0x00000000, 0x00000000, 0x00000000] },
    CpuidLeaf { leaf: 0x40000000, subleaf: None, registers: [0x40000001, 0x4b4d564b, 0x564b4d56, 0x0000004d] },
    CpuidLeaf { leaf: 0x80000000, subleaf: None, registers: [0x80000008, 0x00000000, 0x00000000, 0x00000000] },
    CpuidLeaf { leaf: 0x80000001, subleaf: None, registers: [0x00000000, 0x00000000, 0x00000121, 0x2c100800] },
    CpuidLeaf { leaf: 0x80000008, subleaf: None, registers: [0x002e392e, 0x0100d200, 0x00000000, 0x00000000] },
];
