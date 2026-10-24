#![allow(unused_variables, non_snake_case, dead_code, unused)]
use crate::mbc::{Bios, MBC};

pub struct MBC0{
    rom: Vec<u8>,
    bios: Option<Bios>,
}

impl MBC0 {
    pub fn new(data: Vec<u8>) -> MBC0 {
        Self { rom: data, bios: None }
    }

    pub fn with_bios(rom: Vec<u8>, bios: Vec<u8>) -> MBC0{
        Self {
            rom,
            bios: Some(Bios::new(bios)),
        }
    }
}

impl MBC for MBC0 {
    fn read_rom(&self, addr: u16) -> u8 {
        self.bios
            .as_ref()
            .and_then(|bios| bios.read(addr))
            .unwrap_or_else(|| *self.rom.get(addr as usize).unwrap_or(&0xFF))
    }
    
    fn read_ram(&self, _addr: u16) -> u8 {
        0
    }
    
    fn write_rom(&mut self, addr: u16, data: u8) {
        if addr == 0xFF50 && data != 0 {
            self.disable_bios();
        }
    }
    
    fn write_ram(&mut self, _addr: u16, _data: u8) {
        ()
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        false
    }

    fn is_battery_backed(&self) -> bool {
        false
    }

    fn load_ram(&mut self, _ram: &[u8]) {
        ()
    }

    fn dump_ram(&self) -> Vec<u8> {
        Vec::new()
    }
    fn disable_bios(&mut self) {
        if let Some(bios) = &mut self.bios {
            bios.disable();
        }
    }
}