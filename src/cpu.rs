pub struct Cpu {
    memory: [u8; 0xFFFF],
    program_counter: u16,
    /// Initially starts at 255. Each push decreases this value by one, each pop increases it.
    stack_pointer: u8,
    accumulator: u8,
    index_x: u8,
    index_y: u8,
    /// Bit locations: <pre>
    /// 1 << 6 => carry flag
    /// 1 << 5 => zero flag
    /// 1 << 4 => interrupt disable
    /// 1 << 3 => decimal mode
    /// 1 << 2 => break command
    /// 1 << 1 => overflow flag
    /// 1      => negative flag
    /// </pre>
    processor_status: u8,

    cycle: u32,

    change_interrupt_disable_flag: i8,
}

/// Instruction reference: https://www.nesdev.org/wiki/Instruction_reference
pub struct Instruction {
    pub op_code: u8,
    pub arguments: [u8; 2],
    // the number of bytes this instruction is long; used for incrementing the program counter
    pub size: u8,
}

impl Instruction {
    pub fn new(op_code: u8, arguments: [u8; 2], size: u8) -> Instruction {
        Instruction {
            op_code,
            arguments,
            size,
        }
    }

    fn get_absolute_addr(&self) -> u16 {
        (self.arguments[0] as u16) << 8 | self.arguments[1] as u16
    }
}

const BRANCHING_OP_CODES: [u8; 14] = [
    0x90, 0xB0, 0xF0, 0x30, 0xD0, 0x10, 0x00, 0x50, 0x70, 0x4C, 0x6C, 0x20, 0x40, 0x60,
];

fn increment_if_crossed_absolute(base: u32, addr: u16, inc: u8) -> u32 {
    if ((addr + inc as u16) & 0xFF00) == (addr & 0xFF00) {
        base
    } else {
        base + 1
    }
}

