#![allow(unused_variables, non_snake_case, dead_code)]

use std::cmp::PartialEq;
use std::thread::sleep;
use crate::initialize::initializable;
use crate::mmu::MMU;
use crate::mode::{ClockSpeed, HardwareMode};

const IF: u16 = 0xFF0F;
const IE: u16 = 0xFFFF;
const KEY1: u16 = 0xFF4D;

pub struct Instruction{
    opcode: u16,
    mnemonic: String,
    length: u8,
    cycles: u8,         // using M-Cycles
}

impl Instruction{
    pub fn new(opcode: u16) -> Self{
        let mut mnemonic;
        let mut length: u8 = 1;
        let mut cycles: u8 = 4;
        if (opcode & 0xFF00) >> 8 == 0xCB{
            let source = match opcode & 0x07 {
                0 => "B",
                1 => "C",
                2 => "D",
                3 => "E",
                4 => "H",
                5 => "L",
                6 => "(HL)",
                7 => "A",
                _ => unreachable!(),
            };
            mnemonic = match opcode & 0xFF {
                0x00..=0x07 => format!("RLC {}", source),
                0x08..=0x0F => format!("RRC {}", source),
                0x10..=0x17 => format!("RL {}", source),
                0x18..=0x1F => format!("RR {}", source),
                0x20..=0x27 => format!("SLA {}", source),
                0x28..=0x2F => format!("SRA {}", source),
                0x30..=0x37 => format!("SWAP {}", source),
                0x38..=0x3F => format!("SRL {}", source),
                0x40..=0x7F => {
                    let b = ((opcode & 0xFF) - 0x40) / 0x08;
                    format!("BIT {}, {}", b, source)
                },
                0x80..=0xBF => {
                    let b = ((opcode & 0xFF) - 0x80) / 0x08;
                    format!("RES {}, {}", b, source)
                },
                0xC0..=0xFF => {
                    let b = ((opcode & 0xFF) - 0xC0) / 0x08;
                    format!("SET {}, {}", b, source)
                }
                _ => unreachable!(),
            };
            length = 2;
            cycles = 8;
        } else {
            // normal instructions
            mnemonic = match opcode & 0xFF {
                0x00 => "NOP".to_string(),
                0x01 | 0x11 | 0x21 | 0x31 => {
                    let src = Self::double_register_pair(0x00, (opcode & 0xFF) as u8);
                    cycles = 12;
                    length = 3;
                    format!("LD {}, d16", src)
                }
                0x02 | 0x12 | 0x22 | 0x32 | 0x0A | 0x1A | 0x2A | 0x3A => {
                    let address_pair = Self::memory_address_pair(0x00, (opcode & 0xFF) as u8);
                    cycles = 8;
                    if (opcode & 0xFF) <= 0x32{
                        format!("LD {}, A", address_pair)
                    } else {
                        format!("LD A, {}", address_pair)
                    }
                },
                0x03 | 0x13 | 0x23 | 0x33 =>{
                    cycles = 8;
                    format!("INC {}", Self::double_register_pair(0x00, (opcode & 0xFF) as u8))
                },
                0x04 | 0x14 | 0x24 | 0x34 => {
                    if opcode & 0xFF == 0x34{
                        cycles = 12
                    }
                    format!("INC {}", Self::reg_or_hl(0x00, (opcode & 0xFF) as u8))
                },
                0x05 | 0x15 | 0x25 | 0x35 => {
                    if opcode & 0xFF == 0x35{
                        cycles = 12
                    }
                    format!("DEC {}", Self::reg_or_hl(0x00, (opcode & 0xFF) as u8))
                },
                0x06 | 0x16 | 0x26 | 0x36 => {
                    length = 2;
                    if opcode & 0xFF == 0x36{
                        cycles = 12;
                    } else {
                        cycles = 8;
                    }
                    format!("LD {}, d8", Self::reg_or_hl(0x00, (opcode & 0xFF) as u8))
                },
                0x07 => {
                    "RLCA".to_string()
                },
                0x08 => {
                    length = 3;
                    cycles = 20;
                    "LD (a16), SP".to_string()
                },
                0x09 | 0x19 | 0x29 | 0x39 => {
                    let reg_pair = Self::register_pair_index(0x00, (opcode & 0xFF) as u8);
                    cycles = 8;
                    format!("ADD HL, {}", reg_pair)
                },
                0x0B | 0x1B | 0x2B | 0x3B => {
                    cycles = 8;
                    format!("DEC {}", Self::double_register_pair(0x00, (opcode & 0xFF) as u8))
                },
                0x0C | 0x1C | 0x2C | 0x3C => {
                    format!("INC {}", Self::reg_or_accumulator(0x00, (opcode & 0xFF) as u8))
                },
                0x0D | 0x1D | 0x2D | 0x3D => {
                    format!("DEC {}", Self::reg_or_accumulator(0x00, (opcode & 0xFF) as u8))
                },
                0x0E | 0x1E | 0x2E | 0x3E => {
                    length = 2;
                    cycles = 8;
                    format!("LD {}, d8", Self::reg_or_accumulator(0x00, (opcode & 0xFF) as u8))
                },
                0x0F => "RRCA".to_string(),
                0x10 => {
                    length = 2;
                    "STOP".to_string()
                },
                0x17 => "RLA".to_string(),
                0x18 | 0x28 | 0x38 => {
                    length = 2;
                    cycles = 8;
                    if opcode & 0xFF == 0x18{
                        "JR s8".to_string()
                    } else{
                        let r = match opcode & 0xFF{
                            0x28 => "Z",
                            _ => "C"
                        };
                        format!("JR {}, s8", r)
                    }
                },
                0x1F => "RRA".to_string(),
                0x20 | 0x30 => {
                    length = 2;
                    cycles = 8;
                    if opcode & 0xFF == 0x20{
                        "JR NZ, s8".to_string()
                    } else{
                        "JR NC, s8".to_string()
                    }
                },
                0x27 => "DAA".to_string(),
                0x2F => "CPL".to_string(),
                0x37 => "SCF".to_string(),
                0x3F => "CCF".to_string(),
                0x40..=0x7F => {
                    let mut m = String::new();
                    if (opcode & 0xFF) == 0x76 {
                        m = "HALT".to_string();
                    } else {
                        let src = opcode & 0x07;
                        let dst = (opcode >> 3) & 0x07;

                        let val = match src {
                            0 => "B",
                            1 => "C",
                            2 => "D",
                            3 => "E",
                            4 => "H",
                            5 => "L",
                            6 => "(HL)",
                            7 => "A",
                            _ => unreachable!()
                        };
                        m = match dst {
                            0 => format!("LD B, {}", val),
                            1 => format!("LD C, {}", val),
                            2 => format!("LD D, {}", val),
                            3 => format!("LD E, {}", val),
                            4 => format!("LD H, {}", val),
                            5 => format!("LD L, {}", val),
                            6 => format!("LD (HL), {}", val),
                            7 => format!("LD A, {}", val),
                            _ => unreachable!()
                        };
                    }
                    mnemonic = m.clone();
                    m
                },
                0x80..=0xBF => {
                    let src = match opcode & 0x07 {
                        0x0 => "B",
                        0x1 => "C",
                        0x2 => "D",
                        0x3 => "E",
                        0x4 => "H",
                        0x5 => "L",
                        0x6 => {
                            cycles = 8;
                            "(HL)"
                        },
                        0x7 => "A",
                        _ => unreachable!()
                    };
                    match opcode & 0xFF{
                        0x80..=0x87 => {
                            format!("ADD A, {}", src)
                        },
                        0x88..=0x8F => {
                            format!("ADC A, {}", src)
                        },
                        0x90..=0x97 => {
                            format!("SUB A, {}", src)
                        },
                        0x98..=0x9F => {
                            format!("SBC A, {}", src)
                        },
                        0xA0..=0xA7 => {
                            format!("AND A, {}", src)
                        },
                        0xA8..=0xAF => {
                            format!("XOR A, {}", src)
                        },
                        0xB0..=0xB7 => {
                            format!("OR A, {}", src)
                        },
                        0xB8..=0xBF => {
                            format!("CP A, {}", src)
                        },
                        _ => unreachable!()
                    }
                },
                0xC0 | 0xD0 => {
                    cycles = 8;
                    let f = if (opcode & 0xFF) == 0xC0 {"NZ"} else {"NC"};
                    format!("RET {}", f)
                },
                0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                    cycles = 12;
                    format!("POP {}", Self::stack_double_pair(0xC0, (opcode & 0xFF) as u8))
                },
                0xC2 | 0xD2 => {
                    length = 3;
                    cycles = 12;
                    let f = if (opcode & 0xFF) == 0xC0 {"NZ"} else {"NC"};
                    format!("JP {}, a16", f)
                },
                0xC3 => {
                    cycles = 16;
                    length = 3;
                    "JP a16".to_string()
                },
                0xC4 | 0xD4 => {
                    cycles = 12;
                    length = 3;
                    let f = if (opcode & 0xFF) == 0xC0 {"NZ"} else {"NC"};
                    format!("CALL {}, a16", f)
                },
                0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                    cycles = 16;
                    format!("PUSH {}", Self::stack_double_pair(0xC0, (opcode & 0xFF) as u8))
                },
                0xC6 | 0xD6 | 0xE6 | 0xF6 | 0xCE | 0xDE | 0xEE | 0xFE => {
                    cycles = 8;
                    length = 2;
                    let op = match opcode & 0xFF{
                        0xC6 => "ADD",
                        0xCE => "ADC",
                        0xD6 => "SUB",
                        0xDE => "SBC",
                        0xE6 => "AND",
                        0xEE => "XOR",
                        0xF6 => "OR",
                        0xFE => "CP",
                        _ => unreachable!()
                    };
                    format!("{} A, d8", op)
                },
                0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                    cycles = 16;
                    let idx = (((opcode & 0xFF) >> 3) & 0x0F) - 0x08;
                    format!("RST {}", idx)
                },
                0xC8 | 0xD8 => {
                    cycles = 8;
                    let f = if (opcode & 0xFF) == 0xC8 {"Z"} else {"C"};
                    format!("RET {}", f)
                },
                0xC9 | 0xD9 => {
                    cycles = 16;
                    let end = if (opcode & 0xFF) == 0xC9 {""} else {"I"};
                    format!("RET{}", end)
                },
                0xCA | 0xDA | 0xCC | 0xDC => {
                    length = 3;
                    cycles = 12;
                    let f = if (opcode & 0xF0) == 0xC0 {"Z"} else {"C"};
                    if (opcode & 0x0F) == 0x0C {format!("CALL {}, a16", f)} else {format!("JP {}, a16", f)}
                },
                0xCD => {
                    cycles = 24;
                    length = 3;
                    "CALL a16".to_string()
                },
                0xE0 | 0xF0 => {
                    cycles = 12;
                    length = 2;
                    let a = "(a8)".to_string();
                    let b = "A".to_string();
                    if (opcode & 0xFF) == 0xE0 {
                        format!("LD {}, {}", a, b)
                    } else {
                        format!("LD {}, {}", b, a)
                    }
                },
                0xE2 | 0xF2 => {
                    cycles = 8;
                    let a = "(C)".to_string();
                    let b = "A".to_string();
                    if (opcode & 0xFF) == 0xE2 {
                        format!("LD {}, {}", a, b)
                    } else {
                        format!("LD {}, {}", b, a)
                    }
                },
                0xF3 => {
                    length = 1;
                    cycles = 4;
                    "DI".to_string()
                },
                0xE8 => {
                    cycles = 16;
                    length = 2;
                    "ADD SP, s8".to_string()
                },
                0xE9 => "JP HL".to_string(),
                0xEA | 0xFA => {
                    cycles = 16;
                    length = 3;
                    let a = "(a16)".to_string();
                    let b = "A".to_string();
                    if (opcode & 0xFF) == 0xEA {
                        format!("LD {}, {}", a, b)
                    } else {
                        format!("LD {}, {}", b, a)
                    }
                },
                0xF8 => {
                    length = 2;
                    cycles = 12;
                    "LD HL, SP+s8".to_string()
                },
                0xF9 => {
                    cycles = 8;
                    "LD SP, HL".to_string()
                },
                0xFB => {
                    length = 1;
                    cycles = 4;
                    "EI".to_string()
                },
                _ => format!("UNKNOWN {:02x}", opcode & 0xFF)
            }
        }

        if (opcode & 0xFF00) >> 8 == 0xCB {
            Self {
                opcode,
                mnemonic,
                length,
                cycles
            }
        } else {
            Self {
                opcode: opcode & 0xFF,
                mnemonic,
                length,
                cycles
            }
        }

    }

    fn register_pair_index(start: u8, n: u8) -> u8 {
        ((n - start) & 0xF0) >> 4
    }

    /// BC, DE, HL, SP
    /// # Arguments:
    /// start - Opcode row
    /// n - opcode
    pub fn double_register_pair(start: u8, n: u8) -> &'static str{
        match Self::register_pair_index(start, n) {
            0x0 => "BC",
            0x1 => "DE",
            0x2 => "HL",
            0x3 => "SP",
            _ => unreachable!(),
        }
    }

    pub fn memory_address_pair(start: u8, n: u8) -> &'static str{
        match Self::register_pair_index(start, n) {
            0x0 => "(BC)",
            0x1 => "(DE)",
            0x2 => "(HL+)",
            0x3 => "(HL-)",
            _ => unreachable!(),
        }
    }

    /// see 0x04, 0x15, 0x24, 0x34
    pub fn reg_or_hl(start: u8, n: u8) -> &'static str {
        match Self::register_pair_index(start, n) {
            0x0 => "B",
            0x1 => "D",
            0x2 => "H",
            0x3 => "(HL)",
            _ => unreachable!(),
        }
    }

    /// see 0x0C, 0x1C, 0x2C, 0x3C
    pub fn reg_or_accumulator(start: u8, n: u8) -> &'static str {
        match Self::register_pair_index(start, n) {
            0x0 => "C",
            0x1 => "E",
            0x2 => "L",
            0x3 => "A",
            _ => unreachable!(),
        }
    }

    pub fn stack_double_pair(start: u8, n: u8) -> &'static str {
        match Self::register_pair_index(start, n) {
            0x0 => "BC",
            0x1 => "DE",
            0x2 => "HL",
            0x3 => "AF",
            _ => unreachable!(),
        }
    }
}

