struct Cpu {
    memory: [u8; 0xFFFF],
    program_counter: u16,
    stack_position: u8,
    stack_pointer: u8,
    accumulator: u8,
    index_x: u8,
    index_y: u8,
    /// Bit locations: <pre>
    /// 0b100_0000 => carry flag
    /// 0b010_0000 => zero flag
    /// 0b001_0000 => interrupt disable
    /// 0b000_1000 => decimal mode
    /// 0b000_0100 => break command
    /// 0b000_0010 => overflow flag
    /// 0b000_0001 => negative flag
    /// </pre>
    processor_status: u8,

    cycle: u32,
}

/// Instruction reference: https://www.nesdev.org/wiki/Instruction_reference
struct Instruction {
    op_code: u8,
    arguments: [u8; 2],
    // the number of bytes this instruction is long; used for incrementing the program counter
    size: u8,
}

impl Instruction {
    fn get_absolute_addr(&self) -> u16 {
        (self.arguments[0] as u16) << 8 & self.arguments[1] as u16
    }
}

const BRANCHING_OP_CODES: [u8; 14] = [
    0x90, 0x80, 0xF0, 0x30, 0xD0, 0x10, 0x00, 0x50, 0x70, 0x4C, 0x6C, 0x20, 0x40, 0x60,
];

fn increment_if_crossed(base: u32, addr: usize) -> u32 {
    if addr <= 0xFF { base } else { base + 1 }
}

