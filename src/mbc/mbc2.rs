#![allow(unused_variables, non_snake_case, dead_code, unused)]
use crate::mbc::{Bios, rom_banks, MBC};

pub struct MBC2 {
    ram: Vec<u8>,
    bios: Option<Bios>,

    ram_enabled: bool,
    ram_updated: bool,

    rom: Vec<u8>,

    rom_bank: usize,
    rom_banks: usize,

    has_battery_backed: bool,
}

impl MBC2 {
    pub fn new(rom: Vec<u8>) -> MBC2 {
        let has_battery_backed = match rom[0x147] {
            0x06 => true,
            _ => false,
        };
        let rom_banks = rom_banks(rom[0x148]);

        MBC2{
            rom,
            ram: vec![0; 512],
            ram_enabled: false,
            ram_updated: false,
            rom_bank: 1,
            has_battery_backed,
            rom_banks,
            bios: None,
        }
    }

    pub fn with_bios(rom: Vec<u8>, bios: Vec<u8>) -> MBC2{
        let mut mbc = Self::new(rom);
        mbc.bios = Some(Bios::new(bios));
        mbc
    }
}

impl MBC for MBC2 {
    fn read_rom(&self, addr: u16) -> u8 {
        if let Some(value) = self.bios.as_ref().and_then(|bios| bios.read(addr)) {
            return value;
        }
        let rom_bank = if addr < 0x4000 { 0 } else { self.rom_bank };
        let idx = rom_bank * 0x4000 | addr as usize & 0x3FFF;
        *self.rom.get(idx).unwrap_or(&0xFF)
    }

    // 0xA000-0xA1FF
    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        self.ram[addr as usize & 0x1FF] | 0xF0
    }

    fn write_rom(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x3FFF => {
                if addr & 0x100 == 0{
                    if data & 0x0F == 0xA{
                        self.ram_enabled = true;
                    }
                }
                else {
                    self.rom_bank = match data & 0x0F {
                        0 => 1,
                        n => n as usize,
                    } % self.rom_banks;
                }
            }
            _ => {}
        }
    }

    fn write_ram(&mut self, addr: u16, data: u8) {
        if !self.ram_enabled {
            return
        }

        self.ram_updated = true;
        self.ram[addr as usize & 0x1FF] = data & 0xF0;
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let res = self.ram_updated;
        self.ram_updated = false;
        res
    }

    fn is_battery_backed(&self) -> bool {
        self.has_battery_backed
    }

    fn load_ram(&mut self, ramdata: &[u8]) {
        if ramdata.len() != self.ram.len(){
            panic!("ramdata length does not match load ram");
        }
        self.ram = ramdata.to_vec();
    }

    fn dump_ram(&self) -> Vec<u8> {
        self.ram.to_vec()
    }
    fn disable_bios(&mut self){
        if let Some(bios) = &mut self.bios {
            bios.disable();
        }
    }
}