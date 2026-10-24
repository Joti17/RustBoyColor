#![allow(unused_variables, non_snake_case, dead_code, unused)]
use crate::mbc::{Bios, ram_banks, rom_banks, MBC};

pub struct MBC5{
    rom: Vec<u8>,
    ram: Vec<u8>,
    bios: Option<Bios>,

    rom_bank: usize,
    ram_bank: usize,

    ram_enabled: bool,
    ram_updated: bool,

    has_battery: bool,

    rom_banks: usize,
    ram_banks: usize,
}

impl MBC5{
    pub fn new(rom: Vec<u8>) -> MBC5{
        let subtype = rom[0x147];
        let ram_banks = match subtype {
            0x1A | 0x1B | 0x1D | 0x1E => ram_banks(rom[0x149]) as usize,
            _ => 0
        };
        let has_battery = match subtype {
            0x1B | 0x1E => true,
            _ => false,
        };

        let ram_size = 0x2000 * ram_banks;
        let rom_banks = rom_banks(rom[0x148]);


        MBC5 {
            rom,
            ram: vec![0x00; ram_size],
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            ram_updated: false,
            has_battery,
            rom_banks,
            ram_banks,
            bios: None,
        }
    }

    pub fn with_bios(rom: Vec<u8>, bios: Vec<u8>) -> MBC5{
        let mut mbc = Self::new(rom);
        mbc.bios = Some(Bios::new(bios));
        mbc
    }
}

impl MBC for MBC5{
    fn read_rom(&self, a: u16) -> u8 {
        if let Some(value) = self.bios.as_ref().and_then(|bios| bios.read(a)) {
            return value;
        }
        let idx = if a < 0x4000{
            a as usize
        } else{
            self.rom_bank * 0x4000 | a as usize & 0x3FFF
        };
        *self.rom.get(idx).unwrap_or(&0)
    }

    fn read_ram(&self, a: u16) -> u8 {
        if !self.ram_enabled {
            return 0x00;
        }
        self.ram[self.ram_bank * 0x2000 | a as usize & 0x1FFF]
    }

    fn write_rom(&mut self, a: u16, v: u8) {
        match a {
            0x0000..=0x1FFF => self.ram_enabled = (v & 0x0F) == 0x0A,
            0x2000..=0x2FFF => self.rom_bank = ((self.rom_bank & 0x100) | (v as usize)) % self.rom_banks,
            0x3000..=0x3FFF => self.rom_bank = ((self.rom_bank & 0xFF)) | (((v as u16 & 0x01) << 8) as usize) % self.rom_banks,
            0x4000..=0x5FFF => self.ram_bank = ((v & 0x0F) as usize) % self.ram_banks,
            0x6000..=0x7FFF => { /* does nothing */},
            _ => panic!("Invalid write address {:#X}", a),
        }
    }

    fn write_ram(&mut self, a: u16, v: u8) {
        if !self.ram_enabled {
            return;
        }
        self.ram[self.ram_bank * 0x2000 | ((a as usize) & 0x1FFF)] = v;
        self.ram_updated = true;
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let res = self.ram_updated;
        self.ram_updated = false;
        res
    }

    fn is_battery_backed(&self) -> bool {
        self.has_battery
    }

    fn load_ram(&mut self, ramdata: &[u8]) {
        if ramdata.len() != self.ram.len() {
            panic!("Loaded RAM has incorrect length");
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