impl Cpu {
    pub fn execute_instruction(&mut self, inst: &Instruction) {
        // branching instructions are special, as they modify the program counter directly instead
        // of simply incrementing it by one. They should be handled first
        if BRANCHING_OP_CODES.contains(&inst.op_code) {
            self.program_counter = match inst.op_code {
                0x90 => self.branch_if_condition(inst.arguments[0], !self.get_carry()),
                0x80 => self.branch_if_condition(inst.arguments[0], self.get_carry()),
                0xF0 => self.branch_if_condition(inst.arguments[0], self.get_zero()),
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
            0x6D => self.execute_adc(self.get_addr_absolute(inst.get_absolute_addr()), 5),
            0x7D => self.execute_adc(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed(4, inst.get_absolute_addr() as usize),
            ),
            0x79 => self.execute_adc(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed(4, inst.get_absolute_addr() as usize),
            ),
            0x61 => self.execute_adc(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0x71 => {
                let memory_index: usize = self.get_addr_indirect_indexed_index(inst.arguments[0]);
                self.execute_adc(
                    self.memory[memory_index],
                    increment_if_crossed(5, memory_index),
                )
            }

            0x29 => self.execute_and(inst.arguments[0], 2),
            0x25 => self.execute_and(self.get_addr_zero(inst.arguments[0]), 3),
            0x35 => self.execute_and(self.get_addr_zero_x(inst.arguments[0]), 4),
            0x2D => self.execute_and(self.get_addr_absolute(inst.get_absolute_addr()), 4),
            0x3D => self.execute_and(
                self.get_addr_absolute_x(inst.get_absolute_addr()),
                increment_if_crossed(4, inst.get_absolute_addr() as usize),
            ),
            0x39 => self.execute_and(
                self.get_addr_absolute_y(inst.get_absolute_addr()),
                increment_if_crossed(4, inst.get_absolute_addr() as usize),
            ),
            0x21 => self.execute_and(self.get_addr_indexed_indirect(inst.arguments[0]), 6),
            0x31 => {
                let memory_index: usize = self.get_addr_indirect_indexed_index(inst.arguments[0]);
                self.execute_and(
                    self.memory[memory_index],
                    increment_if_crossed(5, memory_index),
                )
            }

            0x0A => self.execute_asl(self.accumulator, |cpu, r| -> () { cpu.accumulator += r }, 2),
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
                self.get_addr_absolute(inst.get_absolute_addr()),
                |cpu, r| -> () { cpu.set_addr_absolute_x(inst.get_absolute_addr(), r) },
                7,
            ),

            _ => panic!("Unknown op code received: {}", inst.op_code),
        };
        self.program_counter += inst.size as u16
    }

    fn execute_adc(&mut self, memory: u8, cycles: u32) {
        let result: u16 =
            self.accumulator as u16 + memory as u16 + (self.processor_status & 0b1000_0000) as u16;
        self.set_carry(result > 0xFF);
        self.set_zero(result == 0);
        self.set_overflow(
            (result ^ self.accumulator as u16) & (result ^ memory as u16) & 0x80 == 0x80,
        );
        self.set_negative(result & 0b1000_0000 == 0b1000_0000);
        self.accumulator = (result & 0xFF) as u8;
        self.cycle += cycles
    }

    fn execute_and(&mut self, memory: u8, cycles: u32) {
        let result: u8 = self.accumulator & memory;
        self.set_zero(result == 0);
        self.set_negative(result & 0b1000_0000 == 0b1000_0000);
        self.accumulator = result;
        self.cycle += cycles
    }

    fn execute_asl<R>(&mut self, value: u8, r: R, cycles: u32)
    where
        R: Fn(&mut Cpu, u8),
    {
        let result: u8 = value << 1 & 0b1111_1110;
        self.set_carry(value & 0b1000_0000 == 0b1000_0000);
        self.set_zero(result == 0);
        self.set_negative(result & 0b1000_0000 == 0b1000_0000);
        r(self, result);
        self.cycle += cycles
    }

    fn branch_if_condition(&mut self, value: u8, condition: bool) -> u16 {
        self.cycle += 2;
        if !condition {
            self.program_counter + 2
        } else {
            self.program_counter + 2 + value.cast_signed() as u16
        }
    }

    //<editor-fold desc="Addressing">
    fn get_addr_zero(&self, arg: u8) -> u8 {
        self.memory[(arg % 0xFF) as usize]
    }
    fn set_addr_zero(&mut self, arg: u8, value: u8) {
        self.memory[(arg % 0xFF) as usize] = value
    }
    fn get_addr_zero_x(&self, arg: u8) -> u8 {
        self.memory[((arg + self.index_x) % 0xFF) as usize]
    }
    fn set_addr_zero_x(&mut self, arg: u8, value: u8) {
        self.memory[((arg + self.index_x) % 0xFF) as usize] = value
    }
    fn address_zero_y(&self, arg: u8) -> u8 {
        self.memory[((arg + self.index_y) % 0xFF) as usize]
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
        self.memory[self.memory[((arg + self.index_x) & 0xFF) as usize] as usize
            + (self.memory[((arg + self.index_x + 1) & 0xFF) as usize] as usize)
            << 8]
    }
    /// (Indirect),Y
    fn get_addr_indirect_indexed(&self, arg: u8) -> u8 {
        self.memory[self.get_addr_indirect_indexed_index(arg)]
    }
    fn get_addr_indirect_indexed_index(&self, arg: u8) -> usize {
        self.memory[arg as usize] as usize + (self.memory[(arg as usize + 1) & 256] as usize)
            << 8 + self.index_y as usize
    }
    //</editor-fold>

    //<editor-fold desc="Processor Status Methods">
    fn get_carry(&self) -> bool {
        self.processor_status & 0b100_0000 == 0b100_0000
    }

    fn set_carry(&mut self, val: bool) {
        if val {
            self.processor_status |= 0b100_0000;
        } else {
            self.processor_status &= 0b011_1111;
        }
    }

    fn get_zero(&self) -> bool {
        self.processor_status & 0b010_0000 == 0b010_0000
    }

    fn set_zero(&mut self, val: bool) {
        if val {
            self.processor_status |= 0b010_0000;
        } else {
            self.processor_status &= 0b101_1111;
        }
    }

    fn get_overflow(&self) -> bool {
        self.processor_status & 0b001_0000 == 0b001_0000
    }

    fn set_overflow(&mut self, val: bool) {
        if val {
            self.processor_status |= 0b001_0000;
        } else {
            self.processor_status &= 0b110_1111;
        }
    }

    fn get_negative(&self) -> bool {
        self.processor_status & 0b000_1000 == 0b000_1000
    }

    fn set_negative(&mut self, val: bool) {
        if val {
            self.processor_status |= 0b000_1000;
        } else {
            self.processor_status &= 0b111_0111;
        }
    }
    //</editor-fold>
}
