#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use crate::dma::{DMA, VramDMA};
use crate::initialize::Initializable;
use crate::mbc;
use crate::mbc::*;
use crate::mode::{ClockSpeed, DIVSpeed, HardwareMode, Interrupt, PPUMode, TAC_SPEED};
use crate::ppu::PPU;

const WRAM_OFFSET: usize = 0xC000;
const WRAM_SIZE: usize = 0x8000;
const GBC_BANK_SIZE: usize = 0x1000;
const GBC_BANK_COUNT: usize = 8;
const DMG_BANK_SIZE: usize = 0x2000;

const VRAM_BANK_SIZE: usize = 0x2000;

const HRAM_SIZE: usize = 0x7F;
const IO_KEY0: u8 = 0x4C;
const IO_SIZE: usize = 0x80;

const OAM_SIZE: usize = 0xA0;

const IF: u16 = 0xFF0F;

pub struct MMU {
    wram: [u8; WRAM_SIZE],

    wram_banks: usize,
    current_wram_bank: usize,

    hram: [u8; HRAM_SIZE],
    io: [u8; IO_SIZE],
    ie: u8,

    mode: HardwareMode,
    cartridge: Cartridge,
    mbc: Box<dyn MBC + 'static>,
    has_bios: bool,
    pub timer: Timer,
    pub ppu: PPU,
    pub cpu_external_stall: u8,
    pub dma: DMA,
    pub vram_dma: VramDMA,
    dot_remainder: u8,

}

pub struct Timer {
    pub is_double_speed: bool,
    internal_timer: u16,
    pub is_stopped: bool,
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    tima_reload_delay: Option<u8>,
    pub tima_reloaded: bool
}

impl Timer {
    pub fn new() -> Timer{
        let res = Self{
            is_double_speed: false,
            internal_timer: 0,
            is_stopped: false,
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            tima_reload_delay: None,
            tima_reloaded: false,
        };

        res
    }

    pub fn tick(&mut self, cycles: u16, interrupt_flags: &mut u8) {
        for _ in 0..cycles {
            if let Some(delay) = self.tima_reload_delay {
                if delay == 1 {
                    self.tima = self.tma;
                    self.tima_reloaded = true;
                    *interrupt_flags |= 1 << 2;
                    self.tima_reload_delay = None;
                } else {
                    self.tima_reload_delay = Some(delay - 1);
                }
            } else {
                self.tima_reloaded = false;
            }

            let old_counter = self.internal_timer;
            self.internal_timer = self.internal_timer.wrapping_add(1);

            self.div = (self.internal_timer >> 8) as u8;

            if self.timer_signal(old_counter) && !self.timer_signal(self.internal_timer) {
                self.increment_tima();
            }
        }
    }

    fn timer_signal(&self, counter: u16) -> bool {
        if self.tac & 0x04 == 0 {
            return false;
        }

        let bit = match self.tac & 0x03 {
            0 => 9,
            1 => 3,
            2 => 5,
            _ => 7,
        };
        counter & (1 << bit) != 0
    }

    fn increment_tima(&mut self) {
        if self.tima_reload_delay.is_some() {
            return;
        }

        let (value, overflow) = self.tima.overflowing_add(1);
        self.tima = value;
        if overflow {
            self.tima_reload_delay = Some(4);
        }
    }

    fn reset_div(&mut self) {
        let timer_was_high = self.timer_signal(self.internal_timer);
        self.internal_timer = 0;
        self.div = 0;
        if timer_was_high {
            self.increment_tima();
        }
    }

    fn write_tac(&mut self, data: u8) {
        let old_signal = self.timer_signal(self.internal_timer);
        self.tac = data & 0x07;
        if old_signal && !self.timer_signal(self.internal_timer) {
            self.increment_tima();
        }
    }
}

pub struct Cartridge {
    rom: Vec<u8>,
    pub title: String,
    old_license: String,
    new_license: String,
    cartridge_type: u8,
}
impl Cartridge {
    /// # Always requires a call to parse_header() afterward
    pub fn new(file_name: &str) -> Cartridge {
        let mut cart = Cartridge{
            title: String::new(),
            rom: Vec::new(),
            old_license: String::new(),
            new_license: String::new(),
            cartridge_type: 0,
        };

        cart.load_rom(file_name).unwrap();
        cart
    }

