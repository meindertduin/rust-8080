use std::{self, fmt, process};

use crate::{flags::Flags, invaders::IOState};

#[derive(Clone, Copy)]
#[repr(C)]
pub union RegisterPair {
    both: u16,
    one: (u8, u8),
}

impl RegisterPair {
    pub fn new() -> Self {
        Self { both: 0 }
    }

    pub fn both(self) -> u16 {
        unsafe { self.both }
    }

    pub fn both_mut(&mut self) -> &mut u16 {
        unsafe { &mut self.both }
    }

    /// Least significant byte
    pub fn lsb(self) -> u8 {
        unsafe { self.one.0 }
    }

    /// Least significant byte
    pub fn lsb_mut(&mut self) -> &mut u8 {
        unsafe { &mut self.one.0 }
    }

    /// Most significant byte
    pub fn msb(self) -> u8 {
        unsafe { self.one.1 }
    }

    /// Most significant byte
    pub fn msb_mut(&mut self) -> &mut u8 {
        unsafe { &mut self.one.1 }
    }
}

const MEMORY_SIZE: usize = 0x4000;

pub struct CpuState {
    a: u8,
    bc: RegisterPair,
    de: RegisterPair,
    hl: RegisterPair,
    sp: u16,
    pc: u16,
    memory: [u8; MEMORY_SIZE],
    flags: Flags,
    interrupts_enabled: bool,
}

impl fmt::Debug for CpuState {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "{:>4} {:>4} {:>4} {:>4} {:>4} {:>4} {:>4}",
                 "a", "bc", "de", "hl", "pc", "sp", "flags")?;

        write!(f,
               "{:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {}",
               self.a,
               self.bc(),
               self.de(),
               self.hl(),
               self.pc,
               self.sp,
               self.flags,
        )
    }
}

impl fmt::Display for CpuState {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{:04x}:\t{:02x}\t{}\n\
             a={:02x} b={:02x} c={:02x} d={:02x} e={:02x} h={:02x} l={:02x}\n\
             sp={:04x} flags={}\n",
            self.pc,
            self.read_byte(self.pc),
            self.next_opcode(),
            self.a,
            self.b(),
            self.c(),
            self.d(),
            self.e(),
            self.h(),
            self.l(),
            self.sp,
            self.flags
        )
    }
}

impl std::default::Default for CpuState {
    fn default() -> Self {
        Self {
            a: 0,
            bc: RegisterPair::new(),
            de: RegisterPair::new(),
            hl: RegisterPair::new(),
            sp: 0,
            pc: 0,
            memory: [0; MEMORY_SIZE],
            flags: Flags {
                zero: false,
                sign: false,
                parity: false,
                carry: false,
                aux_carry: false,
            },
            interrupts_enabled: false,
        }
    }
}

