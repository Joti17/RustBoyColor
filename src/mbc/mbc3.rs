#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::io::Write;
use crate::mbc::{Bios, MBC, ram_banks};
use std::time;

pub struct MBC3{
    rom: Vec<u8>,
    ram: Vec<u8>,
    bios: Option<Bios>,

    rom_bank: u8,
    ram_bank: u8,

    ram_banks: usize,
    select_rtc: bool,

    ram_enable: bool,
    ram_updated: bool,

    has_battery: bool,

    rtc_ram: [u8; 5],
    rtc_ram_latch: [u8; 5],
    rtc_zero: Option<u64>,
}

impl MBC3 {
    pub fn new(rom: Vec<u8>) -> MBC3 {
        let subtype = rom[0x147];
        let has_battery = match subtype {
            0x0F | 0x10 | 0x13 => true,
            _ => false,
        };
        let rtc: Option<u64> = match subtype{
            0x0F | 0x10 => Some(0),
            _ => None,
        };
        let ram_banks = match subtype {
            0x10 | 0x12 | 0x13 => ram_banks(rom[0x149]) as usize,
            _ => 0,
        };
        let ram_size = ram_banks * 0x2000;
        MBC3{
            rom,
            ram: vec![0x00; ram_size],

            rom_bank: 1,
            ram_bank: 0,

            ram_banks,
            select_rtc: false,

            ram_enable: false,
            ram_updated: false,

            has_battery,
            rtc_ram: [0; 5],
            rtc_ram_latch: [0; 5],
            rtc_zero: rtc,
            bios: None,
        }
    }

    pub fn with_bios(rom: Vec<u8>, bios: Vec<u8>) -> MBC3{
        let mut mbc = Self::new(rom);
        mbc.bios = Some(Bios::new(bios));
        mbc
    }

    fn latch_rtc_reg(&mut self){
        self.calculate_rtc_reg();
        self.rtc_ram_latch.clone_from_slice(&self.rtc_ram);
    }

    fn calculate_rtc_reg(&mut self){
        if self.rtc_ram[4] & 0x40 == 0x40{
            return
        }

        let time_zero = match self.rtc_zero{
            Some(t) => time::UNIX_EPOCH + time::Duration::from_secs(t),
            _ => return,
        };

        if self.compute_dt() == self.rtc_zero {
            return
        }

        let dt = match time::SystemTime::now().duration_since(time_zero) {
            Ok(n) => n.as_secs(),
            _ => 0,
        };

        self.rtc_ram[0] = (dt % 60) as u8;
        self.rtc_ram[1] = ((dt / 60) % 60) as u8;
        self.rtc_ram[2] = ((dt / 3600) % 24) as u8;
        let days = dt / (3600 * 24);
        self.rtc_ram[3] = days as u8;
        self.rtc_ram[4] = (self.rtc_ram[4] & 0xFE) | (((days >> 8) & 0x01) as u8);
        if days >= 512 {
            self.rtc_ram[4] |= 0x80;
            self.calc_rtc_zero();
        }
    }

    fn compute_dt(&self) -> Option<u64>{
        if self.rtc_zero.is_none(){
            return None
        }

        let mut dt = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => {
                panic!("SystemTime before UNIX EPOCH!")
            }
        };

        dt -= self.rtc_ram[0] as u64;
        dt -= (self.rtc_ram[1] as u64) * 60;
        dt -= (self.rtc_ram[2] as u64) * 3600;
        let days = ((self.rtc_ram[4] as u64 & 0x1) << 8) | (self.rtc_ram[3] as u64);
        dt -= days * 3600 * 24;
        Some(dt)
    }

    fn calc_rtc_zero(&mut self) {
        self.rtc_zero = self.compute_dt();
    }
}

impl MBC for MBC3 {
    fn read_rom(&self, a: u16) -> u8 {
        if let Some(value) = self.bios.as_ref().and_then(|bios| bios.read(a)) {
            return value;
        }
        let idx = if a < 0x3FFF {a as usize}
        else {
            self.rom_bank as usize * 0x4000 | (a as usize & 0x3FFF)
        };

        *self.rom.get(idx).unwrap_or(&0xFF)
    }

    fn read_ram(&self, a: u16) -> u8 {
        if !self.ram_enable {
            return 0xFF
        }
        if !self.select_rtc && self.ram_bank < self.ram_banks as u8 {
            self.ram[self.ram_bank  as usize * 0x2000 | (a as usize & 0x1FFF)]
        } else if self.select_rtc && self.ram_bank < 5{
            self.rtc_ram_latch[self.ram_bank as usize]
        } else {
            0xFF
        }
    }

    fn write_rom(&mut self, a: u16, v: u8) {
        match a {
            0x0000..=0x1FFF => {
                self.ram_enable = (v & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // only 7 bits
                self.rom_bank = match v & 0x7F {
                    0 => 1,
                    n => n
                }
            }
            0x4000..=0x5FFF => {
                self.select_rtc = (0x08..=0x0C).contains(&v);
                self.ram_bank = v & 0x07;
            }
            0x6000..=0x7FFF => self.latch_rtc_reg(),
            _ => panic!("Unrecognized address access: {:04X}", a)
        }
    }

    fn write_ram(&mut self, a: u16, v: u8) {
        if !self.ram_enable {
            return;
        }

        if !self.select_rtc && self.ram_bank < self.ram_banks as u8 {
            self.ram[self.ram_bank as usize * 0x2000 | a as usize & 0x1FFF] = v;
            self.ram_updated = true;
        } else if self.select_rtc && self.ram_bank < 5 {
            self.calculate_rtc_reg();
            let mask = match self.ram_bank{
                0 | 1 => 0x3F,
                2 => 0x1F,
                4 => 0xC1,
                _ => 0xFF,
            };
            self.rtc_ram[self.ram_bank as usize] = v & mask;
            self.calc_rtc_zero();
            self.ram_updated = true;
        }
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
        if ramdata.len() != 8 + self.ram.len(){
            panic!("RAM is too small");
        }

        let (int_bytes, rest) = ramdata.split_at(8);
        let rtc = u64::from_be_bytes(int_bytes.try_into().unwrap());
        if self.rtc_zero.is_some() {
            self.rtc_zero = Some(rtc);
        }

        self.ram = rest.to_vec()
    }

    fn dump_ram(&self) -> Vec<u8> {
        let rtc = self.rtc_zero.unwrap_or_else(|| 0);

        let mut file = vec![];

        let mut ok = true;

        if ok {
            let rtc_bytes = rtc.to_be_bytes();
            ok = file.write_all(&rtc_bytes).is_ok();
        };
        if ok {
            let _ = file.write_all(&*self.ram);
        };

        file
    }
    fn disable_bios(&mut self){
        if let Some(bios) = &mut self.bios {
            bios.disable();
        }
    }
}