    fn load_rom(&mut self, file_name: &str) -> io::Result<()>{
        self.rom.clear();
        let file = BufReader::new(File::open(file_name)?);
        let mut reader = BufReader::new(file);

        reader.read_to_end(&mut self.rom)?;

        Ok(())
    }

    pub fn parse_header(&mut self, io: &mut [u8; 128], mode: &mut HardwareMode) {
        let compare_logo: Vec<u8> = vec![
            0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B,
            0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
            0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E,
            0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
            0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC,
            0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
        ];

        if self.rom[0x104..=0x133] == compare_logo {
            println!("Valid Nintendo logo")
        }
        else {
            println!("Invalid Nintendo logo")
        }

        self.title = String::from_utf8_lossy(&self.rom[0x134..0x142]).to_string();
        let cgb_flag: u8 = self.rom[0x143];

        *mode = match (cgb_flag & 0xC0, *mode) {
            (0x80 | 0xC0, HardwareMode::Cgb) => HardwareMode::Cgb,
            (0x80 | 0xC0, HardwareMode::Dmg) => HardwareMode::Dmg,
            (_, HardwareMode::Cgb) => HardwareMode::GbcCompatible,
            (_, m) => m,
        };

        if *mode == HardwareMode::GbcCompatible {
            io[IO_KEY0 as usize] = cgb_flag;
        }


        if cgb_flag & 0x80 != 0 {
            io[IO_KEY0 as usize] = cgb_flag;
        }

        self.new_license = String::from_utf8_lossy(&self.rom[0x144..0x145]).to_string();
        let _sgb_flag: u8 = self.rom[0x146]; // theoretically the SGB Flag in the future

        self.cartridge_type = self.rom[0x147];

        let mut checksum: u8 = 0;
        for addr in 0x134..=0x14C{
            checksum = checksum.wrapping_sub(self.rom[addr]).wrapping_sub(1);
        }

        if !self.rom[0x14D] == checksum {
            panic!("Invalid checksum");
        }
    }
}

#[cfg(target_os = "linux")]
fn random_array(input: &mut [u8]) {
    if let Ok(mut file) = File::open("/dev/urandom") {
        let _ = file.read_exact(input);
    } else {
        random(input);
    }
}

#[cfg(not(target_os = "linux"))]
fn random_array(input: &mut [u8]) {
    random(input);
}
pub(crate) fn random(input: &mut [u8]) {
    let mut seed: u32 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32;
    for byte in input.iter_mut() {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        *byte = (seed & 0xFF) as u8;
    }
}

impl MMU {
    pub fn new(is_gbc: bool, rom_file: &str, bios: &str) -> MMU {
        let has_bios;
        if bios == ""{
            has_bios = false;
        } else {
            has_bios = true;
        }
        let wram_bank_count = if is_gbc { 8 } else { 2 };
        let mut wram= [0x00; WRAM_SIZE];
        random_array(&mut wram);

        let mut hram = [0x00; HRAM_SIZE];
        random_array(&mut hram);

        let mut io = [0x00; IO_SIZE];
        random_array(&mut io);

        let mut mode = if is_gbc {HardwareMode::Cgb } else {HardwareMode::Dmg};
        let mut cartridge = Cartridge::new(rom_file);
        cartridge.parse_header(&mut io, &mut mode);
        // if bios != ""
        let mbc: Box<dyn MBC> = if has_bios {
            let bios = std::fs::read(bios).unwrap();
            match cartridge.cartridge_type{
                0 => Box::new(mbc0::MBC0::with_bios(cartridge.rom.clone(), bios)),
                0x01..=0x03 => Box::new(mbc1::MBC1::with_bios(cartridge.rom.clone(), bios)),
                0x05..=0x06 => Box::new(mbc2::MBC2::with_bios(cartridge.rom.clone(), bios)),
                0x08..=0x09 => Box::new(mbc0::MBC0::with_bios(cartridge.rom.clone(), bios)),
                0x0F..=0x13 => Box::new(mbc3::MBC3::with_bios(cartridge.rom.clone(), bios)),
                0x19..=0x1E => Box::new(mbc5::MBC5::with_bios(cartridge.rom.clone(), bios)),
                _ => panic!("Unsupported MBC type {:?}!", cartridge.cartridge_type),
            }
        } else {
            match cartridge.cartridge_type{
                0 => Box::new(mbc0::MBC0::new(cartridge.rom.clone())),
                0x01..=0x03 => Box::new(mbc1::MBC1::new(cartridge.rom.clone())),
                0x05..=0x06 => Box::new(mbc2::MBC2::new(cartridge.rom.clone())),
                0x08..=0x09 => Box::new(mbc0::MBC0::new(cartridge.rom.clone())),
                0x0F..=0x13 => Box::new(mbc3::MBC3::new(cartridge.rom.clone())),
                0x19..=0x1E => Box::new(mbc5::MBC5::new(cartridge.rom.clone())),
                _ => panic!("Unsupported MBC type {:?}!", cartridge.cartridge_type),
            }
        };

        Self {
            wram,
            wram_banks: wram_bank_count,

            current_wram_bank: 0,
            mode,
            cartridge,
            hram,
            io,
            mbc,
            has_bios,
            timer: Timer::new(),
            ppu: PPU::new(),
            ie: 0x00,
            cpu_external_stall: 0,
            dma: DMA::new(),
            vram_dma: VramDMA::new(),
            dot_remainder: 0,
        }

        // n.load_rom(rom_file, &mut n.wram).unwrap();
    }