pub struct Cpu {
    pub pc: u16,
    pub sp: u16,
    pub a: u8, pub f: u8,
    pub b: u8, pub c: u8,
    pub d: u8, pub e: u8,
    pub h: u8, pub l: u8,

    last_instruction_length: usize,
    has_bios: bool,
    ime: bool,
    ime_delay: u8,
    halt: bool,
    stopped: bool,
    mode: HardwareMode,
    speed: ClockSpeed,
    pub speed_change_timer: u16,
    halt_bug: bool,
}

impl Cpu{
    pub fn new(is_cgb: bool, has_bios: bool) -> Self {
        let mut res = Self {
            pc: 0,
            sp: 0,
            a: 0, f: 0,
            b: 0, c: 0,
            d: 0, e: 0,
            h: 0, l: 0,
            last_instruction_length: 0,
            stopped: false,
            has_bios,
            ime: false,
            ime_delay: 0,
            halt: false,
            mode: if is_cgb {HardwareMode::Cgb } else {HardwareMode::Dmg},
            speed: ClockSpeed::Normal,
            speed_change_timer: 0,
            halt_bug: false,
        };
        res.init();
        res
    }
    pub fn double_register(high: u8, low: u8) -> u16{
        ((high as u16) << 8) | low as u16
    }
    pub fn AF(&self) -> u16{
        Self::double_register(self.a, self.f)
    }

