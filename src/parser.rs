use crate::cpu::Instruction;
use std::collections::{BTreeMap, LinkedList};

struct Parser {
    instruction_length_map: BTreeMap<u8, u8>,
}

impl Parser {
    pub fn new() -> Parser {
        let mut out = Parser {
            instruction_length_map: BTreeMap::new(),
        };

        out.instruction_length_map.insert(0x69, 2);
        out.instruction_length_map.insert(0x65, 2);
        out.instruction_length_map.insert(0x75, 2);
        out.instruction_length_map.insert(0x6D, 3);
        out.instruction_length_map.insert(0x7D, 3);
        out.instruction_length_map.insert(0x79, 3);
        out.instruction_length_map.insert(0x61, 2);
        out.instruction_length_map.insert(0x71, 2);

        out.instruction_length_map.insert(0x29, 2);
        out.instruction_length_map.insert(0x25, 2);
        out.instruction_length_map.insert(0x35, 2);
        out.instruction_length_map.insert(0x2D, 3);
        out.instruction_length_map.insert(0x3D, 3);
        out.instruction_length_map.insert(0x39, 3);
        out.instruction_length_map.insert(0x21, 2);
        out.instruction_length_map.insert(0x31, 2);

        out.instruction_length_map.insert(0x0A, 1);
        out.instruction_length_map.insert(0x06, 2);
        out.instruction_length_map.insert(0x16, 2);
        out.instruction_length_map.insert(0x0E, 3);
        out.instruction_length_map.insert(0x1E, 3);

        out.instruction_length_map.insert(0x90, 2);

        out.instruction_length_map.insert(0xB0, 2);

        out.instruction_length_map.insert(0xF0, 2);

        out.instruction_length_map.insert(0x24, 2);
        out.instruction_length_map.insert(0x2C, 3);

        out.instruction_length_map.insert(0x30, 2);

        out.instruction_length_map.insert(0xD0, 2);

        out.instruction_length_map.insert(0x10, 2);

        out.instruction_length_map.insert(0x00, 2);

        out.instruction_length_map.insert(0x50, 2);

        out.instruction_length_map.insert(0x70, 2);

        out.instruction_length_map.insert(0x18, 1);

        out.instruction_length_map.insert(0xD8, 1);

        out.instruction_length_map.insert(0x58, 1);

        out.instruction_length_map.insert(0xB8, 1);

        out.instruction_length_map.insert(0xC9, 2);
        out.instruction_length_map.insert(0xC5, 2);
        out.instruction_length_map.insert(0xD5, 2);
        out.instruction_length_map.insert(0xCD, 3);
        out.instruction_length_map.insert(0xDD, 3);
        out.instruction_length_map.insert(0xD9, 3);
        out.instruction_length_map.insert(0xC1, 2);
        out.instruction_length_map.insert(0xD1, 2);

        out.instruction_length_map.insert(0xE0, 2);
        out.instruction_length_map.insert(0xE4, 2);
        out.instruction_length_map.insert(0xEC, 3);

        out.instruction_length_map.insert(0xC0, 2);
        out.instruction_length_map.insert(0xC4, 2);
        out.instruction_length_map.insert(0xCC, 3);

        out.instruction_length_map.insert(0xC6, 2);
        out.instruction_length_map.insert(0xD6, 2);
        out.instruction_length_map.insert(0xCE, 3);
        out.instruction_length_map.insert(0xDE, 3);

        out.instruction_length_map.insert(0xCA, 1);

        out.instruction_length_map.insert(0x88, 1);

        out.instruction_length_map.insert(0x49, 2);
        out.instruction_length_map.insert(0x45, 2);
        out.instruction_length_map.insert(0x55, 2);
        out.instruction_length_map.insert(0x4D, 3);
        out.instruction_length_map.insert(0x5D, 3);
        out.instruction_length_map.insert(0x59, 3);
        out.instruction_length_map.insert(0x41, 2);
        out.instruction_length_map.insert(0x51, 2);

        out.instruction_length_map.insert(0xE6, 2);
        out.instruction_length_map.insert(0xF6, 2);
        out.instruction_length_map.insert(0xEE, 3);
        out.instruction_length_map.insert(0xFE, 3);

        out.instruction_length_map.insert(0xE8, 1);

        out.instruction_length_map.insert(0xC8, 1);

        out.instruction_length_map.insert(0x4C, 3);
        out.instruction_length_map.insert(0x6C, 3);

        out.instruction_length_map.insert(0x20, 3);

        out.instruction_length_map.insert(0xA9, 2);
        out.instruction_length_map.insert(0xA5, 2);
        out.instruction_length_map.insert(0xB5, 2);
        out.instruction_length_map.insert(0xAD, 3);
        out.instruction_length_map.insert(0xBD, 3);
        out.instruction_length_map.insert(0xB9, 3);
        out.instruction_length_map.insert(0xA1, 2);
        out.instruction_length_map.insert(0xB1, 2);

        out.instruction_length_map.insert(0xA2, 2);
        out.instruction_length_map.insert(0xA6, 2);
        out.instruction_length_map.insert(0xB6, 2);
        out.instruction_length_map.insert(0xAE, 3);
        out.instruction_length_map.insert(0xBE, 3);

        out.instruction_length_map.insert(0xA0, 2);
        out.instruction_length_map.insert(0xA4, 2);
        out.instruction_length_map.insert(0xB4, 2);
        out.instruction_length_map.insert(0xAC, 3);
        out.instruction_length_map.insert(0xBC, 3);

        out.instruction_length_map.insert(0x4A, 1);
        out.instruction_length_map.insert(0x46, 2);
        out.instruction_length_map.insert(0x56, 2);
        out.instruction_length_map.insert(0x4E, 3);
        out.instruction_length_map.insert(0x5E, 3);

        out.instruction_length_map.insert(0xEA, 1);

        out.instruction_length_map.insert(0x09, 2);
        out.instruction_length_map.insert(0x05, 2);
        out.instruction_length_map.insert(0x15, 2);
        out.instruction_length_map.insert(0x0D, 3);
        out.instruction_length_map.insert(0x1D, 3);
        out.instruction_length_map.insert(0x19, 3);
        out.instruction_length_map.insert(0x01, 2);
        out.instruction_length_map.insert(0x11, 2);

        out.instruction_length_map.insert(0x48, 1);

        out.instruction_length_map.insert(0x08, 1);

        out.instruction_length_map.insert(0x68, 1);

        out.instruction_length_map.insert(0x28, 1);

        out.instruction_length_map.insert(0x2A, 1);
        out.instruction_length_map.insert(0x26, 2);
        out.instruction_length_map.insert(0x36, 2);
        out.instruction_length_map.insert(0x2E, 3);
        out.instruction_length_map.insert(0x3E, 3);

        out.instruction_length_map.insert(0x6A, 1);
        out.instruction_length_map.insert(0x66, 2);
        out.instruction_length_map.insert(0x76, 2);
        out.instruction_length_map.insert(0x6E, 3);
        out.instruction_length_map.insert(0x7E, 3);

        out.instruction_length_map.insert(0x40, 1);

        out.instruction_length_map.insert(0x60, 1);

        out.instruction_length_map.insert(0xE9, 2);
        out.instruction_length_map.insert(0xE5, 2);
        out.instruction_length_map.insert(0xF5, 2);
        out.instruction_length_map.insert(0xED, 3);
        out.instruction_length_map.insert(0xFD, 3);
        out.instruction_length_map.insert(0xF9, 3);
        out.instruction_length_map.insert(0xE1, 2);
        out.instruction_length_map.insert(0xF1, 2);

        out.instruction_length_map.insert(0x38, 1);

        out.instruction_length_map.insert(0xF8, 1);

        out.instruction_length_map.insert(0x78, 1);

        out.instruction_length_map.insert(0x85, 2);
        out.instruction_length_map.insert(0x95, 2);
        out.instruction_length_map.insert(0x8D, 3);
        out.instruction_length_map.insert(0x9D, 3);
        out.instruction_length_map.insert(0x99, 3);
        out.instruction_length_map.insert(0x81, 2);
        out.instruction_length_map.insert(0x91, 2);

        out.instruction_length_map.insert(0x86, 2);
        out.instruction_length_map.insert(0x96, 2);
        out.instruction_length_map.insert(0x8E, 3);

        out.instruction_length_map.insert(0x84, 2);
        out.instruction_length_map.insert(0x94, 2);
        out.instruction_length_map.insert(0x8C, 3);

        out.instruction_length_map.insert(0xAA, 1);

        out.instruction_length_map.insert(0xA8, 1);

        out.instruction_length_map.insert(0xBA, 1);

        out.instruction_length_map.insert(0x8A, 1);

        out.instruction_length_map.insert(0x9A, 1);

        out.instruction_length_map.insert(0x98, 1);

        return out;
    }

    pub fn parse_to_instructions(bytes: &[u8], instructions: &LinkedList<Instruction>) {
        // TODO: insert all Instructions into the provided list
    }
}