    pub fn tick_timer(&mut self, cycles: u16) {
        self.timer.tick(cycles, &mut self.io[0x0F]);
    }

    pub fn rb(&self, addr: u16) -> u8 {
        if self.dma.is_active()
            && !(0xFF80..=0xFFFE).contains(&addr)
            && addr != 0xFF46
        {
            return 0xFF;
        }

        self.rb_unrestricted(addr)
    }

    fn rb_unrestricted(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return self.mbc.read_rom(addr);
        }
        if (0x8000..=0x9FFF).contains(&addr) {
            return if self.ppu.mode != PPUMode::Drawing {
                self.ppu.vram[self.ppu.vram_bank as usize][addr as usize - 0x8000]
            } else {
                0xFF
            }
        }
        if (0xA000..=0xBFFF).contains(&addr) {
            return self.mbc.read_ram(addr);
        }
        if (0xC000..=0xDFFF).contains(&addr) {
            let offset = addr as usize - 0xC000;
            if self.mode == HardwareMode::Cgb {
                let bank = if offset < 0x1000 { 0 } else { self.current_wram_bank.max(1) };
                let bank_offset = if offset < 0x1000 { offset } else { (offset - 0x1000) + bank * 0x1000 };
                return self.wram[bank_offset];
            }
            return self.wram[offset];
        }
        if (0xE000..=0xFDFF).contains(&addr) {
             return self.rb_unrestricted(addr - 0x2000);
        }
        if (0xFE00..=0xFE9F).contains(&addr) {
            return if self.ppu.mode != PPUMode::OAM && self.ppu.mode != PPUMode::Drawing {
                self.ppu.oam[(addr - 0xFE00) as usize]
            } else {
                0xFF
            }
        }
        if (0xFF00..=0xFF7F).contains(&addr) {
            let reg = (addr - 0xFF00) as u8;
            return match reg {
                0x04 => self.timer.div,
                0x05 => self.timer.tima,
                0x06 => self.timer.tma,
                0x07 => self.timer.tac | 0xF8,
                0x46 => self.io[reg as usize],
                0x40 => self.ppu.read_lcdc(),
                0x44 => self.ppu.ly,
                0x51..=0x55 if self.mode != HardwareMode::Dmg => {
                    self.vram_dma.read_register(reg)
                }
                0x51..=0x55 => 0xFF,
                0x4F => self.ppu.vram_bank | 0xFE,
                0x50 => {
                    if self.has_bios {
                        0xFF
                    } else {
                        self.io[reg as usize]
                    }
                }
                _ => {
                    self.io[reg as usize]
                }
            }
        }
        if (0xFF80..=0xFFFE).contains(&addr) {
            return self.hram[(addr - 0xFF80) as usize];
        }
        if addr == 0xFFFF {
            return self.ie | 0xE0;
        }