fn increment_if_crossed_indirect_indexed(base: u32, addr: u8, cpu: &Cpu) -> u32 {
    let indirect_indexed: u16 = cpu.get_addr_indirect_indexed_index(addr) as u16;
    if ((indirect_indexed - cpu.index_y as u16) & 0xFF00) == (indirect_indexed & 0xFF00) {
        base
    } else {
        base + 1
    }
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            memory: [0; 65535],
            program_counter: 0,
            stack_pointer: 0xFF,
            accumulator: 0,
            index_x: 0,
            index_y: 0,
            processor_status: 0,
            cycle: 0,
            change_interrupt_disable_flag: -1,
        }
    }

    pub fn execute_instruction(&mut self, inst: &Instruction) {
        if self.change_interrupt_disable_flag != -1 {
            self.set_flag_interrupt(self.change_interrupt_disable_flag != 0);
            self.change_interrupt_disable_flag = -1;
        }

        // branching instructions are special, as they modify the program counter directly instead
        // of simply incrementing it by one. They should be handled first
        if BRANCHING_OP_CODES.contains(&inst.op_code) {
            self.program_counter = match inst.op_code {
                0x90 => self.branch_if_condition(inst.arguments[0], !self.get_flag_carry()),
                0xB0 => self.branch_if_condition(inst.arguments[0], self.get_flag_carry()),

                0xF0 => self.branch_if_condition(inst.arguments[0], self.get_flag_zero()),
                0xD0 => self.branch_if_condition(inst.arguments[0], !self.get_flag_zero()),

                0x30 => self.branch_if_condition(inst.arguments[0], self.get_flag_negative()),
                0x10 => self.branch_if_condition(inst.arguments[0], !self.get_flag_negative()),

                0x70 => self.branch_if_condition(inst.arguments[0], self.get_flag_overflow()),
                0x50 => self.branch_if_condition(inst.arguments[0], !self.get_flag_overflow()),

                0x00 => {
                    // TODO implement the brk hardware bug
                    let val: u16 = self.program_counter + 2;
                    let bytes: [u8; 2] = val.to_be_bytes();
                    self.push(bytes[0]);
                    self.push(bytes[1]);

                    self.push(self.get_processor_status());
                    self.set_flag_interrupt(true);

                    self.cycle += 7;
                    0xFFFE
                }

                0x4C => {
                    self.cycle += 3;
                    self.get_addr_absolute(inst.get_absolute_addr()) as u16
                }
                0x6C => panic!("Indirect jmp instruction is not supported yet."), // TODO: Implement this

                0x20 => {
                    // jsr
                    let val: u16 = self.program_counter + 2;
                    let bytes: [u8; 2] = val.to_be_bytes();
                    self.push(bytes[0]);
                    self.push(bytes[1]);
                    self.cycle += 6;
                    self.get_addr_absolute(inst.get_absolute_addr()) as u16
                }

                0x40 => {
                    let flags: u8 = self.pop();
                    self.set_processor_status(flags, false);

                    let low: u8 = self.pop();
                    let high: u8 = self.pop();
                    self.cycle += 6;
                    u16::from_be_bytes([high, low])
                }
                0x60 => {
                    let low: u8 = self.pop();
                    let high: u8 = self.pop();
                    self.cycle += 6;
                    u16::from_be_bytes([high, low]) + 1
                }

                _ => panic!(
                    "Not implemented branching op code received: {}",
                    inst.op_code
                ),
            };
            return;
        }

        match inst.op_code {
            0x69 => self.execute_adc(inst.arguments[0], 2),
            0x65 => self.execute_adc(self.get_addr_zero(inst.arguments[0]), 3),
            0x75 => self.execute_adc(self.get_addr_zero_x(inst.arguments[0]), 4),
            0x6D => self.execute_adc(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0x7D => self.execute_adc(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0x79 => self.execute_adc(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0x61 => self.execute_adc(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0x71 => self.execute_adc(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0x29 => self.execute_and(inst.arguments[0], 2),
            0x25 => self.execute_and(self.get_addr_zero(inst.arguments[0]), 3),
            0x35 => self.execute_and(self.get_addr_zero_x(inst.arguments[0]), 4),
            0x2D => self.execute_and(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0x3D => self.execute_and(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0x39 => self.execute_and(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0x21 => self.execute_and(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0x31 => self.execute_and(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0x0A => self.execute_asl(self.accumulator, |cpu, r| -> () { cpu.accumulator = r }, 2),
            0x06 => self.execute_asl(
                self.get_addr_zero(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero(inst.arguments[0], r) },
                5,
            ),
            0x16 => self.execute_asl(
                self.get_addr_zero_x(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero_x(inst.arguments[0], r) },
                6,
            ),
            0x0E => self.execute_asl(
                self.get_addr_absolute(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute(inst.get_absolute_addr(), r) },
                6,
            ),
            0x1E => self.execute_asl(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute_x(inst.get_absolute_addr(), r) },
                7,
            ),

            0x24 => self.execute_bit(self.get_addr_zero(inst.arguments[0]), 3),
            0x2C => self.execute_bit(self.get_addr_absolute(inst.get_absolute_addr()), 4),

            0x18 => {
                self.set_flag_carry(false);
                self.cycle += 2;
            }
            0xD8 => {
                self.set_flag_decimal(false);
                self.cycle += 2;
            }
            0x58 => {
                self.change_interrupt_disable_flag = 0;
                self.cycle += 2;
            }
            0xB8 => {
                self.set_flag_overflow(false);
                self.cycle += 2;
            }

            0xC9 => self.execute_cmp(inst.arguments[0], 2),
            0xC5 => self.execute_cmp(self.get_addr_zero(inst.arguments[0]), 3),
            0xD5 => self.execute_cmp(self.get_addr_zero_x(inst.arguments[0]), 4),
            0xCD => self.execute_cmp(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0xDD => self.execute_cmp(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0xD9 => self.execute_cmp(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0xC1 => self.execute_cmp(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0xD1 => self.execute_cmp(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0xE0 => self.execute_cmx(inst.arguments[0], 2),
            0xE4 => self.execute_cmx(self.get_addr_zero(inst.arguments[0]), 3),
            0xEC => self.execute_cmx(self.get_addr_absolute(inst.get_absolute_addr()), 4),

            0xC0 => self.execute_cmy(inst.arguments[0], 2),
            0xC4 => self.execute_cmy(self.get_addr_zero(inst.arguments[0]), 3),
            0xCC => self.execute_cmy(self.get_addr_absolute(inst.get_absolute_addr()), 4),

            0xC6 => self.execute_dec(self.get_addr_zero_index(inst.arguments[0]) as u16, 5),
            0xD6 => self.execute_dec(self.get_addr_zero_x_index(inst.arguments[0]) as u16, 6),
            0xCE => self.execute_dec(inst.get_absolute_addr(), 6),
            0xDE => self.execute_dec(inst.get_absolute_addr() + self.index_x as u16, 7),

            0xCA => {
                // dex
                self.index_x -= 1;
                self.set_flag_zero_by_val(self.index_x);
                self.set_flag_negative_by_val(self.index_x);
                self.cycle += 2;
            }
            0x88 => {
                // dey
                self.index_y -= 1;
                self.set_flag_zero_by_val(self.index_y);
                self.set_flag_negative_by_val(self.index_y);
                self.cycle += 2;
            }

            0x49 => self.execute_eor(inst.arguments[0], 2),
            0x45 => self.execute_eor(self.get_addr_zero(inst.arguments[0]), 3),
            0x55 => self.execute_eor(self.get_addr_zero_x(inst.arguments[0]), 4),
            0x4D => self.execute_eor(self.get_addr_absolute(inst.get_absolute_addr()), 5),
            0x5D => self.execute_eor(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0x59 => self.execute_eor(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0x41 => self.execute_eor(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0x51 => self.execute_eor(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0xE6 => self.execute_inc(self.get_addr_zero_index(inst.arguments[0]) as u16, 5),
            0xF6 => self.execute_inc(self.get_addr_zero_x_index(inst.arguments[0]) as u16, 6),
            0xEE => self.execute_inc(inst.get_absolute_addr(), 6),
            0xFE => self.execute_inc(inst.get_absolute_addr() + self.index_x as u16, 7),

            0xE8 => {
                // inx
                self.index_x += 1;
                self.set_flag_zero_by_val(self.index_x);
                self.set_flag_negative_by_val(self.index_x);
                self.cycle += 2;
            }
            0xC8 => {
                // iny
                self.index_y += 1;
                self.set_flag_zero_by_val(self.index_y);
                self.set_flag_negative_by_val(self.index_y);
                self.cycle += 2;
            }

            0xA9 => self.execute_lda(inst.arguments[0], 2),
            0xA5 => self.execute_lda(self.get_addr_zero(inst.arguments[0]), 3),
            0xB5 => self.execute_lda(self.get_addr_zero_x(inst.arguments[0]), 4),
            0xAD => self.execute_lda(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0xBD => self.execute_lda(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0xB9 => self.execute_lda(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0xA1 => self.execute_lda(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0xB1 => self.execute_lda(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0xA2 => self.execute_ldx(inst.arguments[0], 2),
            0xA6 => self.execute_ldx(self.get_addr_zero(inst.arguments[0]), 3),
            0xB6 => self.execute_ldx(self.get_addr_zero_y(inst.arguments[0]), 4),
            0xAE => self.execute_ldx(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0xBE => self.execute_ldx(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),

            0xA0 => self.execute_ldy(inst.arguments[0], 2),
            0xA4 => self.execute_ldy(self.get_addr_zero(inst.arguments[0]), 3),
            0xB4 => self.execute_ldy(self.get_addr_zero_x(inst.arguments[0]), 4),
            0xAC => self.execute_ldy(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0xBC => self.execute_ldy(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),

            0x4A => self.execute_lsr(self.accumulator, |cpu, r| -> () { cpu.accumulator = r }, 2),
            0x46 => self.execute_lsr(
                self.get_addr_zero(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero(inst.arguments[0], r) },
                5,
            ),
            0x56 => self.execute_lsr(
                self.get_addr_zero_x(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero_x(inst.arguments[0], r) },
                5,
            ),
            0x4E => self.execute_lsr(
                self.get_addr_absolute(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute(inst.get_absolute_addr(), r) },
                5,
            ),
            0x5E => self.execute_lsr(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute_x(inst.get_absolute_addr(), r) },
                5,
            ),

            0xEA => self.cycle += 2, // nop

            0x09 => self.execute_ora(inst.arguments[0], 2),
            0x05 => self.execute_ora(self.get_addr_zero(inst.arguments[0]), 3),
            0x15 => self.execute_ora(self.get_addr_zero_x(inst.arguments[0]), 4),
            0x0D => self.execute_ora(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0x1D => self.execute_ora(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0x19 => self.execute_ora(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0x01 => self.execute_ora(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0x11 => self.execute_ora(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0x48 => {
                self.push(self.accumulator);
                self.cycle += 3;
            }
            0x08 => {
                self.push(self.get_processor_status());
                self.cycle += 3;
            }
            0x68 => {
                self.accumulator = self.pop();
                self.set_flag_zero_by_val(self.accumulator);
                self.set_flag_negative_by_val(self.accumulator);
                self.cycle += 4;
            }

            0x28 => {
                let val: u8 = self.pop();
                self.set_processor_status(val, true);
            }

            0x2A => self.execute_rol(self.accumulator, |cpu, r| -> () { cpu.accumulator = r }, 2),
            0x26 => self.execute_rol(
                self.get_addr_zero(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero(inst.arguments[0], r) },
                5,
            ),
            0x36 => self.execute_rol(
                self.get_addr_zero_x(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero_x(inst.arguments[0], r) },
                5,
            ),
            0x2E => self.execute_rol(
                self.get_addr_absolute(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute(inst.get_absolute_addr(), r) },
                6,
            ),
            0x3E => self.execute_rol(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute_x(inst.get_absolute_addr(), r) },
                6,
            ),

            0x6A => self.execute_ror(self.accumulator, |cpu, r| -> () { cpu.accumulator = r }, 2),
            0x66 => self.execute_ror(
                self.get_addr_zero(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero(inst.arguments[0], r) },
                5,
            ),
            0x76 => self.execute_ror(
                self.get_addr_zero_x(inst.arguments[0]),
                |cpu, r| -> () { cpu.set_addr_zero_x(inst.arguments[0], r) },
                5,
            ),
            0x6E => self.execute_ror(
                self.get_addr_absolute(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute(inst.get_absolute_addr(), r) },
                6,
            ),
            0x7E => self.execute_ror(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute_x(inst.get_absolute_addr(), r) },
                6,
            ),

            0xE9 => self.execute_sbc(inst.arguments[0], 2),
            0xE5 => self.execute_sbc(self.get_addr_zero(inst.arguments[0]), 3),
            0xF5 => self.execute_sbc(self.get_addr_zero_x(inst.arguments[0]), 4),
            0xED => self.execute_sbc(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0xFD => self.execute_sbc(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_x),
            ),
            0xF9 => self.execute_sbc(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed_absolute(4, inst.get_absolute_addr(), self.index_y),
            ),
            0xE1 => self.execute_sbc(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0xF1 => self.execute_sbc(
                self.get_addr_indirect_indexed(inst.arguments[0]),
                increment_if_crossed_indirect_indexed(5, inst.arguments[0], self),
            ),

            0x38 => {
                self.set_flag_carry(true);
                self.cycle += 2;
            }
            0xF8 => {
                self.set_flag_decimal(true);
                self.cycle += 2;
            }
            0x78 => {
                self.change_interrupt_disable_flag = 1;
                self.cycle += 2;
            }

            0x85 => self.execute_st(
                self.get_addr_zero_index(inst.arguments[0]) as u16,
                self.accumulator,
                3,
            ),
            0x95 => self.execute_st(
                self.get_addr_zero_x_index(inst.arguments[0]) as u16,
                self.accumulator,
                4,
            ),
            0x8D => self.execute_st(inst.get_absolute_addr(), self.accumulator, 4),
            0x9D => self.execute_st(
                inst.get_absolute_addr() + self.index_x as u16,
                self.accumulator,
                5,
            ),
            0x99 => self.execute_st(
                inst.get_absolute_addr() + self.index_y as u16,
                self.accumulator,
                5,
            ),
            0x81 => self.execute_st(
                self.get_addr_indexed_indirect_index(inst.arguments[0]) as u16,
                self.accumulator,
                6,
            ),
            0x91 => self.execute_st(
                self.get_addr_indirect_indexed_index(inst.arguments[0]) as u16,
                self.accumulator,
                6,
            ),

            0x86 => self.execute_st(
                self.get_addr_zero_index(inst.arguments[0]) as u16,
                self.index_x,
                3,
            ),
            0x96 => self.execute_st(
                self.get_addr_zero_y_index(inst.arguments[0]) as u16,
                self.index_x,
                4,
            ),
            0x8E => self.execute_st(inst.get_absolute_addr(), self.index_x, 4),

            0x84 => self.execute_st(
                self.get_addr_zero_index(inst.arguments[0]) as u16,
                self.index_y,
                3,
            ),
            0x94 => self.execute_st(
                self.get_addr_zero_x_index(inst.arguments[0]) as u16,
                self.index_y,
                4,
            ),
            0x8C => self.execute_st(inst.get_absolute_addr(), self.index_y, 4),

            0xAA => {
                self.index_x = self.accumulator;
                self.set_flag_zero_by_val(self.index_x);
                self.set_flag_negative_by_val(self.index_x);
                self.cycle += 2;
            }
            0xA8 => {
                self.index_y = self.accumulator;
                self.set_flag_zero_by_val(self.index_y);
                self.set_flag_negative_by_val(self.index_y);
                self.cycle += 2;
            }
            0xBA => {
                self.index_x = self.stack_pointer;
                self.set_flag_zero_by_val(self.index_y);
                self.set_flag_negative_by_val(self.index_y);
                self.cycle += 2;
            }
            0x8A => {
                self.accumulator = self.index_x;
                self.set_flag_zero_by_val(self.accumulator);
                self.set_flag_negative_by_val(self.accumulator);
                self.cycle += 2;
            }
            0x9A => {
                self.stack_pointer = self.index_x;
                self.cycle += 2;
            }
            0x98 => {
                self.accumulator = self.index_y;
                self.set_flag_zero_by_val(self.accumulator);
                self.set_flag_negative_by_val(self.accumulator);
                self.cycle += 2;
            }

            _ => panic!("Unknown op code received: {}", inst.op_code),
        };
        self.program_counter += inst.size as u16
    }

    fn execute_adc(&mut self, memory: u8, cycles: u32) {
        let result: u16 = self.accumulator as u16
            + memory as u16
            + (if self.get_flag_carry() { 1 } else { 0 }) as u16;
        self.set_flag_carry_by_val(result);
        self.set_flag_zero_by_val(result as u8);
        self.set_flag_overflow(
            (result ^ (self.accumulator as u16)) & (result ^ (memory as u16)) & 0x80 == 0x80,
        );
        self.set_flag_negative_by_val(result as u8);
        self.accumulator = (result & 0xFF) as u8;
        self.cycle += cycles
    }

    fn execute_and(&mut self, memory: u8, cycles: u32) {
        let result: u8 = self.accumulator & memory;
        self.set_flag_zero_by_val(result);
        self.set_flag_negative_by_val(result);
        self.accumulator = result;
        self.cycle += cycles
    }

    fn execute_asl<R>(&mut self, value: u8, r: R, cycles: u32)
    where
        R: Fn(&mut Cpu, u8),
    {
        let result: u8 = value << 1;
        self.set_flag_carry((value >> 7) & 1 == 1);
        self.set_flag_zero(result == 0);
        self.set_flag_negative_by_val(result);
        r(self, result);
        self.cycle += cycles
    }

    fn branch_if_condition(&mut self, value: u8, condition: bool) -> u16 {
        self.cycle += 2;
        if !condition {
            self.program_counter + 2
        } else {
            let new_pc = self.program_counter + 2 + value.cast_signed() as u16;
            self.cycle += if new_pc & 0xFF00 == self.program_counter & 0xFF00 {
                1
            } else {
                2
            };
            new_pc
        }
    }

    fn execute_bit(&mut self, value: u8, cycles: u32) {
        let result: u8 = self.accumulator & value;
        self.set_flag_zero_by_val(result);
        self.set_flag_overflow_by_val(result);
        self.set_flag_negative_by_val(result);

        self.cycle += cycles;
    }

    fn execute_cmp(&mut self, value: u8, cycles: u32) {
        self.set_flag_carry(self.accumulator >= value);
        self.set_flag_zero(self.accumulator == value);
        self.set_flag_negative(self.accumulator < value);

        self.cycle += cycles
    }

    fn execute_cmx(&mut self, value: u8, cycles: u32) {
        self.set_flag_carry(self.index_x >= value);
        self.set_flag_zero(self.index_x == value);
        self.set_flag_negative(self.index_x < value);

        self.cycle += cycles
    }

    fn execute_cmy(&mut self, value: u8, cycles: u32) {
        self.set_flag_carry(self.index_y >= value);
        self.set_flag_zero(self.index_y == value);
        self.set_flag_negative(self.index_y < value);

        self.cycle += cycles
    }

    fn execute_dec(&mut self, addr: u16, cycles: u32) {
        let result: u8 = self.memory[addr as usize] - 1;
        self.memory[addr as usize] = result;
        self.set_flag_zero_by_val(result);
        self.set_flag_negative_by_val(result);
        self.cycle += cycles;
    }

    fn execute_eor(&mut self, value: u8, cycles: u32) {
        self.accumulator ^= value;
        self.set_flag_zero_by_val(self.accumulator);
        self.set_flag_negative_by_val(self.accumulator);
        self.cycle += cycles;
    }

    fn execute_inc(&mut self, addr: u16, cycles: u32) {
        let result: u8 = self.memory[addr as usize] + 1;
        self.memory[addr as usize] = result;
        self.set_flag_zero_by_val(result);
        self.set_flag_negative_by_val(result);
        self.cycle += cycles;
    }

    fn execute_lda(&mut self, value: u8, cycles: u32) {
        self.accumulator = value;
        self.set_flag_zero_by_val(self.accumulator);
        self.set_flag_negative_by_val(self.accumulator);
        self.cycle += cycles;
    }

    fn execute_ldx(&mut self, value: u8, cycles: u32) {
        self.index_x = value;
        self.set_flag_zero_by_val(self.index_x);
        self.set_flag_negative_by_val(self.index_x);
        self.cycle += cycles;
    }

    fn execute_ldy(&mut self, value: u8, cycles: u32) {
        self.index_y = value;
        self.set_flag_zero_by_val(self.index_y);
        self.set_flag_negative_by_val(self.index_y);
        self.cycle += cycles;
    }

    fn execute_lsr<R>(&mut self, value: u8, r: R, cycles: u32)
    where
        R: Fn(&mut Cpu, u8),
    {
        let result: u8 = (value >> 1) & !(1 >> 1);
        self.set_flag_carry(false);
        self.set_flag_zero(result == 0);
        self.set_flag_negative(false);
        r(self, result);
        self.cycle += cycles
    }

    fn execute_ora(&mut self, value: u8, cycles: u32) {
        self.accumulator |= value;
        self.set_flag_zero_by_val(self.accumulator);
        self.set_flag_negative_by_val(self.accumulator);
        self.cycle += cycles;
    }

    fn execute_rol<R>(&mut self, value: u8, r: R, cycles: u32)
    where
        R: Fn(&mut Cpu, u8),
    {
        let result: u8 = (value << 1) | self.get_flag_carry() as u8;
        self.set_flag_carry((value >> 7) & 1 == 1);
        self.set_flag_zero_by_val(result);
        self.set_flag_negative_by_val(result);
        r(self, result);
        self.cycle += cycles;
    }

    fn execute_ror<R>(&mut self, value: u8, r: R, cycles: u32)
    where
        R: Fn(&mut Cpu, u8),
    {
        let result: u8 = (value >> 1) | ((self.get_flag_carry() as u8) << 7);
        self.set_flag_carry(value & 1 == 1);
        self.set_flag_zero_by_val(result);
        self.set_flag_negative_by_val(result);
        r(self, result);
        self.cycle += cycles;
    }

    fn execute_sbc(&mut self, value: u8, cycles: u32) {
        let acc: u8 = self.accumulator;
        let result: i16 = acc as i16 + (!value) as i16 + (self.get_flag_carry() as u8) as i16;
        self.accumulator = (result & 0xFF) as u8;
        self.set_flag_carry(!(result < 0));
        self.set_flag_zero(result == 0);
        self.set_flag_overflow((result ^ acc as i16) & (result & !value as i16) & 0x80 == 0x80);
        self.set_flag_negative_by_val(self.accumulator);
        self.cycle += cycles;
    }

    fn execute_st(&mut self, addr: u16, value: u8, cycles: u32) {
        self.memory[addr as usize] = value;
        self.cycle += cycles;
    }

    fn push(&mut self, val: u8) {
        self.memory[self.stack_pointer as usize + 0x0100] = val;
        self.stack_pointer -= 1;
    }

    fn pop(&mut self) -> u8 {
        self.stack_pointer += 1;
        self.memory[self.stack_pointer as usize + 0x0100]
    }

    fn get_processor_status(&self) -> u8 {
        let mut out: u8 = 0b11 << 4;
        if self.get_flag_negative() {
            out |= 1 << 7
        }
        if self.get_flag_overflow() {
            out |= 1 << 6
        }
        if self.get_flag_decimal() {
            out |= 1 << 3
        }
        if self.get_flag_interrupt() {
            out |= 1 << 2
        }
        if self.get_flag_zero() {
            out |= 1 << 1
        }
        if self.get_flag_carry() {
            out |= 1
        }
        out
    }

    fn set_processor_status(&mut self, flags: u8, delay: bool) {
        self.set_flag_carry(flags & 1 == 1);
        self.set_flag_zero((flags << 1) & 1 == 1);
        if delay {
            self.change_interrupt_disable_flag = ((flags << 2) & 1) as i8;
        } else {
            self.set_flag_interrupt((flags << 2) & 1 == 1);
        }
        self.set_flag_decimal((flags << 3) & 1 == 1);
        self.set_flag_overflow((flags << 6) & 1 == 1);
        self.set_flag_negative((flags << 7) & 1 == 1);
    }

    //<editor-fold desc="Addressing">
    fn get_addr_zero(&self, arg: u8) -> u8 {
        self.memory[self.get_addr_zero_index(arg) as usize]
    }
    fn get_addr_zero_index(&self, arg: u8) -> u8 {
        arg % 0xFF
    }
    fn set_addr_zero(&mut self, arg: u8, value: u8) {
        self.memory[self.get_addr_zero_index(arg) as usize] = value
    }
    fn get_addr_zero_x(&self, arg: u8) -> u8 {
        self.memory[self.get_addr_zero_x_index(arg) as usize]
    }
    fn get_addr_zero_x_index(&self, arg: u8) -> u8 {
        (arg + self.index_x) % 0xFF
    }
    fn get_addr_zero_y(&self, arg: u8) -> u8 {
        self.memory[self.get_addr_zero_y_index(arg) as usize]
    }
    fn get_addr_zero_y_index(&self, arg: u8) -> u8 {
        (arg + self.index_y) % 0xFF
    }
    fn set_addr_zero_x(&mut self, arg: u8, value: u8) {
        self.memory[self.get_addr_zero_x_index(arg) as usize] = value
    }
    fn get_addr_absolute(&self, arg: u16) -> u8 {
        self.memory[arg as usize]
    }
    fn set_addr_absolute(&mut self, arg: u16, value: u8) {
        self.memory[arg as usize] = value
    }
    fn get_addr_absolute_x(&self, arg: u16) -> u8 {
        self.memory[(arg + self.index_x as u16) as usize]
    }
    fn set_addr_absolute_x(&mut self, arg: u16, value: u8) {
        self.memory[(arg + self.index_x as u16) as usize] = value
    }
    fn get_addr_absolute_y(&self, arg: u16) -> u8 {
        self.memory[(arg + self.index_y as u16) as usize]
    }
    /// (Indirect,X)
    fn get_addr_indexed_indirect(&self, arg: u8) -> u8 {
        self.memory[self.get_addr_indexed_indirect_index(arg)]
    }
    /// (Indirect,X)
    ///
    /// Indirectly retrieves a 16-bit address at (arg + x)'s location.
    /// (arg + x) points to the low byte, (arg + x + 1) points to the high byte.
    #[rustfmt::skip]
    fn get_addr_indexed_indirect_index(&self, arg: u8) -> usize {
        (((self.memory[((arg + self.index_x + 1) as usize) & 0xFF] as u16) << 8)
            | self.memory[((arg + self.index_x) & 0xFF) as usize] as u16
        ) as usize
    }
    /// (Indirect),Y
    fn get_addr_indirect_indexed(&self, arg: u8) -> u8 {
        self.memory[self.get_addr_indirect_indexed_index(arg)]
    }
    /// (Indirect),Y
    ///
    /// Indirectly retrieves a 16-bit address at arg's location, adding y to it.
    /// arg points to the low byte, (arg + 1) points to the high byte.
    #[rustfmt::skip]
    fn get_addr_indirect_indexed_index(&self, arg: u8) -> usize {
        (
            (((self.memory[(arg as usize + 1) & 0xFF] as u16) << 8)
                | self.memory[arg as usize] as u16)
            + self.index_y as u16
        ) as usize
    }
    //</editor-fold>

    //<editor-fold desc="Processor Status Methods">
    fn get_flag(&self, offset: u8) -> bool {
        (self.processor_status >> offset) & 1 == 1
    }

    fn set_flag(&mut self, val: bool, offset: u8) {
        if val {
            self.processor_status |= 1 << offset;
        } else {
            self.processor_status &= !(1 << offset);
        }
    }

    fn get_flag_carry(&self) -> bool {
        self.get_flag(6)
    }

    fn set_flag_carry(&mut self, val: bool) {
        self.set_flag(val, 6)
    }

    fn set_flag_carry_by_val(&mut self, val: u16) {
        self.set_flag_carry(val > 0xFF)
    }

    fn get_flag_zero(&self) -> bool {
        self.get_flag(5)
    }

    fn set_flag_zero(&mut self, val: bool) {
        self.set_flag(val, 5)
    }

    fn set_flag_zero_by_val(&mut self, val: u8) {
        self.set_flag_zero(val == 0);
    }

    fn get_flag_overflow(&self) -> bool {
        self.get_flag(1)
    }

    fn set_flag_overflow(&mut self, val: bool) {
        self.set_flag(val, 1)
    }

    fn set_flag_overflow_by_val(&mut self, val: u8) {
        self.set_flag_overflow((val >> 6) & 1 == 1);
    }

    fn get_flag_negative(&self) -> bool {
        self.get_flag(0)
    }

    fn set_flag_negative(&mut self, val: bool) {
        self.set_flag(val, 0)
    }

    fn set_flag_negative_by_val(&mut self, val: u8) {
        self.set_flag_negative(val >> 7 & 1 == 1);
    }

    fn get_flag_decimal(&self) -> bool {
        self.get_flag(3)
    }

    fn set_flag_decimal(&mut self, val: bool) {
        self.set_flag(val, 3)
    }
    fn get_flag_interrupt(&self) -> bool {
        self.get_flag(4)
    }

    fn set_flag_interrupt(&mut self, val: bool) {
        self.set_flag(val, 4)
    }
    //</editor-fold>
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
use std::hint::assert_unchecked;
use implicit_fn::implicit_fn;
use crate::cpu::{Cpu, Instruction};

    //<editor-fold desc="Test Utility Methods">
    fn no_init(_: &mut Cpu) {}

    fn no_test(_: &Cpu) {}

    fn test_inst<I, T>(
        init: I, op_code: u8, args: [u8; 2], size: u8, test: T, pc: u16, cycle: u32,
    ) where
        I: Fn(&mut Cpu) -> (),
        T: Fn(&Cpu) -> (),
    {
        let mut cpu = Cpu::new();
        init(&mut cpu);
        let inst = Instruction::new(op_code, args, size);
        cpu.execute_instruction(&inst);

        assert_eq!(cpu.cycle, cycle);
        assert_eq!(cpu.program_counter, pc);
        test(&cpu);
    }

    fn test_branch<S: Fn(&mut Cpu, bool)>(op_code: u8, init: S, value: bool) {
        test_inst(
            |cpu| -> () {
                cpu.program_counter = 0x20;
                init(cpu, value);
            },
            op_code, [0x10, 0], 2,
            no_test, 0x32, 3
        );
        test_inst(
            |cpu| -> () {
                cpu.program_counter = 0x20;
                init(cpu, !value);
            },
            op_code, [0x10, 0], 2,
            no_test, 0x22, 2
        );
        test_inst(
            |cpu| -> () {
                cpu.program_counter = 0x02F0;
                init(cpu, value);
            },
            op_code, [0x20, 0], 2,
            no_test, 0x0312, 4
        );
    }

    fn test_compare<S: Fn(&mut Cpu, u8)>(op_code_im: u8, op_code_zero: u8, op_code_abs: u8, set: S) {
        test_inst(
            |cpu| -> () { set(cpu, 20) },
            op_code_im, [30, 0], 2,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_carry(), false);
                assert_eq!(cpu.get_flag_zero(), false);
                assert_eq!(cpu.get_flag_negative(), true);
            }, 2, 2
        );
        test_inst(
            |cpu| -> () {
                set(cpu, 20);
                cpu.memory[10] = 10;
            },
            op_code_zero, [10, 0], 2,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_carry(), true);
                assert_eq!(cpu.get_flag_zero(), false);
                assert_eq!(cpu.get_flag_negative(), false);
            }, 2, 3
        );
        test_inst(
            |cpu| -> () {
                set(cpu, 20);
                cpu.memory[0x2457] = 20;
            },
            op_code_abs, [0x24, 0x57], 3,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_carry(), true);
                assert_eq!(cpu.get_flag_zero(), true);
                assert_eq!(cpu.get_flag_negative(), false);
            }, 3, 4
        );
    }

    fn test_set<S, G>(op_code: u8, set: S, get: G, val: bool)
    where
        S: Fn(&mut Cpu, bool),
        G: Fn(&Cpu) -> bool
    {
        test_inst(
            |cpu| -> () { set(cpu, !val); },
            op_code, [0, 0], 1,
            |cpu| -> () { assert_eq!(get(cpu), val) },
            1, 2
        );
    }

    fn test_zero_negative(cpu: &Cpu, result: u8) {
        assert_eq!(cpu.get_flag_zero(), result == 0);
        assert_eq!(cpu.get_flag_negative(), (result >> 7) & 1 == 1)
    }

    fn test_accumulator<M>(
         op_code: u8, acc: u8, check_value: M, cycles: u32
    ) where
        M: Fn(&Cpu, u8) -> u8
    {
        test_inst(
            |cpu| -> () { cpu.accumulator = acc },
            op_code, [0, 0], 1,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.accumulator)); },
            1, cycles
        );
    }

    fn test_immediate<M, I>(
         init: I, op_code: u8, val: u8, check_value: M, cycles: u32
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        test_inst(
            init,
            op_code, [val, 0x00], 2,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, val)); },
            2, cycles
        );
    }

    fn test_zero_page<M, I>(
        init: I, op_code: u8, val: u8, check_value: M, cycles: u32
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        test_inst(
            |cpu| -> () {
                cpu.memory[val as usize] = 0x10;
                init(cpu);
            },
            op_code, [val, 0x00], 2,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[val as usize])) },
            2, cycles
        );
    }

    fn test_zero_page_x<M, I>(
        init: I, op_code: u8, val: u8, check_value: M, cycles: u32
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        test_inst(
            |cpu| -> () {
                cpu.index_x = 10;
                cpu.memory[val as usize + cpu.index_x as usize] = 0x10;
                init(cpu);
            },
            op_code, [val, 0x00], 2,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[val as usize + cpu.index_x as usize])) },
            2, cycles
        );
    }

    fn test_absolute<M, I>(
        init: I, op_code: u8, val: u16, check_value: M, cycles: u32
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        let bytes = val.to_be_bytes();
        test_inst(
            |cpu| -> () {
                cpu.memory[val as usize] = 0x10;
                init(cpu);
            },
            op_code, bytes, 3,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[val as usize])) },
            3, cycles
        );
    }

    fn test_absolute_x<M, I>(
        init: I, op_code: u8, val: u16, check_value: M, cycles: u32, cross_page: bool
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        let bytes = val.to_be_bytes();
        test_inst(
            |cpu| -> () {
                cpu.index_x = if cross_page { 0xF0 } else { 0x10 };
                init(cpu);
                cpu.memory[val as usize + cpu.index_x as usize] = 0x10;
            },
            op_code, bytes, 3,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[val as usize + cpu.index_x as usize])) },
            3, if cross_page { cycles + 1 } else { cycles }
        );
    }

    fn test_absolute_y<M, I>(
        init: I, op_code: u8, val: u16, check_value: M, cycles: u32, cross_page: bool
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        let bytes = val.to_be_bytes();
        test_inst(
            |cpu| -> () {
                cpu.index_y = if cross_page { 0xF0 } else { 0x10 };
                init(cpu);
                cpu.memory[val as usize + cpu.index_y as usize] = 0x10;
            },
            op_code, bytes, 3,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[val as usize + cpu.index_y as usize])) },
            3, if cross_page { cycles + 1 } else { cycles }
        );
    }

    #[implicit_fn]
    fn test_all_indirect<S, G, E>(
        op_code_x: u8, op_code_y: u8,
        setter: S, getter: G, expected: E, set: u8
    ) where
        S: Fn(&mut Cpu, u8),
        G: Fn(&Cpu) -> u8,
        E: Fn(u8, u8) -> u8
    {
        if op_code_x != 0 {
            test_indirect_indexed(
                |cpu| -> () { setter(cpu, set) },
                op_code_x, 0x50,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 6
            );
        }
        if op_code_y != 0 {
            test_indexed_indirect(
                |cpu| -> () { setter(cpu, set) },
                op_code_y, 0x1B,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 5, false
            );
            test_indexed_indirect(
                |cpu| -> () { setter(cpu, set) },
                op_code_y, 0xC4,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 5, true
            );
        }
    }

    fn test_all_absolute<S, G, E>(
            op_code_abs: u8, op_code_x: u8, op_code_y: u8,
            setter: S, getter: G, expected: E, set: u8
        ) where
            S: Fn(&mut Cpu, u8),
            G: Fn(&Cpu) -> u8,
        E: Fn(u8, u8) -> u8
    {
        if op_code_abs != 0 {
            test_absolute(
                |cpu| -> () { setter(cpu, set) },
                op_code_abs, 0x1145,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 4
            );
        }
        if op_code_x != 0 {
            test_absolute_x(
                |cpu| -> () { setter(cpu, set) },
                op_code_x, 0x1145,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 4, false
            );
            test_absolute_x(
                |cpu| -> () { setter(cpu, set) },
                op_code_x, 0x1145,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 4, true
            );
        }
        if op_code_y != 0 {
            test_absolute_y(
                |cpu| -> () { setter(cpu, set) },
                op_code_y, 0x1145,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 4, false
            );
            test_absolute_y(
                |cpu| -> () { setter(cpu, set) },
                op_code_y, 0x1145,
                |cpu, val| -> u8 {
                    assert_eq!(getter(cpu), expected(val, set));
                    getter(cpu)
                }, 4, true
            );
        }
    }

    /// (Indirect,X)
    fn test_indirect_indexed<M, I>(
        init: I, op_code: u8, val: u8, check_value: M, cycles: u32
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        let x = 0x10;
        test_inst(
            |cpu| -> () {
                cpu.index_x = x;
                init(cpu);
                cpu.memory[(val + cpu.index_x) as usize & 0xFF] = 0x10;
                cpu.memory[(val + cpu.index_x + 1) as usize & 0xFF] = 0x20;
                cpu.memory[0x2010] = 0x50;
            },
            op_code, [val, 0], 2,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[0x2010])) },
            2, cycles
        );
    }

    /// (Indirect),Y
    fn test_indexed_indirect<M, I>(
        init: I, op_code: u8, val: u8, check_value: M, cycles: u32, cross_page: bool
    ) where
        I: Fn(&mut Cpu),
        M: Fn(&Cpu, u8) -> u8
    {
        let y = if cross_page { 0xF0 } else { 0x10 };

        test_inst(
            |cpu| -> () {
                cpu.index_y = y;
                init(cpu);
                cpu.memory[val as usize] = 0x10;
                cpu.memory[val as usize + 1] = 0x20;
                cpu.memory[0x2010 + cpu.index_y as usize] = 0x50;
            },
            op_code, [val, 0], 2,
            |cpu| -> () { test_zero_negative(cpu, check_value(cpu, cpu.memory[0x2010 + cpu.index_y as usize])) },
            2, if cross_page { cycles + 1 } else { cycles }
        );
    }
    //</editor-fold>

    #[test]
    #[implicit_fn]
    fn test_adc() {
        test_immediate(
            no_init,
            0x69, 0x05,
            |cpu, val| -> u8 {
                assert_eq!(cpu.accumulator, val);
                cpu.accumulator
            }, 2
        );
        test_zero_page(
            |cpu| -> () {
                cpu.accumulator = 0x80;
                cpu.memory[0x05] = 0xA2;
            },
            0x65, 0x05,
            |cpu, val| -> u8 {
                let res = (val as u16 + (0x80 & 0xFF)) as u8;
                assert_eq!(cpu.accumulator, res);
                assert_eq!(cpu.get_flag_carry(), true);
                assert_eq!(cpu.get_flag_overflow(), true);
                cpu.accumulator
            }, 3
        );
        test_zero_page_x(
            |cpu| -> () { cpu.accumulator = 10; },
            0x75, 0x05,
            |cpu, val| -> u8 {
                assert_eq!(cpu.accumulator, val + 10);
                cpu.accumulator
            }, 4
        );

        test_all_absolute(
            0x6D, 0x7D, 0x79,
            _.accumulator = _, _.accumulator, _ + _,
            40
        );
        test_all_indirect(
            0x61, 0x71,
            _.accumulator = _, _.accumulator, _ + _,
            40
        )
    }

    #[test]
    #[implicit_fn]
    fn test_and() {
        test_immediate(
            no_init,
            0x29, 0xAB,
            |cpu, _val| -> u8 {
                assert_eq!(cpu.accumulator, 0);
                cpu.accumulator
            }, 2
        );
        test_zero_page(
            |cpu| -> () { cpu.accumulator = 0x0F },
            0x25, 0x82,
            |cpu, val| -> u8 {
                assert_eq!(cpu.accumulator, val & 0x0F);
                cpu.accumulator
            }, 3
        );
        test_zero_page_x(
            |cpu| -> () { cpu.accumulator = 0xFF },
            0x35, 0xAB,
            |cpu, val| -> u8 {
                assert_eq!(cpu.accumulator, val & 0xFF);
                cpu.accumulator
            }, 4
        );

        test_all_absolute(
            0x2D, 0x3D, 0x39,
            _.accumulator = _, _.accumulator, _ & _,
            0x40
        );
        test_all_indirect(
            0x21, 0x31,
            _.accumulator = _, _.accumulator, _ & _,
            251
        );
    }

    #[test]
    fn test_asl() {
        test_accumulator(
            0x0A, 0x9B,
            |cpu, val| -> u8 {
                let expected = 0x9B << 1;
                assert_eq!(val, expected);
                assert_eq!(cpu.get_flag_carry(), true);
                val
            }, 2
        );
        test_zero_page(
            no_init,
            0x06, 0x21,
            |cpu, val| -> u8 {
                assert_eq!(val, 0x10 << 1);
                assert_eq!(cpu.get_flag_carry(), false);
                val
            }, 5
        );
        test_zero_page_x(
            no_init,
            0x16, 0x21,
            |cpu, val| -> u8 {
                assert_eq!(val, 0x10 << 1);
                assert_eq!(cpu.get_flag_carry(), false);
                val
            }, 6
        );
        test_absolute(
            no_init,
            0x0E, 0x2112,
            |cpu, val| -> u8 {
                assert_eq!(val, 0x10 << 1);
                assert_eq!(cpu.get_flag_carry(), false);
                val
            }, 6
        );
        test_absolute_x(
            no_init,
            0x1E, 0x21FF,
            |cpu, val| -> u8 {
                assert_eq!(val, 0x10 << 1);
                assert_eq!(cpu.get_flag_carry(), false);
                val
            }, 7, false
        );
        test_absolute_x(
            no_init,
            0x1E, 0x2100,
            |cpu, val| -> u8 {
                assert_eq!(val, 0x10 << 1);
                assert_eq!(cpu.get_flag_carry(), false);
                val
            }, 7, false
        );
    }

    #[test]
    fn test_bcc() {
        test_branch(0x90, Cpu::set_flag_carry, false);
    }

    #[test]
    fn test_bcs() {
        test_branch(0xB0, Cpu::set_flag_carry, true);
    }

    #[test]
    fn test_beq() {
        test_branch(0xF0, Cpu::set_flag_zero, true);
    }

    #[test]
    fn test_bit() {
        test_inst(
            |cpu| -> () {
                cpu.accumulator = 0xFF;
                cpu.memory[0x80] = 0;
            },
            0x24, [0x80, 0], 2,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_zero(), true);
                assert_eq!(cpu.get_flag_overflow(), false);
                assert_eq!(cpu.get_flag_negative(), false);
            }, 2, 3
        );
        test_inst(
            |cpu| -> () {
                cpu.accumulator = 0xFF;
                cpu.memory[0x80] = 0x80;
            },
            0x24, [0x80, 0], 2,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_zero(), false);
                assert_eq!(cpu.get_flag_overflow(), false);
                assert_eq!(cpu.get_flag_negative(), true);
            }, 2, 3
        );
        test_inst(
            |cpu| -> () {
                cpu.accumulator = 0xFF;
                cpu.memory[0x80] = 1 << 6;
            },
            0x24, [0x80, 0], 2,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_zero(), false);
                assert_eq!(cpu.get_flag_overflow(), true);
                assert_eq!(cpu.get_flag_negative(), false);
            }, 2, 3
        );
        test_inst(
            |cpu| -> () {
                cpu.accumulator = 0xFF;
                cpu.memory[0x80] = 3 << 6;
            },
            0x24, [0x80, 0], 2,
            |cpu| -> () {
                assert_eq!(cpu.get_flag_zero(), false);
                assert_eq!(cpu.get_flag_overflow(), true);
                assert_eq!(cpu.get_flag_negative(), true);
            }, 2, 3
        );
    }

    #[test]
    fn test_bmi() {
        test_branch(0x30, Cpu::set_flag_negative, true);
    }

    #[test]
    fn test_bne() {
        test_branch(0xD0, Cpu::set_flag_zero, false);
    }

    #[test]
    fn test_bpl() {
        test_branch(0x10, Cpu::set_flag_negative, false);
    }

    #[test]
    fn test_brk() {
        let mut cpu = Cpu::new();
        cpu.set_flag_negative(true);
        cpu.set_flag_carry(true);
        cpu.program_counter = 0xAB01;

        assert_eq!(cpu.get_flag_interrupt(), false);

        cpu.execute_instruction(&Instruction::new(0x00, [0, 0], 1));

        assert_eq!(cpu.cycle, 7);
        assert_eq!(cpu.program_counter, 0xFFFE);
        assert_eq!(cpu.memory[cpu.stack_pointer as usize + 0x100 + 1], 0b10110001);
        assert_eq!(cpu.memory[cpu.stack_pointer as usize + 0x100 + 2], 0x03);
        assert_eq!(cpu.memory[cpu.stack_pointer as usize + 0x100 + 3], 0xAB);

        assert_eq!(cpu.get_flag_interrupt(), true);
    }

    #[test]
    fn test_bvc() {
        test_branch(0x50, Cpu::set_flag_overflow, false);
    }

    #[test]
    fn test_bvs() {
        test_branch(0x70, Cpu::set_flag_overflow, true);
    }

    #[test]
    fn test_clc() {
        test_set(0x18, Cpu::set_flag_carry, Cpu::get_flag_carry, false);
    }

    #[test]
    fn test_cld() {
        test_set(0xD8, Cpu::set_flag_decimal, Cpu::get_flag_decimal, false);
    }

    #[test]
    fn test_cli() {
        let mut cpu = Cpu::new();
        cpu.set_flag_interrupt(true);

        assert_eq!(cpu.get_flag_interrupt(), true);

        cpu.execute_instruction(&Instruction::new(0x58, [0, 0], 1));

        assert_eq!(cpu.cycle, 2);
        assert_eq!(cpu.program_counter, 1);

        assert_eq!(cpu.get_flag_interrupt(), true); // the effect of CLI is delayed by one instruction
        assert_eq!(cpu.change_interrupt_disable_flag, 0);

        cpu.execute_instruction(&Instruction::new(0xEA, [0, 0], 1));

        assert_eq!(cpu.get_flag_interrupt(), false);
        assert_eq!(cpu.change_interrupt_disable_flag, -1);
    }

    #[test]
    fn test_clv() {
        test_set(0xB8, Cpu::set_flag_overflow, Cpu::get_flag_overflow, false);
    }

    #[test]
    #[implicit_fn]
    fn test_cmp() {
        test_compare(
            0xC9, 0xC5, 0xCD,
            _.accumulator = _
        );

        test_zero_page_x(
            _.accumulator = 20,
            0xD5, 0xF0,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), true);
                1
            }, 4
        );

        test_absolute_x(
            _.accumulator = 0,
            0xDD, 0x20,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), false);
                0x80
            }, 4, false
        );
        test_absolute_x(
            _.accumulator = 0,
            0xDD, 0xF0,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), false);
                0x80
            }, 4, true
        );
        test_absolute_y(
            _.accumulator = 0x10,
            0xD9, 0x20,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), true);
                0
            }, 4, false
        );
        test_absolute_y(
            _.accumulator = 0x10,
            0xD9, 0xF0,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), true);
                0
            }, 4, true
        );

        test_indirect_indexed(
            _.accumulator = 250,
            0xC1, 0x21,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), true);
                1
            }, 6
        );
        test_indexed_indirect(
            _.accumulator = 5,
            0xD1, 0x26,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), false);
                0x80
            }, 5, false
        );
        test_indexed_indirect(
            _.accumulator = 0x75,
            0xD1, 0x26,
            |cpu, _| -> u8 {
                assert_eq!(cpu.get_flag_carry(), true);
                1
            }, 5, true
        );
    }

    #[test]
    #[implicit_fn]
    fn test_cmx() {
        test_compare(
            0xE0, 0xE4, 0xEC,
            _.index_x = _
        );
    }

    #[test]
    #[implicit_fn]
    fn test_cmy() {
        test_compare(
            0xC0, 0xC4, 0xCC,
            _.index_y = _
        );
    }
}