impl CpuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_rom(rom: &[u8], rom_start: usize, pc_start: u16) -> Self {
        let mut new = Self::new();
        new.load_rom(rom, rom_start);
        new.pc = pc_start;
        new
    }

    pub fn load_rom(&mut self, rom: &[u8], rom_start: usize) {
        self.memory[rom_start..rom_start + rom.len()].clone_from_slice(rom);
    }

    pub fn pc(&self) -> u16 {
        self.pc
    }

    pub fn sp(&self) -> u16 {
        self.sp
    }

    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | self.flags.psw() as u16
    }

    pub fn a(&self) -> u8 {
        self.a
    }

    pub fn bc(&self) -> u16 {
        self.bc.both()
    }

    pub fn b(&self) -> u8 {
        self.bc.msb()
    }

    pub fn c(&self) -> u8 {
        self.bc.lsb()
    }

    pub fn de(&self) -> u16 {
        self.de.both()
    }

    pub fn d(&self) -> u8 {
        self.de.msb()
    }

    pub fn e(&self) -> u8 {
        self.de.lsb()
    }

    pub fn hl(&self) -> u16 {
        self.hl.both()
    }

    pub fn h(&self) -> u8 {
        self.hl.msb()
    }

    pub fn l(&self) -> u8 {
        self.hl.lsb()
    }

    pub fn m(&self) -> u8 {
        self.read_byte(self.hl())
    }

    pub fn flags(&self) -> &Flags {
        &self.flags
    }

    pub fn memory(&self) -> &[u8] {
        &self.memory
    }

    pub fn interrupt(&mut self, interrupt_num: u16) {
        if self.interrupts_enabled {
            self.push(self.pc);
            self.pc = 8 * interrupt_num;
            self.interrupts_enabled = false;
        }
    }

    // Private

    fn set_af(&mut self, value: u16) {
        self.flags.set_psw(value as u8);
        self.a = (value >> 8) as u8;
    }

    fn bc_mut(&mut self) -> &mut u16 {
        self.bc.both_mut()
    }

    fn b_mut(&mut self) -> &mut u8 {
        self.bc.msb_mut()
    }

    fn c_mut(&mut self) -> &mut u8 {
        self.bc.lsb_mut()
    }

    fn de_mut(&mut self) -> &mut u16 {
        self.de.both_mut()
    }

    fn d_mut(&mut self) -> &mut u8 {
        self.de.msb_mut()
    }

    fn e_mut(&mut self) -> &mut u8 {
        self.de.lsb_mut()
    }

    fn hl_mut(&mut self) -> &mut u16 {
        self.hl.both_mut()
    }

    fn h_mut(&mut self) -> &mut u8 {
        self.hl.msb_mut()
    }

    fn l_mut(&mut self) -> &mut u8 {
        self.hl.lsb_mut()
    }

    fn m_mut(&mut self) -> &mut u8 {
        self.byte_mut(self.hl())
    }

    fn byte_mut(&mut self, address: u16) -> &mut u8 {
        if address < 0x2000 {
            panic!("Trying to write to ROM at address ${:04x}", address)
        }
        &mut self.memory[address as usize]
    }

    /// Reads the byte at the specified address
    fn read_byte(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    /// Reads two bytes starting at the specified address
    fn read_bytes(&self, address: u16) -> u16 {
        ((self.read_byte(address + 1) as u16) << 8) | self.read_byte(address) as u16
    }

    /// Reads the byte following the current instruction
    fn read_byte_immediate(&self) -> u8 {
        self.read_byte(self.pc + 1)
    }

    /// Reads two bytes following the current instruction
    fn read_bytes_immediate(&self) -> u16 {
        self.read_bytes(self.pc + 1)
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        *self.byte_mut(address) = value
    }

    fn write_bytes(&mut self, address: u16, value: u16) {
        self.write_byte(address, value as u8);
        self.write_byte(address + 1, (value >> 8) as u8);
    }

    fn jmp(&mut self, adr: u16) {
        self.pc = adr;
    }

    fn call(&mut self, adr: u16) {
        self.push(self.pc + 3);
        self.pc = adr;
    }

    fn ret(&mut self) {
        self.pc = self.pop();
    }

    fn pop(&mut self) -> u16 {
        self.sp += 2;
        self.read_bytes(self.sp - 2)
    }

    fn push(&mut self, value: u16) {
        self.sp -= 2;
        self.write_bytes(self.sp, value);
    }

    /// Increments `operand`
    fn inr(&mut self, operand: u8) -> u8 {
        let result = operand.wrapping_add(1);
        self.flags.set_all_but_carry(result);
        result
    }

    /// Decrements `operand`
    fn dcr(&mut self, operand: u8) -> u8 {
        let result = operand.wrapping_sub(1);
        self.flags.set_all_but_carry(result);
        result
    }

    /// Add `operand` to A
    fn add(&mut self, operand: u8) {
        let result = (self.a as u16).wrapping_add(operand as u16);
        self.flags.set_all(result, (self.a & 0xf).wrapping_add(operand & 0xf));
        self.a = result as u8;
    }

    /// Add `operand` + carry to A
    fn adc(&mut self, operand: u8) {
        let result = (self.a as u16).wrapping_add(operand as u16).wrapping_add(self.flags.carry as u16);
        self.flags.set_all(result, (self.a & 0xf).wrapping_add(operand.wrapping_add(self.flags.carry as u8) & 0xf));
        self.a = result as u8;
    }

    /// Subtract `operand` from A
    fn sub(&mut self, operand: u8) {
        let result = (self.a as u16).wrapping_sub(operand as u16);
        self.flags.set_all(result, (self.a & 0xf).wrapping_sub(operand & 0xf));
        self.a = result as u8;
    }

    /// Subtract `operand` from A with borrow
    fn sbb(&mut self, operand: u8) {
        let result = (self.a as u16).wrapping_sub(operand as u16).wrapping_sub(self.flags.carry as u16);
        self.flags.set_all(result, (self.a & 0xf).wrapping_sub(operand.wrapping_sub(self.flags.carry as u8) & 0xf));
        self.a = result as u8;
    }

    /// Add `operand` to HL
    fn dad(&mut self, operand: u16) {
        let result = (self.hl() as u32).wrapping_add(operand as u32);
        self.flags.set_carry(result as u16);
        *self.hl_mut() = result as u16;
    }

    /// Immediate bitwise AND A
    fn ani(&mut self) {
        self.a &= self.read_byte_immediate();
        self.flags.set_all_but_aux_carry(self.a as u16);
    }

    /// Bitwise AND between A and `operand`
    fn and(&mut self, operand: u8) {
        self.a &= operand;
        self.flags.set_all(self.a as u16, self.a);
    }

    /// Bitwise OR between A and `operand`
    fn or(&mut self, operand: u8) {
        self.a |= operand;
        self.flags.set_all_but_aux_carry(self.a as u16);
    }

    /// Bitwise XOR between A and `operand`
    fn xor(&mut self, operand: u8) {
        self.a ^= operand;
        self.flags.set_all(self.a as u16, self.a);
        self.flags.carry = false;
    }

    /// Compare `operand` to A
    fn cmp(&mut self, operand: u8) {
        self.flags.set_all((self.a as u16).wrapping_sub(operand as u16), (self.a & 0xf).wrapping_sub(operand & 0xf));
    }

    fn daa(&mut self) {
        let mut result = self.a as u16;

        let lsb = result & 0xf;

        if self.flags.aux_carry || lsb > 9 {
            result += 6;

            if result & 0xf < lsb {
                self.flags.aux_carry = true;
            }
        }

        let lsb = result & 0xf;
        let mut msb = (result >> 4) & 0xf;

        if self.flags.carry || msb > 9 {
            msb += 6;
        }

        let result = (msb << 4) | lsb;
        self.flags.set_all_but_aux_carry(result);

        self.a = result as u8;
    }

    fn next_opcode(&self) -> String {
        self.op_name(self.pc)
    }

    /// Executes the next instruction.
    /// Advances PC appropriately, and returns the number of cycles taken.
    pub fn emulate(&mut self, io_state: &mut IOState) -> u64 {
        let op_code = self.read_byte(self.pc);

        if cfg!(feature="logging") && self.pc != 0xada && self.pc != 0xadd && self.pc != 0xade {
            println!("{}", self);
        }

        let (op_length, cycles) = match op_code {
            // NOP
            0x00 | 0x20 => (1, 4),
            // LXI B, D16
            0x01 => {
                *self.bc_mut() = self.read_bytes_immediate();
                (3, 10)
            }
            // STAX B
            0x02 => {
                self.write_byte(self.bc(), self.a);
                (1, 7)
            }
            // INX B
            0x03 => {
                *self.bc_mut() = self.bc().wrapping_add(1);
                (1, 5)
            }
            // INR B
            0x04 => {
                *self.b_mut() = self.inr(self.b());
                (1, 5)
            }
            // DCR B
            0x05 => {
                *self.b_mut() = self.dcr(self.b());
                (1, 5)
            }
            // MVI B, D8
            0x06 => {
                *self.b_mut() = self.read_byte_immediate();
                (2, 7)
            }
            // RLC
            0x07 => {
                let bit7: u8 = self.a & (1 << 7);
                self.a <<= 1;
                self.a |= bit7 >> 7;
                self.flags.carry = bit7 != 0;
                (1, 4)
            }
            // DAD B
            0x09 => {
                self.dad(self.bc());
                (1, 10)
            }
            // LDAX B
            0x0a => {
                self.a = self.read_byte(self.bc());
                (1, 7)
            }
            // INR C
            0x0c => {
                *self.c_mut() = self.inr(self.c());
                (1, 5)
            }
            // DCR C
            0x0d => {
                *self.c_mut() = self.dcr(self.c());
                (1, 5)
            }
            // MVI C, D8
            0x0e => {
                *self.c_mut() = self.read_byte_immediate();
                (2, 7)
            }
            // RRC
            0x0f => {
                let bit0: u8 = self.a & 1;
                self.a >>= 1;
                self.a |= bit0 << 7;
                self.flags.carry = bit0 != 0;
                (1, 4)
            }
            // LXI D, D16
            0x11 => {
                *self.de_mut() = self.read_bytes_immediate();
                (3, 10)
            }
            // STAX D
            0x12 => {
                self.write_byte(self.de(), self.a);
                (1, 7)
            }
            // INX D
            0x13 => {
                *self.de_mut() = self.de().wrapping_add(1);
                (1, 5)
            }
            // INR D
            0x14 => {
                *self.d_mut() = self.inr(self.d());
                (1, 5)
            }
            // DCR D
            0x15 => {
                *self.d_mut() = self.dcr(self.d());
                (1, 5)
            }
            // MVI D, D8
            0x16 => {
                *self.d_mut() = self.read_byte_immediate();
                (2, 7)
            }
            // RAL
            0x17 => {
                let bit7: u8 = self.a & (1 << 7);
                self.a <<= 1;
                self.a |= self.flags.carry as u8;
                self.flags.carry = bit7 != 0;
                (1, 4)
            }
            // DAD D
            0x19 => {
                self.dad(self.de());
                (1, 10)
            }
            // LDAX D
            0x1a => {
                self.a = self.read_byte(self.de());
                (1, 7)
            }
            // DCX D
            0x1b => {
                *self.de_mut() = self.de().wrapping_sub(1);
                (1, 5)
            }
            // MVI E, D8
            0x1e => {
                *self.e_mut() = self.read_byte_immediate();
                (2, 7)
            }
            // RAR
            0x1f => {
                let bit0: u8 = self.a & 1;
                self.a >>= 1;
                if self.flags.carry { self.a |= 1 << 7; }
                self.flags.carry = bit0 != 0;
                (1, 4)
            }
            // LXI H, D16
            0x21 => {
                *self.hl_mut() = self.read_bytes_immediate();
                (3, 10)
            }
            // SHLD adr
            0x22 => {
                self.write_bytes(self.read_bytes_immediate(), self.hl());
                (3, 16)
            }
            // INX H
            0x23 => {
                *self.hl_mut() = self.hl().wrapping_add(1);
                (1, 5)
            }
            // MVI H, D8
            0x26 => {
                *self.h_mut() = self.read_byte_immediate();
                (2, 7)
            }
            // DAA
            0x27 => {
                self.daa();
                (1, 4)
            }
            // DAD H
            0x29 => {
                self.dad(self.hl());
                (1, 10)
            }
            // LHLD adr
            0x2a => {
                *self.hl_mut() = self.read_bytes(self.read_bytes_immediate());
                (3, 16)
            }
            // DCX H
            0x2b => {
                *self.hl_mut() = self.hl().wrapping_sub(1);
                (1, 5)
            }
            // INR L
            0x2c => {
                *self.l_mut() = self.inr(self.l());
                (1, 5)
            }
            // MVI L, D8
            0x2e => {
                *self.l_mut() = self.read_byte_immediate();
                (2, 7)
            }
            // CMA
            0x2f => {
                self.a = !self.a;
                (1, 4)
            }
            // LXI SP, D16
            0x31 => {
                self.sp = self.read_bytes_immediate();
                (3, 10)
            }
            // STA adr
            0x32 => {
                self.write_byte(self.read_bytes_immediate(), self.a);
                (3, 13)
            }
            // INR M
            0x34 => {
                *self.m_mut() = self.inr(self.m());
                (1, 10)
            }
            // DCR M
            0x35 => {
                *self.m_mut() = self.dcr(self.m());
                (1, 10)
            }
            // MVI M, D8
            0x36 => {
                *self.m_mut() = self.read_byte_immediate();
                (2, 10)
            }
            // STC
            0x37 => {
                self.flags.carry = true;
                (1, 4)
            }
            // LDA adr
            0x3a => {
                self.a = self.read_byte(self.read_bytes_immediate());
                (3, 13)
            }
            // INR A
            0x3c => {
                self.a = self.inr(self.a);
                (1, 5)
            }
            // DCR A
            0x3d => {
                self.a = self.dcr(self.a);
                (1, 7)
            }
            // MVI A, D8
            0x3e => {
                self.a = self.read_byte_immediate();
                (2, 7)
            }
            // CMC
            0x3f => {
                self.flags.carry = !self.flags.carry;
                (1, 4)
            }
            // MOV B,C
            0x41 => {
                *self.b_mut() = self.c();
                (1, 5)
            }
            // MOV B,M
            0x46 => {
                *self.b_mut() = self.m();
                (1, 7)
            }
            // MOV B,A
            0x47 => {
                *self.b_mut() = self.a;
                (1, 5)
            }
            // MOV C,B
            0x48 => {
                *self.c_mut() = self.b();
                (1, 5)
            }
            // MOV C,M
            0x4e => {
                *self.c_mut() = self.m();
                (1, 7)
            }
            // MOV C,A
            0x4f => {
                *self.c_mut() = self.a;
                (1, 5)
            }
            // MOV D,M
            0x56 => {
                *self.d_mut() = self.m();
                (1, 7)
            }
            // MOV D,A
            0x57 => {
                *self.d_mut() = self.a;
                (1, 5)
            }
            // MOV E,M
            0x5e => {
                *self.e_mut() = self.m();
                (1, 7)
            }
            // MOV E,A
            0x5f => {
                *self.e_mut() = self.a;
                (1, 5)
            }
            // MOV H,C
            0x61 => {
                *self.h_mut() = self.c();
                (1, 5)
            }
            // MOV H,L
            0x65 => {
                *self.h_mut() = self.l();
                (1, 5)
            }
            // MOV H,M
            0x66 => {
                *self.h_mut() = self.m();
                (1, 7)
            }
            // MOV H,A
            0x67 => {
                *self.h_mut() = self.a;
                (1, 5)
            }
            // MOV L,B
            0x68 => {
                *self.l_mut() = self.b();
                (1, 5)
            }
            // MOV L,C
            0x69 => {
                *self.l_mut() = self.c();
                (1, 5)
            }
            // MOV L,A
            0x6f => {
                *self.l_mut() = self.a;
                (1, 5)
            }
            // MOV M,B
            0x70 => {
                *self.m_mut() = self.b();
                (1, 7)
            }
            // MOV M,C
            0x71 => {
                *self.m_mut() = self.c();
                (1, 7)
            }
            // HLT
            0x76 => {
                println!("HLT instruction received");
                process::exit(0)
            }
            // MOV M,A
            0x77 => {
                *self.m_mut() = self.a;
                (1, 7)
            }
            // MOV A,B
            0x78 => {
                self.a = self.b();
                (1, 5)
            }
            // MOV A,C
            0x79 => {
                self.a = self.c();
                (1, 5)
            }
            // MOV A,D
            0x7a => {
                self.a = self.d();
                (1, 5)
            }
            // MOV A,E
            0x7b => {
                self.a = self.e();
                (1, 5)
            }
            // MOV A,H
            0x7c => {
                self.a = self.h();
                (1, 5)
            }
            // MOV A,L
            0x7d => {
                self.a = self.l();
                (1, 5)
            }
            // MOV A,M
            0x7e => {
                self.a = self.m();
                (1, 7)
            }
            // ADD B
            0x80 => {
                self.add(self.b());
                (1, 4)
            }
            // ADD C
            0x81 => {
                self.add(self.c());
                (1, 4)
            }
            // ADD D
            0x82 => {
                self.add(self.d());
                (1, 4)
            }
            // ADD E
            0x83 => {
                self.add(self.e());
                (1, 4)
            }
            // ADD H
            0x84 => {
                self.add(self.h());
                (1, 4)
            }
            // ADD L
            0x85 => {
                self.add(self.l());
                (1, 4)
            }
            // ADD M
            0x86 => {
                self.add(self.m());
                (1, 7)
            }
            // ADD A
            0x87 => {
                self.add(self.a);
                (1, 4)
            }
            // ADC D
            0x8a => {
                self.adc(self.d());
                (1, 4)
            }
            // SUB A
            0x97 => {
                self.sub(self.a);
                (1, 4)
            }
            // ANA B
            0xa0 => {
                self.and(self.b());
                (1, 4)
            }
            // ANA C
            0xa1 => {
                self.and(self.c());
                (1, 4)
            }
            // ANA D
            0xa2 => {
                self.and(self.d());
                (1, 4)
            }
            // ANA E
            0xa3 => {
                self.and(self.e());
                (1, 4)
            }
            // ANA H
            0xa4 => {
                self.and(self.h());
                (1, 4)
            }
            // ANA L
            0xa5 => {
                self.and(self.l());
                (1, 4)
            }
            // ANA M
            0xa6 => {
                self.and(self.m());
                (1, 7)
            }
            // ANA A
            0xa7 => {
                self.and(self.a);
                (1, 4)
            }
            // XRA B
            0xa8 => {
                self.xor(self.b());
                (1, 4)
            }
            // XRA C
            0xa9 => {
                self.xor(self.c());
                (1, 4)
            }
            // XRA D
            0xaa => {
                self.xor(self.d());
                (1, 4)
            }
            // XRA E
            0xab => {
                self.xor(self.e());
                (1, 4)
            }
            // XRA H
            0xac => {
                self.xor(self.h());
                (1, 4)
            }
            // XRA L
            0xad => {
                self.xor(self.l());
                (1, 4)
            }
            // XRA M
            0xae => {
                self.xor(self.m());
                (1, 7)
            }
            // XRA A
            0xaf => {
                self.xor(self.a);
                (1, 4)
            }
            // ORA B
            0xb0 => {
                self.or(self.b());
                (1, 4)
            }
            // ORA C
            0xb1 => {
                self.or(self.c());
                (1, 4)
            }
            // ORA D
            0xb2 => {
                self.or(self.d());
                (1, 4)
            }
            // ORA E
            0xb3 => {
                self.or(self.e());
                (1, 4)
            }
            // ORA H
            0xb4 => {
                self.or(self.h());
                (1, 4)
            }
            // ORA L
            0xb5 => {
                self.or(self.l());
                (1, 4)
            }
            // ORA M
            0xb6 => {
                self.or(self.m());
                (1, 7)
            }
            // ORA A
            0xb7 => {
                self.or(self.a);
                (1, 4)
            }
            // CMP B
            0xb8 => {
                self.cmp(self.b());
                (1, 4)
            }
            // CMP C
            0xb9 => {
                self.cmp(self.c());
                (1, 4)
            }
            // CMP D
            0xba => {
                self.cmp(self.d());
                (1, 4)
            }
            // CMP E
            0xbb => {
                self.cmp(self.e());
                (1, 4)
            }
            // CMP H
            0xbc => {
                self.cmp(self.h());
                (1, 4)
            }
            // CMP L
            0xbd => {
                self.cmp(self.l());
                (1, 4)
            }
            // CMP M
            0xbe => {
                self.cmp(self.m());
                (1, 7)
            }
            // CMP A
            0xbf => {
                self.cmp(self.a);
                (1, 4)
            }
            // RNZ
            0xc0 => {
                if self.flags.zero {
                    (1, 5)
                } else {
                    self.ret();
                    (0, 11)
                }
            }
            // POP B
            0xc1 => {
                *self.bc_mut() = self.pop();
                (1, 10)
            }
            // JNZ adr
            0xc2 => {
                if self.flags.zero {
                    (3, 10)
                } else {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                }
            }
            // JMP adr
            0xc3 => {
                self.jmp(self.read_bytes_immediate());
                (0, 10)
            }
            // CNZ adr
            0xc4 => {
                if self.flags.zero {
                    (3, 11)
                } else {
                    self.call(self.read_bytes_immediate());
                    (0, 17)
                }
            }
            // PUSH B
            0xc5 => {
                self.push(self.bc());
                (1, 11)
            }
            // ADI D8
            0xc6 => {
                self.add(self.read_byte_immediate());
                (2, 7)
            }
            // RZ
            0xc8 => {
                if self.flags.zero {
                    self.ret();
                    (0, 11)
                } else {
                    (1, 5)
                }
            }
            // RET
            0xc9 => {
                self.ret();
                (0, 10)
            }
            // JZ adr
            0xca => {
                if self.flags.zero {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                } else {
                    (3, 10)
                }
            }
            // CZ adr
            0xcc => {
                if self.flags.zero {
                    self.call(self.read_bytes_immediate());
                    (0, 17)
                } else {
                    (3, 11)
                }
            }
            // CALL adr
            0xcd => {
                self.call(self.read_bytes_immediate());
                (0, 17)
            }
            // RNC
            0xd0 => {
                if self.flags.carry {
                    (1, 5)
                } else {
                    self.ret();
                    (0, 11)
                }
            }
            // POP D
            0xd1 => {
                *self.de_mut() = self.pop();
                (1, 10)
            }
            // JNC adr
            0xd2 => {
                if self.flags.carry {
                    (3, 10)
                } else {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                }
            }
            // OUT D8
            0xd3 => {
                io_state.output(self.read_byte_immediate(), self.a);
                (2, 10)
            }
            // CNC adr
            0xd4 => {
                if self.flags.carry {
                    (3, 11)
                } else {
                    self.call(self.read_bytes_immediate());
                    (0, 17)
                }
            }
            // PUSH D
            0xd5 => {
                self.push(self.de());
                (1, 11)
            }
            // SUI d8
            0xd6 => {
                self.sub(self.read_byte_immediate());
                (2, 7)
            }
            // RC
            0xd8 => {
                if self.flags.carry {
                    self.ret();
                    (0, 11)
                } else {
                    (1, 5)
                }
            }
            // JC adr
            0xda => {
                if self.flags.carry {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                } else {
                    (3, 10)
                }
            }
            // IN D8
            0xdb => {
                self.a = io_state.input(self.read_byte_immediate());
                (2, 10)
            }
            // SBI D8
            0xde => {
                self.sbb(self.read_byte_immediate());
                (2, 7)
            }
            // POP H
            0xe1 => {
                *self.hl_mut() = self.pop();
                (1, 10)
            }
            // JPO adr
            0xe2 => {
                if self.flags.parity {
                    (3, 10)
                } else {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                }
            }
            // XTHL
            0xe3 => {
                let tmp = self.hl();
                *self.hl_mut() = self.pop();
                self.push(tmp);
                (1, 18)
            }
            // PUSH H
            0xe5 => {
                self.push(self.hl());
                (1, 11)
            }
            // ANI D8
            0xe6 => {
                self.ani();
                (2, 7)
            }
            0xe9 => {
                self.jmp(self.hl());
                (0, 5)
            }
            // JPE adr
            0xea => {
                if self.flags.parity {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                } else {
                    (3, 10)
                }
            }
            // XCHG
            0xeb => {
                let tmp = self.de();
                *self.de_mut() = self.hl();
                *self.hl_mut() = tmp;
                (1, 5)
            }
            // POP AF
            0xf1 => {
                let pop = self.pop();
                self.set_af(pop);
                (1, 10)
            }
            // JP adr
            0xf2 => {
                if self.flags.sign {
                    (3, 10)
                } else {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                }
            }
            // DI
            0xf3 => {
                self.interrupts_enabled = false;
                (1, 4)
            }
            // PUSH AF
            0xf5 => {
                self.push(self.af());
                (1, 11)
            }
            // ORI d8
            0xf6 => {
                self.or(self.read_byte_immediate());
                (2, 7)
            }
            // JM adr
            0xfa => {
                if self.flags.sign {
                    self.jmp(self.read_bytes_immediate());
                    (0, 10)
                } else {
                    (3, 10)
                }
            }
            // EI
            0xfb => {
                self.interrupts_enabled = true;
                (1, 4)
            }
            // CPI D8
            0xfe => {
                self.cmp(self.read_byte_immediate());
                (2, 7)
            }
            // Unimplemented
            _ => {
                println!(
                    "Unimplemented instruction: {:04x} {:02x} {}",
                    self.pc,
                    op_code,
                    self.next_opcode()
                );
                process::exit(0)
            }
        };

        self.pc += op_length;
        cycles
    }

    /// Returns the name of the instruction at the specified address in memory
    fn op_name(&self, address: u16) -> String {
        match self.read_byte(address) {
            0x00 | 0x08 | 0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => "NOP".into(),
            0x01 => format!("LXI B, ${:04x}", self.read_bytes(address + 1)),
            0x02 => "STAX B".into(),
            0x03 => "INX B".into(),
            0x04 => "INR B".into(),
            0x05 => "DCR B".into(),
            0x06 => format!("MVI B, ${:02x}", self.read_byte(address + 1)),
            0x07 => "RLC".into(),
            0x09 => "DAD B".into(),
            0x0a => "LDAX B".into(),
            0x0b => "DCX B".into(),
            0x0c => "INR C".into(),
            0x0d => "DCR C".into(),
            0x0e => format!("MVI C, ${:02x}", self.read_byte(address + 1)),
            0x0f => "RRC".into(),
            0x11 => format!("LXI D, ${:04x}", self.read_bytes(address + 1)),
            0x12 => "STAX D".into(),
            0x13 => "INX D".into(),
            0x14 => "INR D".into(),
            0x15 => "DCR D".into(),
            0x16 => format!("MVI D, ${:02x}", self.read_byte(address + 1)),
            0x17 => "RAL".into(),
            0x19 => "DAD D".into(),
            0x1a => "LDAX D".into(),
            0x1b => "DCX D".into(),
            0x1c => "INR E".into(),
            0x1d => "DCR E".into(),
            0x1e => format!("MVI E, ${:02x}", self.read_byte(address + 1)),
            0x1f => "RAR".into(),
            0x21 => format!("LXI H, ${:04x}", self.read_bytes(address + 1)),
            0x22 => format!("SHLD ${:04x}", self.read_bytes(address + 1)),
            0x23 => "INX H".into(),
            0x24 => "INR H".into(),
            0x25 => "DCR H".into(),
            0x26 => format!("MVI H, ${:02x}", self.read_byte(address + 1)),
            0x27 => "DAA".into(),
            0x29 => "DAD H".into(),
            0x2a => format!("LHLD ${:04x}", self.read_bytes(address + 1)),
            0x2b => "DCX H".into(),
            0x2c => "INR L".into(),
            0x2d => "DCR L".into(),
            0x2e => format!("MVI L, ${:02x}", self.read_byte(address + 1)),
            0x2f => "CMA".into(),
            0x31 => format!("LXI SP, ${:04x}", self.read_bytes(address + 1)),
            0x32 => format!("STA ${:04x}", self.read_bytes(address + 1)),
            0x33 => "INX SP".into(),
            0x34 => "INR M".into(),
            0x35 => "DCR M".into(),
            0x36 => format!("MVI M, ${:02x}", self.read_byte(address + 1)),
            0x37 => "STC".into(),
            0x39 => "DAD SP".into(),
            0x3a => format!("LDA ${:04x}", self.read_bytes(address + 1)),
            0x3b => "DCX SP".into(),
            0x3c => "INR A".into(),
            0x3d => "DCR A".into(),
            0x3e => format!("MVI A, ${:02x}", self.read_byte(address + 1)),
            0x3f => "CMC".into(),
            0x40 => "MOV B,B".into(),
            0x41 => "MOV B,C".into(),
            0x42 => "MOV B,D".into(),
            0x43 => "MOV B,E".into(),
            0x44 => "MOV B,H".into(),
            0x45 => "MOV B,L".into(),
            0x46 => "MOV B,M".into(),
            0x47 => "MOV B,A".into(),
            0x48 => "MOV C,B".into(),
            0x49 => "MOV C,C".into(),
            0x4a => "MOV C,D".into(),
            0x4b => "MOV C,E".into(),
            0x4c => "MOV C,H".into(),
            0x4d => "MOV C,L".into(),
            0x4e => "MOV C,M".into(),
            0x4f => "MOV C,A".into(),
            0x50 => "MOV D,B".into(),
            0x51 => "MOV D,C".into(),
            0x52 => "MOV D,D".into(),
            0x53 => "MOV D,E".into(),
            0x54 => "MOV D,H".into(),
            0x55 => "MOV D,L".into(),
            0x56 => "MOV D,M".into(),
            0x57 => "MOV D,A".into(),
            0x58 => "MOV E,B".into(),
            0x59 => "MOV E,C".into(),
            0x5a => "MOV E,D".into(),
            0x5b => "MOV E,E".into(),
            0x5c => "MOV E,H".into(),
            0x5d => "MOV E,L".into(),
            0x5e => "MOV E,M".into(),
            0x5f => "MOV E,A".into(),
            0x60 => "MOV H,B".into(),
            0x61 => "MOV H,C".into(),
            0x62 => "MOV H,D".into(),
            0x63 => "MOV H,E".into(),
            0x64 => "MOV H,H".into(),
            0x65 => "MOV H,L".into(),
            0x66 => "MOV H,M".into(),
            0x67 => "MOV H,A".into(),
            0x68 => "MOV L,B".into(),
            0x69 => "MOV L,C".into(),
            0x6a => "MOV L,D".into(),
            0x6b => "MOV L,E".into(),
            0x6c => "MOV L,H".into(),
            0x6d => "MOV L,L".into(),
            0x6e => "MOV L,M".into(),
            0x6f => "MOV L,A".into(),
            0x70 => "MOV M,B".into(),
            0x71 => "MOV M,C".into(),
            0x72 => "MOV M,D".into(),
            0x73 => "MOV M,E".into(),
            0x74 => "MOV M,H".into(),
            0x75 => "MOV M,L".into(),
            0x76 => "HLT".into(),
            0x77 => "MOV M,A".into(),
            0x78 => "MOV A,B".into(),
            0x79 => "MOV A,C".into(),
            0x7a => "MOV A,D".into(),
            0x7b => "MOV A,E".into(),
            0x7c => "MOV A,H".into(),
            0x7d => "MOV A,L".into(),
            0x7e => "MOV A,M".into(),
            0x7f => "MOV A,A".into(),
            0x80 => "ADD B".into(),
            0x81 => "ADD C".into(),
            0x82 => "ADD D".into(),
            0x83 => "ADD E".into(),
            0x84 => "ADD H".into(),
            0x85 => "ADD L".into(),
            0x86 => "ADD M".into(),
            0x87 => "ADD A".into(),
            0x88 => "ADC B".into(),
            0x89 => "ADC C".into(),
            0x8a => "ADC D".into(),
            0x8b => "ADC E".into(),
            0x8c => "ADC H".into(),
            0x8d => "ADC L".into(),
            0x8e => "ADC M".into(),
            0x8f => "ADC A".into(),
            0x90 => "SUB B".into(),
            0x91 => "SUB C".into(),
            0x92 => "SUB D".into(),
            0x93 => "SUB E".into(),
            0x94 => "SUB H".into(),
            0x95 => "SUB L".into(),
            0x96 => "SUB M".into(),
            0x97 => "SUB A".into(),
            0x98 => "SBB B".into(),
            0x99 => "SBB C".into(),
            0x9a => "SBB D".into(),
            0x9b => "SBB E".into(),
            0x9c => "SBB H".into(),
            0x9d => "SBB L".into(),
            0x9e => "SBB M".into(),
            0x9f => "SBB A".into(),
            0xa0 => "ANA B".into(),
            0xa1 => "ANA C".into(),
            0xa2 => "ANA D".into(),
            0xa3 => "ANA E".into(),
            0xa4 => "ANA H".into(),
            0xa5 => "ANA L".into(),
            0xa6 => "ANA M".into(),
            0xa7 => "ANA A".into(),
            0xa8 => "XRA B".into(),
            0xa9 => "XRA C".into(),
            0xaa => "XRA D".into(),
            0xab => "XRA E".into(),
            0xac => "XRA H".into(),
            0xad => "XRA L".into(),
            0xae => "XRA M".into(),
            0xaf => "XRA A".into(),
            0xb0 => "ORA B".into(),
            0xb1 => "ORA C".into(),
            0xb2 => "ORA D".into(),
            0xb3 => "ORA E".into(),
            0xb4 => "ORA H".into(),
            0xb5 => "ORA L".into(),
            0xb6 => "ORA M".into(),
            0xb7 => "ORA A".into(),
            0xb8 => "CMP B".into(),
            0xb9 => "CMP C".into(),
            0xba => "CMP D".into(),
            0xbb => "CMP E".into(),
            0xbc => "CMP H".into(),
            0xbd => "CMP L".into(),
            0xbe => "CMP M".into(),
            0xbf => "CMP A".into(),
            0xc0 => "RNZ".into(),
            0xc1 => "POP B".into(),
            0xc2 => format!("JNZ ${:04x}", self.read_bytes(address + 1)),
            0xc3 | 0xcb => format!("JMP ${:04x}", self.read_bytes(address + 1)),
            0xc4 => format!("CNZ ${:04x}", self.read_bytes(address + 1)),
            0xc5 => "PUSH B".into(),
            0xc6 => format!("ADI ${:02x}", self.read_byte(address + 1)),
            0xc7 => "RST 0".into(),
            0xc8 => "RZ".into(),
            0xc9 | 0xd9 => "RET".into(),
            0xca => format!("JZ ${:04x}", self.read_bytes(address + 1)),
            0xcc => format!("CZ ${:04x}", self.read_bytes(address + 1)),
            0xcd | 0xdd | 0xed | 0xfd => format!("CALL ${:04x}", self.read_bytes(address + 1)),
            0xce => format!("ACI ${:02x}", self.read_byte(address + 1)),
            0xcf => "RST 1".into(),
            0xd0 => "RNC".into(),
            0xd1 => "POP D".into(),
            0xd2 => format!("JNC ${:04x}", self.read_bytes(address + 1)),
            0xd3 => format!("OUT ${:02x}", self.read_byte(address + 1)),
            0xd4 => format!("CNC ${:04x}", self.read_bytes(address + 1)),
            0xd5 => "PUSH D".into(),
            0xd6 => format!("SUI ${:02x}", self.read_byte(address + 1)),
            0xd7 => "RST 2".into(),
            0xd8 => "RC".into(),
            0xda => format!("JC ${:04x}", self.read_bytes(address + 1)),
            0xdb => format!("IN ${:02x}", self.read_byte(address + 1)),
            0xdc => format!("CC ${:04x}", self.read_bytes(address + 1)),
            0xde => "SBI D8".into(),
            0xdf => "RST 3".into(),
            0xe0 => "RPO".into(),
            0xe1 => "POP H".into(),
            0xe2 => format!("JPO ${:04x}", self.read_bytes(address + 1)),
            0xe3 => "XTHL".into(),
            0xe4 => format!("CPO ${:04x}", self.read_bytes(address + 1)),
            0xe5 => "PUSH H".into(),
            0xe6 => format!("ANI ${:02x}", self.read_byte(address + 1)),
            0xe7 => "RST 4".into(),
            0xe8 => "RPE".into(),
            0xe9 => "PCHL".into(),
            0xea => format!("JPE ${:04x}", self.read_bytes(address + 1)),
            0xeb => "XCHG".into(),
            0xec => format!("CPE ${:04x}", self.read_bytes(address + 1)),
            0xee => format!("XRI ${:02x}", self.read_byte(address + 1)),
            0xef => "RST 5".into(),
            0xf0 => "RP".into(),
            0xf1 => "POP AF".into(),
            0xf2 => format!("JP ${:04x}", self.read_bytes(address + 1)),
            0xf3 => "DI".into(),
            0xf4 => format!("CP ${:04x}", self.read_bytes(address + 1)),
            0xf5 => "PUSH AF".into(),
            0xf6 => format!("ORI ${:02x}", self.read_byte(address + 1)),
            0xf7 => "RST 6".into(),
            0xf8 => "RM".into(),
            0xf9 => "SPHL".into(),
            0xfa => format!("JM ${:04x}", self.read_bytes(address + 1)),
            0xfb => "EI".into(),
            0xfc => format!("CM ${:04x}", self.read_bytes(address + 1)),
            0xfe => format!("CPI ${:02x}", self.read_byte(address + 1)),
            0xff => "RST 7".into(),
        }
    }
}