    pub fn BC(&self) -> u16{
        Self::double_register(self.b, self.c)
    }

    pub fn DE(&self) -> u16{
        Self::double_register(self.d, self.e)
    }

    pub fn HL(&self) -> u16{
        Self::double_register(self.h, self.l)
    }

    pub fn set_AF(&mut self, value: u16){
        self.a = (value >> 8) as u8;
        self.f = (value & 0xFF) as u8 & 0xF0; // lowest nibble of F is always 0ed
    }

    pub fn set_BC(&mut self, value: u16){
        self.b = (value >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }

    pub fn set_DE(&mut self, value: u16){
        self.d = (value >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }

    pub fn set_HL(&mut self, value: u16){
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }

    fn set_bit(dest: &mut u8, bit: u8, value: bool){
        if bit > 8{
            println!("bit <{}> is out of bounds", bit);
            return;
        }
        *dest = if value {*dest | 1 << bit} else {*dest & !(1 << bit)};
    }
    pub fn set_Z(&mut self, v: bool){
        Self::set_bit(&mut self.f, 7, v);
    }

    pub fn set_N(&mut self, v: bool){
        Self::set_bit(&mut self.f, 6, v);
    }

    pub fn set_H(&mut self, v: bool){
        Self::set_bit(&mut self.f, 5, v);
    }

    pub fn set_C(&mut self, v: bool){
        Self::set_bit(&mut self.f, 4, v);
    }

    fn get_bit(&self, reg: u8, bit: u8) -> u8{
        (reg >> bit) & 0x01
    }

    pub fn get_Z(&self) -> bool{
        self.get_bit(self.f, 7) == 1
    }

    pub fn get_N(&self) -> bool{
        self.get_bit(self.f, 6) == 1
    }

    pub fn get_H(&self) -> bool{
        self.get_bit(self.f, 5) == 1
    }

    pub fn get_C(&self) -> bool{
        self.get_bit(self.f, 4) == 1
    }




    pub fn d8(&self, mmu: &MMU) -> u8 {
        mmu.rb(self.pc.wrapping_add(1))
    }

    pub fn d16(&self, mmu: &MMU) -> u16 {
        mmu.rb(self.pc.wrapping_add(1)) as u16 | ((mmu.rb(self.pc.wrapping_add(2)) as u16) << 8)
    }

    pub fn a8(&self, mmu: &MMU) -> u16 {
        0xFF00 | self.d8(mmu) as u16
    }

    pub fn a16(&self, mmu: &MMU) -> u16 {
        self.d16(mmu)
    }

    pub fn r8(&self, mmu: &MMU) -> i8 {
        mmu.rb(self.pc+1) as i8
    }


    fn vertical_range(start: u8, stop: u8) -> impl Iterator<Item = u8> {
        (start..=stop).step_by(0x10)
    }
    pub fn execute(&mut self, mmu: &mut MMU) -> u8 {
        self.last_instruction_length = 0;
        let instr_opcode = if mmu.rb(self.pc) == 0xCB {
            ((mmu.rb(self.pc) as u16) << 8) | (mmu.rb(self.pc + 1) as u16)
        } else {
            mmu.rb(self.pc) as u16
        };
        if self.speed_change_timer > 0 {
           if self.speed == ClockSpeed::Double {
               self.speed_change_timer = self.speed_change_timer.wrapping_sub(2);
           }
            self.speed_change_timer = self.speed_change_timer.wrapping_sub(4);
            return 4
        }
        let instr = Instruction::new(instr_opcode);
        // println!("PC:{:4x}, Opcode: {:4x} {} {:2x}", self.pc, mmu.rw(self.pc), instr.mnemonic, instr.opcode);
        // sleep(core::time::Duration::from_millis(10));
        let mut cycles = 67;

        if self.speed_change_timer > 0{
            self.speed_change_timer -= 4;
            return 4
        }

        self.service_interrupt(mmu);
        self.last_instruction_length = instr.length as usize;

        if instr_opcode & 0xFF00 == 0xCB00 {
            match instr_opcode as u8 {
                0x00..=0x07 => cycles = self.RLC(instr_opcode, mmu),
                0x08..=0x0F => cycles = self.RRC(instr_opcode, mmu),
                0x10..=0x17 => cycles = self.RL(instr_opcode, mmu),
                0x18..=0x1F => cycles = self.RR(instr_opcode, mmu),
                0x20..=0x27 => cycles = self.SLA(instr_opcode, mmu),
                0x28..=0x2F => cycles = self.SRA(instr_opcode, mmu),
                0x30..=0x37 => cycles = self.SWAP(instr_opcode, mmu),
                0x38..=0x3F => cycles = self.SRL(instr_opcode, mmu),
                0x40..=0x7F => cycles = self.BIT(instr_opcode, mmu),
                0x80..=0xBF => cycles = self.RES(instr_opcode, mmu),
                0xC0..=0xFF => cycles = self.SET(instr_opcode, mmu),
            }
        } else {
            match instr_opcode as u8 {
                0x00 => cycles = self.NOP(),
                opcode if Self::vertical_range(0x01, 0x31).any(|v| v == opcode)
                    || Self::vertical_range(0x02, 0x32).any(|v| v == opcode)
                    || Self::vertical_range(0x06, 0x36).any(|v| v == opcode)
                    || Self::vertical_range(0x0A, 0x3A).any(|v| v == opcode)
                    || opcode == 0x08
                    || Self::vertical_range(0xE0, 0xF0).any(|v| v == opcode)
                    || Self::vertical_range(0xE2, 0xF2).any(|v| v == opcode)
                    || (0xF8..=0xFA).contains(&opcode)
                    || opcode == 0xEA
                    || Self::vertical_range(0x0E, 0x3E).any(|v| v == opcode)
                    || (0x40..=0x7F).contains(&opcode) =>
                    {
                        cycles = self.LD(instr_opcode, mmu);
                    },
                0x03 | 0x13 | 0x23 | 0x33 | 0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C =>  {
                    cycles = self.INC(instr_opcode, mmu);
                },
                0x05 | 0x15 | 0x25 | 0x35 | 0x0B | 0x1B | 0x2B | 0x3B | 0x0D | 0x1D | 0x2D | 0x3D => cycles = self.DEC(instr_opcode, mmu),
                0x07 => cycles = self.RLCA(instr_opcode),
                0x0F => cycles = self.RRCA(instr_opcode),
                0x17 => cycles = self.RLA(instr_opcode),
                0x1F => cycles = self.RRA(instr_opcode),
                0x10 => cycles = self.STOP(instr_opcode, mmu),
                0x18 | 0x28 | 0x38 | 0x20 | 0x30 |
                0xC2 | 0xD2 | 0xC3 | 0xE9 | 0xCA | 0xDA => {
                    cycles = self.JP(instr_opcode, mmu);
                },
                0x27 => cycles = self.DAA(instr_opcode),
                0x37 => cycles = self.SCF(instr_opcode),
                0x2F => cycles = self.CPL(instr_opcode),
                0x3F => cycles = self.CCF(instr_opcode),
                opcode if Self::vertical_range(0x09, 0x39).any(|v| v == opcode) => {
                    cycles = self.ADD(instr_opcode, mmu);
                },
                0x80..=0x8F | 0xE8 | 0xC6 | 0xCE => cycles = self.ADD(instr_opcode, mmu),
                0x90..=0x9F | 0xD6 | 0xDE => cycles = self.SUB(instr_opcode, mmu),
                0xA0..=0xA7 | 0xE6 => cycles = self.AND(instr_opcode, mmu),
                0xA8..=0xAF | 0xEE => cycles = self.XOR(instr_opcode, mmu),
                0xB0..=0xB7 | 0xF6 => cycles = self.OR(instr_opcode, mmu),
                0xB8..=0xBF | 0xFE => cycles = self.CP(instr_opcode, mmu),
                0xC0 | 0xD0 | 0xC8 | 0xC9 | 0xD8 | 0xD9 => cycles = self.RET(instr_opcode, mmu),
                0xC1 | 0xD1 | 0xE1 | 0xF1 => cycles = self.POP(instr_opcode, mmu),
                0xC5 | 0xD5 | 0xE5 | 0xF5 => cycles = self.PUSH(instr_opcode, mmu),
                0xC4 | 0xD4 | 0xCC | 0xDC | 0xCD => cycles = self.CALL(instr_opcode, mmu),
                0xC7 | 0xD7 | 0xE7 | 0xF7 | 0xCF | 0xDF | 0xEF | 0xFF => cycles = self.RST(instr_opcode, mmu),
                0xF3 => {
                    cycles = 4;
                    self.ime = false
                },
                0xFB => {
                    cycles = 4;
                    self.ime_delay = 2;
                },
                _ => {
                    eprintln!("Unknown opcode {:02x} at PC {:04x}", instr_opcode as u8, self.pc);
                    cycles = 4;
                }
             }
        }

        if self.halt_bug {
            self.halt_bug = false;
        } else {
            self.pc = self.pc.wrapping_add(self.last_instruction_length as u16);
        }



        // delayed by one
        if self.ime_delay > 0{
            self.ime_delay -= 1;
            if self.ime_delay == 0 {
                self.ime = true;
            }
        }

        if cycles == 67 {
            println!("  {:x} didn't set cycles", instr_opcode);
        }
        cycles
    }

    fn service_interrupt(&mut self, mmu: &mut MMU) -> u8{
        let interrupt_flag = mmu.rb(IF) & 0x1F;
        let interrupt_enabled = mmu.rb(IE);

        let fired = interrupt_flag & interrupt_enabled;



        if fired > 0{
            if self.halt{
                self.halt = false
            }
        }

        let fire = fired > 0;
        if (!self.ime || !fire){
            return 0x00
        }

        self.ime = false;

        for i in 0..5{
            if fired & (1 << i) > 0{
                mmu.wb(IF, mmu.rb(IF) & !(1 << i));

                self.push_stack(self.pc, mmu);

                match i {
                    0 => self.pc = 0x40,
                    1 => self.pc = 0x48,
                    2 => self.pc = 0x50,
                    3 => self.pc = 0x58,
                    4 => {
                        self.pc = 0x60;
                        if self.stopped {
                            self.stopped = false;
                        }
                    },
                    _ => {
                        eprintln!("Unknown interrupt {:02x}", i);
                    }
                }
                return 20
            }
        }
        0x00
    }

    // Instructions
    // Every Instruction returns its cycles(in T-cycles)
    fn NOP(&mut self) -> u8{
        self.last_instruction_length = 1;

        4
    }

    fn HALT(&mut self, mmu: &MMU) -> u8{
        let ie = mmu.rb(IE);
        let if_reg = mmu.rb(IF);
        let interrupt_pending = (ie & if_reg & 0x1F) != 0;

        if !self.ime && interrupt_pending {
            self.halt_bug = true;
        } else if self.ime && interrupt_pending {
            self.halt = false;
        } else {
            self.halt = true;
        }
        4
    }

    fn LD(&mut self, opcode: u16, mmu: &mut MMU) -> u8{
        let mut cycles = 8;
        match opcode as u8 {
            0x01 => self.set_BC(self.d16(mmu)),
            0x02 => mmu.wb(self.BC(), self.a),
            0x06 => self.b = self.d8(mmu),
            0x08 => mmu.ww(self.d16(mmu), self.sp),
            0x0A => self.a = mmu.rb(self.BC()),
            0x0E => self.c = self.d8(mmu),
            0x11 => self.set_DE(self.d16(mmu)),
            0x12 => mmu.wb(self.DE(), self.a),
            0x16 => self.d = self.d8(mmu),
            0x1A => self.a = mmu.rb(self.DE()),
            0x1E => self.e = self.d8(mmu),
            0x21 => self.set_HL(self.d16(mmu)),
            0x22 => {
                mmu.wb(self.HL(), self.a);
                self.set_HL(self.HL()+1)
            },
            0x26 => self.h = self.d8(mmu),
            0x2A => {
                self.a = mmu.rb(self.HL());
                self.set_HL(self.HL()+1)
            },
            0x2E => self.l = self.d8(mmu),
            0x31 => self.sp = self.d16(mmu),
            0x32 => {
                mmu.wb(self.HL(), self.a);
                self.set_HL(self.HL()-1)
            }
            0x36 => mmu.wb(self.HL(), self.d8(mmu)),
            0x3A => {
                self.a = mmu.rb(self.HL());
                self.set_HL(self.HL()-1)
            },
            0x3E => self.a = self.d8(mmu),
            0xE0 => mmu.wb(self.a8(mmu), self.a),
            0xE2 => mmu.wb(0xFF00 | self.c as u16, self.a),
            0xEA => mmu.wb(self.a16(mmu), self.a),
            0xF0 => self.a = mmu.rb(self.a8(mmu)),
            0xF2 => self.a = mmu.rb(0xFF00 | self.c as u16),
            0xF8 => {
                let offset = self.r8(mmu);
                let old = self.sp;

                self.set_HL(old.wrapping_add_signed(offset as i16));

                self.set_Z(false);
                self.set_N(false);
                self.set_H((old & 0x0F) + (offset as u8 as u16 & 0x0F) > 0x0F);
                self.set_C((old & 0xFF) + offset as u8 as u16 > 0xFF);
            },
            0xF9 => self.sp = self.HL(),
            0xFA => self.a = mmu.rb(self.a16(mmu)),
            0x40..=0x7F => {
                if opcode == 0x76{
                    return self.HALT(mmu);
                }
                cycles = 4;
                let src = opcode & 0x07;
                let dst = (opcode >> 3) & 0x07;

                let val;
                match src {
                    0 => val = self.b,
                    1 => val = self.c,
                    2 => val = self.d,
                    3 => val = self.e,
                    4 => val = self.h,
                    5 => val = self.l,
                    6 => {
                        cycles = 8;
                        val = mmu.rb(self.HL())
                    },
                    7 => val = self.a,
                    _ => unreachable!()
                };
                match dst {
                    0 => self.b = val,
                    1 => self.c = val,
                    2 => self.d = val,
                    3 => self.e = val,
                    4 => self.h = val,
                    5 => self.l = val,
                    6 => {
                        cycles = 8;
                        mmu.wb(self.HL(), val)
                    },
                    7 => self.a = val,
                    _ => unreachable!()
                }
            },
            _ => unreachable!()
        };
        // forgot in the past code
        match opcode{
            0x01 | 0x11 | 0x21 | 0x31 | 0x36 | 0xE0 | 0xF0 => {
                cycles = 12
            },
            0x08 => cycles = 20,
            0xEA | 0xFA => cycles = 16,
            0xF8 => cycles = 12,
            _ => {/* nothing */},
        }
        cycles
    }

    fn INC(&mut self, opcode: u16, mmu: &mut MMU) -> u8{
        let cycles;
        match opcode & 0x0F {
            0x03 => {
                cycles = 8;
                match opcode {
                    0x33 => self.sp = self.sp.wrapping_add(1),
                    0x03 => self.set_BC(Self::double_register(self.b, self.c).wrapping_add(1)),
                    0x13 => self.set_DE(Self::double_register(self.d, self.e).wrapping_add(1)),
                    0x23 => self.set_HL(Self::double_register(self.h, self.l).wrapping_add(1)),
                    _ => unreachable!()
                }
            },
            _ => {
                if opcode != 0x34 {
                    cycles = 4
                } else {
                    cycles = 12
                }

                let val;
                let res;
                match opcode {
                    0x04 => {
                        val = self.b;
                        res = val.wrapping_add(1);
                        self.b = res;
                    },
                    0x14 => {
                        val = self.d;
                        res = val.wrapping_add(1);
                        self.d = res;
                    },
                    0x24 => {
                        val = self.h;
                        res = val.wrapping_add(1);
                        self.h = res;
                    },
                    0x34 => {
                        let hl = self.HL();
                        val = mmu.rb(hl);
                        res = val.wrapping_add(1);

                        mmu.wb(hl, res);
                    },
                    0x0C => {
                        val = self.c;
                        res = val.wrapping_add(1);
                        self.c = res;
                    },
                    0x1C => {
                        val = self.e;
                        res = val.wrapping_add(1);
                        self.e = res;
                    },
                    0x2C => {
                        val = self.l;
                        res = val.wrapping_add(1);
                        self.l = res;
                    },
                    0x3C => {
                        val = self.a;
                        res = val.wrapping_add(1);
                        self.a = res;
                    }
                    _ => unreachable!()
                };
                self.set_Z(res == 0);
                self.set_N(false);
                self.set_H((val & 0x0F) == 0x0F);
            }
        }

        cycles
    }

    fn DEC(&mut self, opcode: u16, mmu: &mut MMU) -> u8{
        let cycles;
        match opcode & 0x0F {
            0x0B => {
                cycles = 8;
                match opcode {
                    0x3B => self.sp = self.sp.wrapping_sub(1),
                    0x0B => self.set_BC(Self::double_register(self.b, self.c).wrapping_sub(1)),
                    0x1B => self.set_DE(Self::double_register(self.d, self.e).wrapping_sub(1)),
                    0x2B => self.set_HL(Self::double_register(self.h, self.l).wrapping_sub(1)),
                    _ => unreachable!()
                }
            },
            _ => {
                if opcode != 0x34 {
                    cycles = 4
                } else {
                    cycles = 12
                }

                let val;
                let res;
                match opcode {
                    0x05 => {
                        val = self.b;
                        res = val.wrapping_sub(1);
                        self.b = res;
                    },
                    0x15 => {
                        val = self.d;
                        res = val.wrapping_sub(1);
                        self.d = res;
                    },
                    0x25 => {
                        val = self.h;
                        res = val.wrapping_sub(1);
                        self.h = res;
                    },
                    0x35 => {
                        let hl = self.HL();
                        val = mmu.rb(hl);
                        res = val.wrapping_sub(1);

                        mmu.wb(hl, res);
                    },
                    0x0D => {
                        val = self.c;
                        res = val.wrapping_sub(1);
                        self.c = res;
                    },
                    0x1D => {
                        val = self.e;
                        res = val.wrapping_sub(1);
                        self.e = res;
                    },
                    0x2D => {
                        val = self.l;
                        res = val.wrapping_sub(1);
                        self.l = res;
                    },
                    0x3D => {
                        val = self.a;
                        res = val.wrapping_sub(1);
                        self.a = res;
                    }
                    _ => unreachable!()
                };
                self.set_Z(res == 0);
                self.set_N(true);
                self.set_H((val & 0x0F) == 0x00);
            }
        }

        cycles
    }

    fn RLCA(&mut self, _opcode: u16) -> u8{
        let MSB = self.a >> 7;
        // 0111 0111
        // 1110 1110
        let res = (self.a << 1) | MSB;
        self.a = res;
        self.set_Z(false);
        self.set_N(false);
        self.set_H(false);
        self.set_C(MSB == 1);
        4
    }

    fn RRCA(&mut self, _opcode: u16) -> u8{
        let LSB = self.a & 0x01;
        let res = (self.a >> 1) | (LSB << 7);
        self.a = res;
        self.set_Z(false);
        self.set_N(false);
        self.set_H(false);
        self.set_C(LSB == 1);

        4
    }

    fn RRA(&mut self, opcode: u16) -> u8 {
        let LSB = self.a & 0x01;
        self.a = self.a >> 1 | ((self.get_C() as u8) << 7);

        self.set_Z(false);
        self.set_N(false);
        self.set_H(false);
        self.set_C(LSB == 1);

        4
    }

    fn STOP(&mut self, _opcode: u16, mmu: &mut MMU) -> u8{
        if self.mode == HardwareMode::Cgb || self.mode == HardwareMode::GbcCompatible{
        let key1 = mmu.rb(KEY1);
        if key1  & 0x01 == 1{
            if key1 >> 7 == 0{
                mmu.wb(KEY1, 1 << 7);
                self.speed_change_timer = 2050*4;
                self.speed = ClockSpeed::Double;
                mmu.timer.is_double_speed = true;
            } else {
                mmu.wb(KEY1, 0);
                self.speed_change_timer = 2050 * 4;
                self.speed = ClockSpeed::Normal;
                mmu.timer.is_double_speed = false;
            }
            } else {
            self.stopped = true;
        }
        } else {
            self.stopped = true;
        }
        if self.stopped {
            mmu.timer.is_stopped = true;
            mmu.wb(0xFF04, 0x00);
        }
        4
    }


    fn opcode_to_register(&mut self, opcode: u16) -> &mut u8 {
        match opcode & 0x07 {
            0 => &mut self.b,
            1 => &mut self.c,
            2 => &mut self.d,
            3 => &mut self.e,
            4 => &mut self.h,
            5 => &mut self.l,
            6 => panic!("Can't give a reference to (HL)"),
            7 => &mut self.a,
            _ => unreachable!(),
        }
    }

    fn opcode_to_register_check(&mut self, opcode: u16) -> bool {
        if opcode & 0x07 == 0x06{
            false
        } else{
            true
        }
    }

    fn rotate_right(reg: &mut u8, val: &mut u8, res: &mut u8) {
        *val = *reg;
        *res = *val >> 1;
        *reg = *res;
    }

    fn rotate_hl(&mut self, left: bool, carry: u8, circular: bool, mmu: &mut MMU) -> (u8, u8) {
        let hl = self.HL();
        let val = mmu.rb(hl);

        let carry_out = if left { val >> 7 } else { val & 0x01 };
        let carry_in = if circular {
            carry_out
        } else {
            carry
        };

        let res = if left {
            (val << 1) | carry_in
        } else {
            (val >> 1) | (carry_in << 7)
        };

        mmu.wb(hl, res);

        (res, carry_out)
    }

    fn RLC(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let res;
        let msb;
        let cycles;

        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            msb = val >> 7;
            res = (val << 1) | msb;
            *reg = res;
            cycles = 8;
        } else {
            let (result, carry_out) = self.rotate_hl(true, 0, true, mmu);
            res = result;
            msb = carry_out;
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(msb == 1);

        cycles
    }

    fn RRC(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let res;
        let lsb;
        let cycles;

        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            lsb = val & 0x01;
            res = (val >> 1) | (lsb << 7);
            *reg = res;
            cycles = 8;
        } else {
            let (result, carry_out) = self.rotate_hl(false, 0, true, mmu);
            res = result;
            lsb = carry_out;
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(lsb == 1);

        cycles
    }

    fn RL(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let c = self.get_C() as u8;
        let res;
        let msb;
        let cycles;

        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            msb = val >> 7;
            res = (val << 1) | c;
            *reg = res;
            cycles = 8;
        } else {
            let (result, carry_out) = self.rotate_hl(true, c, false, mmu);
            res = result;
            msb = carry_out;
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(msb == 1);

        cycles
    }

    fn RR(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let c = self.get_C() as u8;
        let res;
        let lsb;
        let cycles;

        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            lsb = val & 0x01;
            res = (val >> 1) | (c << 7);
            *reg = res;
            cycles = 8;
        } else {
            let (result, carry_out) = self.rotate_hl(false, c, false, mmu);
            res = result;
            lsb = carry_out;
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(lsb == 1);

        cycles
    }

    fn SLA(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let cycles;
        let res;
        let MSB;
        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            MSB = val >> 7;
            res = val << 1;
            *reg = res;
            cycles = 8;
        } else{
            let val = mmu.rb(self.HL());
            MSB = val >> 7;
            res = val << 1;
            let hl = self.HL();
            mmu.wb(hl, res);
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(MSB == 1);

        cycles
    }

    fn SRA(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let cycles;
        let res;
        let LSB;

        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            LSB = val & 0x01;
            res = (val >> 1) | (val & 0x80);
            *reg = res;
            cycles = 8;
        } else {
            let val = mmu.rb(self.HL());
            LSB = val & 0x01;
            res = (val >> 1) | (val & 0x80);
            let hl = self.HL();
            mmu.wb(hl, res);
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(LSB == 1);

        cycles
    }

    fn SWAP(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let cycles;
        let res;
        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let mut val = *reg;
            res = ((val >> 4) | (val << 4)) & 0xFF;
            *reg = res;
            cycles = 8;
        } else {
            let v = mmu.rb(self.HL());
            let hl = self.HL();
            res = ((v >> 4) | (v << 4)) & 0xFF;
            mmu.wb(hl, res);
            cycles = 16;
        }
        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(false);

        cycles
    }

    fn SRL(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let cycles;
        let res;
        let LSB;

        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            LSB = val & 0x01;
            res = val >> 1;
            *reg = res;
            cycles = 8;
        } else {
            let val = mmu.rb(self.HL());
            LSB = val & 0x01;
            res = val >> 1;
            let hl = self.HL();
            mmu.wb(hl, res);
            cycles = 16;
        }

        self.set_Z(res == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(LSB == 1);

        cycles
    }

    fn BIT(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let cycles;
        let bit = ((opcode & 0xFF) >> 3) & 0x07;
        let set;
        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            let val = *reg;
            set = (val & (1 << bit)) != 0;
            cycles = 8
        } else {
            let val = mmu.rb(self.HL());
            set = (val & (1 << bit)) != 0;
            cycles = 7
        }

        self.set_Z(!set);
        self.set_N(false);
        self.set_H(true);

        cycles
    }

