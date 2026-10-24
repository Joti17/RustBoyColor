#![allow(unused_variables, non_snake_case, dead_code, unused)]
use crate::mbc::{Bios, ram_banks, rom_banks};
use crate::mbc::MBC;
use crate::mbc::mbc3::MBC3;

const RAM_BANK_SIZE: usize = 0x2000;
const ROM_BANK_SIZE: usize = 0x4000;

pub struct MBC1{

    ram: Vec<u8>,
    bios: Option<Bios>,

    ram_enabled: bool,
    ram_updated: bool,
    ram_bank: usize,
    ram_banks: usize,

    rom: Vec<u8>,

    rom_bank: usize,
    rom_banks: usize,

    banking_mode: u8,
    has_battery_backed: bool,
}

impl MBC1{
    pub fn new(data: Vec<u8>) -> MBC1{
        let mut has_battery_backed = false;
        let ram_banks = ram_banks(data[0x149]) as usize;
        match data[0x147]{
            0x02 => has_battery_backed = false,
            0x03 => has_battery_backed = true,
            _ => has_battery_backed = false,
        }

        let rom_banks = rom_banks(data[0x148]);

        Self {
            ram: vec![0; ram_banks * RAM_BANK_SIZE],

            ram_enabled: false,
            ram_updated: false,
            ram_bank: 0,
            ram_banks,

            rom: data,

            rom_bank: 1,
            rom_banks,

            banking_mode: 0,
            has_battery_backed,
            bios: None,
        }
    }

    pub fn with_bios(rom: Vec<u8>, bios: Vec<u8>) -> MBC1{
        let mut mbc = Self::new(rom);
        mbc.bios = Some(Bios::new(bios));
        mbc
    }
}

impl MBC for MBC1{
    fn read_rom(&self, addr: u16) -> u8 {
        if let Some(value) = self.bios.as_ref().and_then(|bios| bios.read(addr)) {
            return value;
        }
        let bank = match addr {
            0x0000..=0x3FFF => {
                if self.banking_mode == 0 {
                    0
                }
                else {
                    self.rom_bank & 0x60
                }
            }
            0x4000..=0x7FFF => self.rom_bank,
            _ => return 0xFF,
        };

        let i = bank * ROM_BANK_SIZE + (addr as usize & 0x3FFF);
        *self.rom.get(i).unwrap_or(&0xFF)
    }

    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        let ram_bank = if self.banking_mode == 1{
            self.ram_bank
        } else {
            0
        };

        self.ram[(ram_bank * RAM_BANK_SIZE) | ((addr as usize) & 0x1FFF)]
    }

    fn write_rom(&mut self, addr: u16, value: u8){
        match addr{
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0xF == 0x0A;
            }
            0x2000..=0x3FFF => {
                let low_bits = match (value as usize) & 0x1F{
                    0 => 1,
                    n => n,
                };
                self.rom_bank = ((self.rom_bank & 0x60) | low_bits) % self.rom_banks;
            }
            0x4000..=0x5FFF => {
                if self.rom_banks > 0x20 {
                    let upper_bits = (value as usize & 0x03) % (self.rom_banks >> 5);
                    self.rom_bank = self.rom_bank & 0x1F | (upper_bits << 5)
                }
                if self.ram_banks > 1 {
                    self.ram_bank = (value as usize) & 0x03;
                }
            }
            0x6000..=0x7FFF => {
                self.banking_mode = value & 0x01;
            }
            _ => panic!("Could not write to {:?} (MBC1)", addr),
        }
    }

    fn write_ram(&mut self, addr: u16, value: u8){
        if !self.ram_enabled {
            return;
        }
        let ram_bank = if self.banking_mode == 1{
            self.ram_bank
        } else {
            0
        };
        let address = (ram_bank * RAM_BANK_SIZE) | ((addr as usize) & 0x1FFF);
        if address < self.ram.len() {
            self.ram[address] = value;
            self.ram_updated = true;
        }
    }

    fn check_and_reset_ram_updated(&mut self) -> bool {
        let result = self.ram_updated;
        self.ram_updated = false;
        result
    }

    fn is_battery_backed(&self) -> bool {
        self.has_battery_backed
    }

    fn load_ram(&mut self, ramdata: &[u8]){
        if ramdata.len() != self.ram.len(){
            panic!("ramdata size does not match load RAM");
        }

        self.ram = ramdata.to_vec();
    }

    fn dump_ram(&self) -> Vec<u8>{
        self.ram.to_vec()
    }
    fn disable_bios(&mut self){
        if let Some(bios) = &mut self.bios {
            bios.disable();
        }
    }
}
