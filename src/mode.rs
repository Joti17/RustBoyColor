#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::string::ToString;
use maplit::*;


#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HardwareMode {
    Dmg,
    Cgb,
    GbcCompatible,
}

#[repr(u8)]
pub enum Interrupt {
    VBlank = 1 << 0,
    LCDStat = 1 << 1,
    Timer = 1 << 2,
    Serial = 1 << 3,
    Joypad = 1 << 4,
}

#[repr(usize)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ClockSpeed{
    Normal = 4194304,
    Double = 8388608,
}

#[repr(usize)]
pub enum DIVSpeed {
    Zero   = 0,
    Normal = 16384,
    Double = 32768,
}

pub const TAC_SPEED: [[u32; 4]; 2] = [[4096, 262144, 65536, 16384],
                                  [8192, 524288, 131072, 32768]];

#[repr(usize)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PPUMode {
    HBlank  = 0,
    VBlank  = 1,
    OAM     = 2,
    Drawing = 3,
}