        0xFF
    }

    pub fn trigger_interrupt(&mut self, interrupt: Interrupt) {
        self.wb(IF, self.rb(IF) | interrupt as u8);
    }

    pub fn tick_ppu(&mut self, dots: u16) -> bool{
        self.ppu.tick(dots, self)
    }

    pub fn wb(&mut self, addr: u16, data: u8) {
        if self.dma.is_active()
            && !(0xFF80..=0xFFFE).contains(&addr)
            && addr != 0xFF46
        {
            return;
        }

        if addr < 0x8000 {
            self.mbc.write_rom(addr, data);
        } else if (0x8000..=0x9FFF).contains(&addr) {
            if self.ppu.mode != PPUMode::Drawing {
                self.ppu.vram[self.ppu.vram_bank as usize][addr as usize - 0x8000] = data;
            }
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.mbc.write_ram(addr, data);
        } else if (0xC000..=0xDFFF).contains(&addr) {
            let offset = addr as usize - 0xC000;
            if self.mode == HardwareMode::Cgb {
                let bank = if offset < 0x1000 { 0 } else { self.current_wram_bank.max(1) };
                let bank_offset = if offset < 0x1000 { offset } else { (offset - 0x1000) + bank * 0x1000 };
                self.wram[bank_offset] = data;
            } else {
                self.wram[offset] = data;
            }
        } else if (0xE000..=0xFDFF).contains(&addr) {
            self.wb(addr - 0x2000, data);
        } else if (0xFE00..=0xFE9F).contains(&addr) {
            if self.ppu.mode != PPUMode::OAM && self.ppu.mode != PPUMode::Drawing {
                self.ppu.oam[(addr - 0xFE00) as usize] = data;
            } // else it's ignored
        } else if (0xFF00..=0xFF7F).contains(&addr) {
            let reg = (addr - 0xFF00) as u8;
            match reg {
                0x02 => {
                    if data & 0x81 == 0x81 {
                        let c: char = self.rb(0xFF01) as char;
                        print!("{}", c);
                        io::stdout().flush().unwrap();
                        self.writeIO(reg, data & !0x80);
                    } else {
                        self.writeIO(reg, data);
                    }
                }
                0x04 => {
                    self.timer.reset_div();
                }
                0x05 => {
                    if self.timer.tima_reload_delay.is_none() {
                        self.timer.tima = data;
                    } else if self.timer.tima_reload_delay != Some(1) {
                        self.timer.tima_reload_delay = None;
                        self.timer.tima = data;
                    }
                }
                0x06 => {
                    self.timer.tma = data;

                    if self.timer.tima_reloaded {
                        self.timer.tima = data;
                    }
                }
                0x07 => {
                    self.timer.write_tac(data);
                }
                0x46 => {
                    self.writeIO(reg, data);
                    self.dma.start_transfer(data);

                }
                0x40 => {
                    let was_enabled = self.ppu.lcd_enabled();
                    self.ppu.write_lcdc(data);
                    if was_enabled && !self.ppu.lcd_enabled() {
                        self.vram_dma.on_lcd_disabled();
                    }
                    self.writeIO(reg, data);
                }
                0x51..=0x55 if self.mode != HardwareMode::Dmg => {
                    self.vram_dma
                        .write_register(
                            reg,
                            data,
                            self.ppu.lcd_enabled(),
                            self.ppu.mode == PPUMode::HBlank,
                        );
                }
                0x51..=0x55 => {}
                0x4F => self.ppu.vram_bank = data & 0x01,
                0x50 => {
                    if self.has_bios && data != 0 {
                        self.writeIO(reg, data);
                        self.mbc.disable_bios();
                    }
                }
                0x70 => {
                    if self.mode == HardwareMode::Cgb {
                        self.current_wram_bank = (data & 0x7) as usize;
                    }
                }
                _ => self.writeIO(reg, data),
            }
        } else if (0xFF80..=0xFFFE).contains(&addr) {
            self.hram[(addr - 0xFF80) as usize] = data;
        } else if addr == 0xFFFF {
            self.ie = data;
        }
    }

    pub fn tick(&mut self, is_double_speed: bool, cycles: u16){
        let dots = if is_double_speed {
            let total_cycles = cycles as u32 + self.dot_remainder as u32;
            self.dot_remainder = (total_cycles % 2) as u8;
            (total_cycles / 2) as u16
        } else {
            cycles
        };

        let mut vram_dma = std::mem::replace(&mut self.vram_dma, VramDMA::new());
        for _ in 0..dots {
            if self.mode != HardwareMode::Dmg && self.ppu.mode != PPUMode::Drawing {
                vram_dma.tick(1, |source, destination| {
                    let value = self.rb_unrestricted(source);
                    let bank = self.ppu.vram_bank as usize;
                    self.ppu.vram[bank][(destination - 0x8000) as usize] = value;
                });
            }
        }
        self.vram_dma = vram_dma;

        let mut dma = std::mem::replace(&mut self.dma, DMA::new());
        dma.tick(cycles, is_double_speed, |address, index| {
            let value = self.rb_unrestricted(address);
            self.ppu.oam[index] = value;
        });
        self.dma = dma;
    }

    pub fn is_cpu_stalled(&self) -> bool {
        self.vram_dma.is_cpu_blocked()
    }

    pub fn ww(&mut self, addr: u16, data: u16) {
        self.wb(addr,(data & 0xFF) as u8);
        self.wb(addr+1, (data >> 8) as u8);
    }

    pub fn rw(&mut self, addr: u16) -> u16 {
        (self.rb(addr) as u16) | ((self.rb(addr.wrapping_add(1)) as u16) << 8)
    }

    pub fn writeIO(&mut self, register: u8, data: u8){
        match register {

            _ => self.io[register as usize] = data,
        }
    }
}
impl Initializable for MMU{
    fn init(&mut self){
        self.ppu = PPU::new();
        self.dma = DMA::new();
        self.vram_dma = VramDMA::new();
        self.dot_remainder = 0;
        self.wb(0xFF00, if self.has_bios { 0xCF } else { 0xCF });
        self.wb(0xFF01, if self.has_bios { 0x00 } else { 0x00 });
        self.wb(0xFF02, if self.has_bios { 0x7E } else { 0x7F });
        self.wb(0xFF04, if self.has_bios { 0x00 } else { 0xAB });
        self.wb(0xFF05, 0x00);
        self.wb(0xFF06, 0x00);
        self.wb(0xFF07, 0xF8);
        self.wb(0xFF0F, 0xE1);
        self.wb(0xFF10, 0x80);
        self.wb(0xFF11, 0xBF);
        self.wb(0xFF12, 0xF3);
        self.wb(0xFF13, 0xFF);
        self.wb(0xFF14, 0xBF);
        self.wb(0xFF16, 0x3F);
        self.wb(0xFF17, 0x00);
        self.wb(0xFF18, 0xFF);
        self.wb(0xFF19, 0xBF);
        self.wb(0xFF1A, 0x7F);
        self.wb(0xFF1B, 0xFF);
        self.wb(0xFF1C, 0x9F);
        self.wb(0xFF1D, 0xFF);
        self.wb(0xFF1E, 0xBF);
        self.wb(0xFF20, 0xFF);
        self.wb(0xFF21, 0x00);
        self.wb(0xFF22, 0x00);
        self.wb(0xFF23, 0xBF);
        self.wb(0xFF24, 0x77);
        self.wb(0xFF25, 0xF3);
        self.wb(0xFF26, if self.has_bios { 0xF0 } else { 0xF1 });
        self.wb(0xFF40, 0x91);
        self.wb(0xFF41, if self.has_bios { 0x81 } else { 0x85 });
        self.wb(0xFF42, 0x00);
        self.wb(0xFF43, 0x00);
        self.wb(0xFF44, if self.has_bios { 0x91 } else { 0x00 });
        self.wb(0xFF45, 0x00);
        self.wb(0xFF46, 0xFF);
        self.wb(0xFF47, 0xFC);
        self.wb(0xFF48, 0x00);
        self.wb(0xFF49, 0x00);
        self.wb(0xFF4A, 0x00);
        self.wb(0xFF4B, 0x00);
        self.wb(0xFF4C, 0x00);
        self.wb(0xFF4D, 0x7E);
        self.wb(0xFF4F, 0xFE);
        self.wb(0xFF50, if self.has_bios { 0x00 } else { 0x01 });
        for i in 0..5{
            self.io[0x51+i] = 0xFF;
        }
        self.wb(0xFF56, 0x3E);
        self.wb(0xFF68, 0x00);
        self.wb(0xFF69, 0x00);
        self.wb(0xFF6A, 0x00);
        self.wb(0xFF6B, 0x00);
        self.wb(0xFF70, 0xF8);

        if !self.has_bios {
            self.wb(0xFF50, 0x01);
        }
        self.timer = Timer::new();
        self.ie = 0x00;
        self.cpu_external_stall = 0;
    }
}