    fn RES(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let bit = ((opcode & 0xFF) >> 3) & 0x07;
        let cycles;
        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            *reg = *reg & !(1 << bit);
            cycles = 8
        } else {
            let val = mmu.rb(self.HL());
            let hl = self.HL();
            mmu.wb(hl, val & !(1 << bit));
            cycles = 16;
        }
        cycles
    }

    fn SET(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let bit = ((opcode & 0xFF) >> 3) & 0x07;
        let cycles;
        if self.opcode_to_register_check(opcode) {
            let reg = self.opcode_to_register(opcode);
            *reg |= 1 << bit;
            cycles = 8
        } else {
            let val = mmu.rb(self.HL());
            let hl = self.HL();
            mmu.wb(hl, val | (1 << bit));
            cycles = 16
        }

        cycles
    }

    fn RLA(&mut self, _opcode: u16) -> u8 {
        let MSB = self.a >> 7;
        self.a = self.a << 1 | self.get_C() as u8;

        self.set_Z(false);
        self.set_N(false);
        self.set_H(false);
        self.set_C(MSB == 1);

        4
    }

    fn JP(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let cycles;

        match opcode as u8 {
            0x18 => {
                let s8 = self.r8(mmu) as i16;
                self.pc = self.pc.wrapping_add(2).wrapping_add_signed(s8);
                self.last_instruction_length = 0;
                cycles = 12
            },
            0x28 => {
                if self.get_Z(){
                    self.pc = self.pc.wrapping_add(2).wrapping_add_signed(self.r8(mmu) as i16);
                    self.last_instruction_length = 0;
                    cycles = 12
                } else {
                    cycles = 8
                }
            },
            0x38 => {
                if self.get_C(){
                    self.pc = self.pc.wrapping_add(2).wrapping_add_signed(self.r8(mmu) as i16);
                    self.last_instruction_length = 0;
                    cycles = 12
                } else {
                    cycles = 8
                }
            },
            0x20 => {
                if !self.get_Z(){
                    self.pc = self.pc.wrapping_add(2).wrapping_add_signed(self.r8(mmu) as i16);
                    self.last_instruction_length = 0;
                    cycles = 12
                } else {
                    cycles = 8
                }
            },
            0x30 => {
                if !self.get_C(){
                    self.pc = self.pc.wrapping_add(2).wrapping_add_signed(self.r8(mmu) as i16);
                    self.last_instruction_length = 0;
                    cycles = 12
                } else {
                    cycles = 8
                }
            },
            0xC3 => {
                self.pc = self.d16(mmu);
                self.last_instruction_length = 0;
                cycles = 16
            },
            0xC2 => {
                if self.get_Z(){
                    cycles = 12
                } else {
                    self.pc = self.d16(mmu);
                    self.last_instruction_length = 0;
                    cycles = 16
                }
            },
            0xD2 => {
                if self.get_C(){
                    cycles = 12
                } else {
                    self.pc = self.d16(mmu);
                    self.last_instruction_length = 0;
                    cycles = 16
                }
            },
            0xE9 => {
                self.pc =  self.HL();
                self.last_instruction_length = 0;
                cycles = 4
            },
            0xCA => {
                if self.get_Z(){
                    self.pc = self.d16(mmu);
                    self.last_instruction_length = 0;
                    cycles = 16
                } else {
                    cycles = 12
                }
            },
            0xDA => {
                if self.get_C(){
                    self.pc = self.d16(mmu);
                    self.last_instruction_length = 0;
                    cycles = 16
                } else {
                    cycles = 12
                }
            }
            _ => unreachable!()
        }


        cycles
    }

    fn DAA(&mut self, _opcode: u16) -> u8 {
        let mut correction = 0;

        if !self.get_N() {
            if self.get_H() || (self.a & 0x0F) > 9 {
                correction |= 0x06;
            }

            if self.get_C() || self.a > 0x99 {
                correction |= 0x60;
                self.set_C(true);
            }

            self.a = self.a.wrapping_add(correction);
        } else {
            if self.get_H() {
                correction |= 0x06;
            }

            if self.get_C() {
                correction |= 0x60;
            }

            self.a = self.a.wrapping_sub(correction);
        }

        self.set_Z(self.a == 0);
        self.set_H(false);

        4
    }

    fn CPL(&mut self, _opcode: u16) -> u8 {
        self.a = !self.a;
        self.set_N(true);
        self.set_H(true);

        4
    }

    fn SCF(&mut self, _opcode: u16) -> u8 {
        self.set_C(true);
        self.set_N(false);
        self.set_H(false);

        4
    }

    fn CCF(&mut self, _opcode: u16) -> u8 {
        self.set_C(!self.get_C());
        self.set_N(false);
        self.set_H(false);

        4
    }

    fn ADD_flags(&mut self, a: u16, b: u16, carry: u16, res: u16, is_16bit: bool) {
        self.set_N(false);

        if is_16bit {
            self.set_H((a & 0x0FFF) + (b & 0x0FFF) + carry > 0x0FFF);
            self.set_C(a as u32 + b as u32 + carry as u32 > 0xFFFF);
        } else {
            self.set_Z(res as u8 == 0);
            self.set_H((a & 0x000F) + (b & 0x000F) + carry > 0x000F);
            self.set_C(a + b + carry > 0x00FF);
        }
    }

    fn read_hl(&self, mmu: &MMU) -> u8 {
        mmu.rb(self.HL())
    }

    fn ADD(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let cycles;
        match opcode as u8 {
            0x09 | 0x19 | 0x29 | 0x39 => {
                let b;
                match (opcode & 0xF0) >> 4 {
                    0x0 => b = self.BC(),
                    0x1 => b = self.DE(),
                    0x2 => b = self.HL(),
                    0x3 => b = self.sp,
                    _ => unreachable!()
                }
                let hl = self.HL();
                let res = hl.wrapping_add(b);
                self.set_HL(res);
                self.ADD_flags(hl, b, 0, res, true);
                cycles = 8;
            },
            0x80..=0x87 | 0x88..=0x8F => {
                let b;
                if self.opcode_to_register_check(opcode) {
                    b = *self.opcode_to_register(opcode);
                } else {
                    b = self.read_hl(mmu);
                }

                let carry = if opcode as u8 >= 0x88 {
                    self.get_C() as u8
                } else {
                    0
                };

                let a = self.a;
                let res = a.wrapping_add(b).wrapping_add(carry);
                self.ADD_flags(
                    a as u16,
                    b as u16,
                    carry as u16,
                    res as u16,
                    false
                );
                self.a = res;
                cycles = if opcode as u8 & 0x07 == 0x06 { 8 } else { 4 };
            },
            0xC6 | 0xCE => {
                let b = self.d8(mmu);
                let carry = if opcode as u8 == 0xCE {
                    self.get_C() as u8
                } else {
                    0
                };

                let a = self.a;
                let res = a.wrapping_add(b).wrapping_add(carry);
                self.ADD_flags(
                    a as u16,
                    b as u16,
                    carry as u16,
                    res as u16,
                    false
                );
                self.a = res;
                cycles = 8;
            },
            0xE8 => {
                let offset = self.d8(mmu);
                let sp = self.sp;
                self.sp = sp.wrapping_add_signed(offset as i8 as i16);
                self.set_Z(false);
                self.set_N(false);
                self.set_H((sp & 0x000F) + (offset as u16 & 0x000F) > 0x000F);
                self.set_C((sp & 0x00FF) + offset as u16 > 0x00FF);
                cycles = 16;
            },
            _ => unreachable!()
        }
        cycles
    }

    fn SUB_flags(&mut self, a: u16, b: u16, carry: u16, res: u16) {
        self.set_N(true);
        self.set_Z(res as u8 == 0);
        self.set_H((a & 0x000F) < (b & 0x000F) + carry);
        self.set_C(a < b + carry);
    }

    fn SUB(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let cycles;
        match opcode as u8 {
            0x90..=0x9F => {
                let b;
                if self.opcode_to_register_check(opcode) {
                    b = *self.opcode_to_register(opcode);
                } else {
                    b = self.read_hl(mmu);
                }

                let carry = if opcode as u8 >= 0x98 {
                    self.get_C() as u8
                } else {
                    0
                };

                let a = self.a;
                let res = a.wrapping_sub(b).wrapping_sub(carry);
                self.SUB_flags(
                    a as u16,
                    b as u16,
                    carry as u16,
                    res as u16
                );
                self.a = res;
                cycles = if opcode as u8 & 0x07 == 0x06 { 8 } else { 4 };
            },
            0xD6 | 0xDE => {
                let b = self.d8(mmu);
                let carry = if opcode as u8 == 0xDE {
                    self.get_C() as u8
                } else {
                    0
                };

                let a = self.a;
                let res = a.wrapping_sub(b).wrapping_sub(carry);
                self.SUB_flags(
                    a as u16,
                    b as u16,
                    carry as u16,
                    res as u16
                );
                self.a = res;
                cycles = 8;
            },
            _ => unreachable!()
        }
        cycles
    }

    fn AND(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let (b, cycles) = match opcode as u8 {
            0xA0..=0xA7 if self.opcode_to_register_check(opcode) => {
                (*self.opcode_to_register(opcode), 4)
            }
            0xA0..=0xA7 => (self.read_hl(mmu), 8),
            0xE6 => (self.d8(mmu), 8),
            _ => unreachable!(),
        };

        self.a = self.a & b;
        self.set_Z(self.a == 0);
        self.set_N(false);
        self.set_H(true);
        self.set_C(false);

        cycles
    }

    fn XOR(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let (b, cycles) = match opcode as u8 {
            0xA8..=0xAF if self.opcode_to_register_check(opcode) => {
                (*self.opcode_to_register(opcode), 4)
            }
            0xA8..=0xAF => (self.read_hl(mmu), 8),
            0xEE => (self.d8(mmu), 8),
            _ => unreachable!(),
        };

        self.a = self.a ^ b;
        self.set_Z(self.a == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(false);

        cycles
    }

    fn OR(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let (b, cycles) = match opcode as u8 {
            0xB0..=0xB7 if self.opcode_to_register_check(opcode) => {
                (*self.opcode_to_register(opcode), 4)
            }
            0xB0..=0xB7 => (self.read_hl(mmu), 8),
            0xF6 => (self.d8(mmu), 8),
            _ => unreachable!(),
        };

        self.a = self.a | b;
        self.set_Z(self.a == 0);
        self.set_N(false);
        self.set_H(false);
        self.set_C(false);

        cycles
    }

    fn CP(&mut self, opcode: u16, mmu: &MMU) -> u8 {
        let b = match opcode as u8 {
            0xB8..=0xBF if self.opcode_to_register_check(opcode) => {
                *self.opcode_to_register(opcode)
            }
            0xB8..=0xBF => self.read_hl(mmu),
            0xFE => self.d8(mmu),
            _ => unreachable!(),
        };
        let cycles = if opcode as u8 == 0xFE || opcode as u8 & 0x07 == 0x06 {
            8
        } else {
            4
        };
        let res = self.a.wrapping_sub(b);
        self.set_Z(res == 0);
        self.set_N(true);
        self.set_H((self.a & 0x0F) < (b & 0x0F));
        self.set_C(self.a < b);

        cycles
    }

    // returns the value, not the cycles
    fn pop_stack(&mut self, mmu: &mut MMU) -> u16 {
        let value = mmu.rw(self.sp);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    fn push_stack(&mut self, value: u16, mmu: &mut MMU) {
        self.sp = self.sp.wrapping_sub(2);
        mmu.ww(self.sp, value);
    }

    fn RET(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        match opcode as u8 {
            0xC0 => {
                if !self.get_Z(){
                    self.last_instruction_length = 0;
                    self.pc = self.pop_stack(mmu);
                    return 20
                }
                8
            },
            0xD0 => {
                if !self.get_C(){
                    self.last_instruction_length = 0;
                    self.pc = self.pop_stack(mmu);
                    return 20
                }
                8
            },
            0xC8 => {
                if self.get_Z(){
                    self.last_instruction_length = 0;
                    self.pc = self.pop_stack(mmu);
                    return 20
                }
                8
            },
            0xD8 => {
                if self.get_C(){
                    self.last_instruction_length = 0;
                    self.pc = self.pop_stack(mmu);
                    return 20
                }
                8
            },
            0xC9 => {
                self.pc = self.pop_stack(mmu);
                self.last_instruction_length = 0;
                16
            },
            0xD9 => {
                self.pc = self.pop_stack(mmu);
                self.last_instruction_length = 0;
                self.ime = true;
                16
            },
            _ => unreachable!()
        }
    }

    fn POP(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let val = self.pop_stack(mmu);
        match opcode as u8 {
            0xC1 => self.set_BC(val),
            0xD1 => self.set_DE(val),
            0xE1 => self.set_HL(val),
            0xF1 => self.set_AF(val & 0xFFF0),
            _ => unreachable!()
        }

        12
    }

    fn PUSH(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        match opcode as u8 {
            0xC5 => self.push_stack(self.BC(), mmu),
            0xD5 => self.push_stack(self.DE(), mmu),
            0xE5 => self.push_stack(self.HL(), mmu),
            0xF5 => self.push_stack(self.AF(), mmu),
            _ => unreachable!()
        }

        12
    }

    fn CALL(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        match opcode as u8 {
            0xC4 => {
                if !self.get_Z(){
                    self.push_stack(self.pc.wrapping_add(self.last_instruction_length as u16), mmu);
                    self.last_instruction_length = 0;
                    self.pc = self.d16(mmu);
                    24
                } else {
                    12
                }
            },
            0xD4 => {
                if !self.get_C(){
                    self.push_stack(self.pc.wrapping_add(self.last_instruction_length as u16), mmu);
                    self.last_instruction_length = 0;
                    self.pc = self.d16(mmu);
                    24
                } else {
                    12
                }
            },
            0xCC => {
                if self.get_Z(){
                    self.push_stack(self.pc.wrapping_add(self.last_instruction_length as u16), mmu);
                    self.last_instruction_length = 0;
                    self.pc = self.d16(mmu);
                    24
                } else {
                    12
                }
            },
            0xCD => {
                self.push_stack(self.pc.wrapping_add(self.last_instruction_length as u16), mmu);
                self.last_instruction_length = 0;
                self.pc = self.d16(mmu);
                24
            },
            0xDC => {
                if self.get_C(){
                    self.push_stack(self.pc.wrapping_add(self.last_instruction_length as u16), mmu);
                    self.last_instruction_length = 0;
                    self.pc = self.d16(mmu);
                    24
                } else {
                    12
               }
           },
        _ => unreachable!()
       }
    }

    fn RST(&mut self, opcode: u16, mmu: &mut MMU) -> u8 {
        let a = ((opcode >> 3) & 0x07) * 0x08;
        self.push_stack(self.pc.wrapping_add(self.last_instruction_length as u16), mmu);
        self.last_instruction_length = 0;
        self.pc = a;

        16
    }
}

impl initializable for Cpu{
    fn init(&mut self){
        if !self.has_bios{
            self.a = 0x01;
            self.f = 0xB0;
            self.b = 0x00;
            self.c = 0x13;
            self.d = 0x00;
            self.e = 0xD8;
            self.h = 0x01;
            self.l = 0x4D;
            self.sp = 0xFFFE;
            self.pc = 0x0100;
            self.ime = false;
            self.halt = false;
            self.stopped = false;
            self.mode = if self.mode == HardwareMode::Dmg {HardwareMode::Dmg} else {HardwareMode::Cgb };
            self.speed = ClockSpeed::Normal;
            self.halt_bug = false;
            self.last_instruction_length = 0;
        } else {
            self.a = 0x11;
            self.f = 0x80;
            self.b = 0x00;
            self.c = 0x00;
            self.d = 0xFF;
            self.e = 0x56;
            self.h = 0x00;
            self.l = 0x0D;
            self.sp = 0xFFFE;
            self.pc = 0x0000;
            self.ime = false;
            self.halt = false;
            self.stopped = false;
            self.speed = ClockSpeed::Normal;
            self.mode = if self.mode == HardwareMode::Dmg {HardwareMode::Dmg} else {HardwareMode::Cgb };
            self.halt_bug = false;
            self.last_instruction_length = 0;
        }
    }
}
