#![allow(unused_variables, non_snake_case, dead_code, unused)]

use std::fs;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::Path;
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
    pub vram_dma: Option<VramDMA>,
    dot_remainder: u8,
    pub save_file: Option<String>

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

        if self.rom[0x14D] != checksum {
            println!("Warning: invalid header checksum");
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
    pub fn new(is_gbc: bool, rom_file: &str, bios: &str, save_file: Option<String>) -> MMU {
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
        let mut mbc: Box<dyn MBC> = if has_bios {
            let bios = fs::read(bios).unwrap();
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
        if mbc.is_battery_backed() {
            if let Some(save_file) = &save_file {
                if Path::new(save_file).exists() {
                    mbc.load_ram(&fs::read(save_file).unwrap());
                }
            }
        }

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
            ppu: {
                let mut ppu = PPU::new();
                ppu.cgb = mode == HardwareMode::Cgb;
                ppu
            },
            ie: 0x00,
            cpu_external_stall: 0,
            dma: DMA::new(),
            vram_dma: VramDMA::new().into(),
            dot_remainder: 0,
            save_file,
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

    #[inline(always)]
    /// only use for raw IO registers
    pub fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF00..=0xFF7F => {
                let reg = (addr - 0xFF00) as u8;
                match reg {
                    0x04 => self.timer.div,
                    0x05 => self.timer.tima,
                    0x06 => self.timer.tma,
                    0x07 => self.timer.tac | 0xF8,
                    0x0F => self.io[reg as usize] | 0xE0,
                    0x46 => self.io[reg as usize],
                    0x40 => self.ppu.read_lcdc(),
                    0x41 => self.ppu.get_stat(),
                    0x42 => self.ppu.scy,
                    0x43 => self.ppu.scx,
                    0x44 => self.ppu.ly,
                    0x45 => self.ppu.lyc,
                    0x47 => self.ppu.bgp,
                    0x48 => self.ppu.obp0,
                    0x49 => self.ppu.obp1,
                    0x4A => self.ppu.wy,
                    0x4B => self.ppu.wx,
                    0x51..=0x55 if self.mode != HardwareMode::Dmg => {
                        self.vram_dma.as_ref().unwrap().read_register(reg)
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
                    0x68 if self.mode != HardwareMode::Dmg => self.ppu.read_bcps(),
                    0x69 if self.mode != HardwareMode::Dmg => {
                        if self.ppu.mode != PPUMode::Drawing { self.ppu.read_bcpd() } else { 0xFF }
                    }
                    0x6A if self.mode != HardwareMode::Dmg => self.ppu.read_ocps(),
                    0x6B if self.mode != HardwareMode::Dmg => {
                        if self.ppu.mode != PPUMode::Drawing { self.ppu.read_ocpd() } else { 0xFF }
                    }
                    0x68..=0x6B => 0xFF,
                    0x6C if self.mode != HardwareMode::Dmg => self.ppu.read_opri(),
                    0x6C => 0xFF,
                    _ => {
                        self.io[reg as usize]
                    }
                }
            }
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie | 0xE0,
            _ => panic!("Invalid IO register address: 0x{:04X}", addr)
        }
    }

    pub fn write_io(&mut self, addr: u16, data: u8){
        if (0xFF00..=0xFF7F).contains(&addr) {
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
                        self.vram_dma.as_mut().unwrap().on_lcd_disabled();
                    }
                    self.writeIO(reg, data);
                }
                0x41 => self.ppu.write_stat(data),
                0x42 => self.ppu.scy = data,
                0x43 => self.ppu.scx = data,
                0x44 => {} // LY is read-only
                0x45 => self.ppu.lyc = data,
                0x47 => self.ppu.bgp = data,
                0x48 => self.ppu.obp0 = data,
                0x49 => self.ppu.obp1 = data,
                0x4A => self.ppu.wy = data,
                0x4B => self.ppu.wx = data,
                0x51..=0x55 if self.mode != HardwareMode::Dmg => {
                    self.vram_dma.as_mut().unwrap()
                        .write_register(
                            reg,
                            data,
                            self.ppu.lcd_enabled(),
                            self.ppu.mode == PPUMode::HBlank,
                        );
                }
                0x4F => self.ppu.vram_bank = data & 0x01,
                0x50 => {
                    if self.has_bios && data != 0 {
                        self.writeIO(reg, data);
                        self.mbc.disable_bios();
                    }
                }
                0x68 if self.mode != HardwareMode::Dmg => self.ppu.write_bcps(data),
                0x69 if self.mode != HardwareMode::Dmg => {
                    if self.ppu.mode != PPUMode::Drawing { self.ppu.write_bcpd(data); }
                }
                0x6A if self.mode != HardwareMode::Dmg => self.ppu.write_ocps(data),
                0x6B if self.mode != HardwareMode::Dmg => {
                    if self.ppu.mode != PPUMode::Drawing { self.ppu.write_ocpd(data); }
                }
                0x68..=0x6B => {}
                0x6C if self.mode != HardwareMode::Dmg => self.ppu.write_opri(data),
                0x6C => {}
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
        } else {
            println!("MMU: write to unmapped address: {:04X}", addr);
        }
    }

    fn rb_unrestricted(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.mbc.read_rom(addr),
            0x8000..=0x9FFF => {
                if self.ppu.mode != PPUMode::Drawing {
                    self.ppu.vram[self.ppu.vram_bank as usize][(addr - 0x8000) as usize]
                } else {
                    0xFF
                }
            }
            0xA000..=0xBFFF => self.mbc.read_ram(addr),
            0xC000..=0xDFFF => {
                let offset = addr as usize - 0xC000;
                if self.mode == HardwareMode::Cgb {
                    let bank = if offset < 0x1000 { 0 } else { self.current_wram_bank.max(1) };
                    let bank_offset = if offset < 0x1000 { offset } else { (offset - 0x1000) + bank * 0x1000 };
                    return self.wram[bank_offset];
                }
                self.wram[offset]
            }
            0xE000..=0xFDFF => self.rb_unrestricted(addr - 0x2000),
            0xFE00..=0xFE9F => {
                if self.ppu.mode != PPUMode::OAM && self.ppu.mode != PPUMode::Drawing {
                    self.ppu.oam[(addr - 0xFE00) as usize]
                } else {
                    0xFF
                }
            }
            0xFF00..=0xFFFF => {
                self.read_io(addr)
            }
            _ => 0xFF
        }
    }

    pub fn save(&mut self){
        if let Some(save_file) = &self.save_file {
            if self.mbc.is_battery_backed() {
                println!("Saving!");
                fs::write(save_file, self.mbc.dump_ram()).unwrap();
            }
        }
    }
    pub fn trigger_interrupt(&mut self, interrupt: Interrupt) {
        self.io[0x0F] |= interrupt as u8;
    }

    pub fn tick_ppu(&mut self, dots: u16) -> bool{
        let res = self.ppu.tick(dots);
        if res.interrupts != 0 {
            self.write_io(IF, self.read_io(IF) | res.interrupts);
        }
        res.hblank
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
        } else {
            self.write_io(addr, data);
        }
    }

    pub fn tick(&mut self, is_double_speed: bool, cycles: u16) -> u16{
        let dots = if is_double_speed {
            let total_cycles = cycles as u32 + self.dot_remainder as u32;
            self.dot_remainder = (total_cycles % 2) as u8;
            (total_cycles / 2) as u16
        } else {
            cycles
        };


        let mut vram_dma = self.vram_dma.take().unwrap();
        for _ in 0..dots {
            if self.mode != HardwareMode::Dmg && self.ppu.mode != PPUMode::Drawing {
                vram_dma.tick(1, |source, destination| {
                    let value = self.rb_unrestricted(source);
                    let bank = self.ppu.vram_bank as usize;
                    self.ppu.vram[bank][(destination - 0x8000) as usize] = value;
                });
            }
        }
        self.vram_dma = Some(vram_dma);

        let mut dma = std::mem::replace(&mut self.dma, DMA::new());
        dma.tick(cycles, is_double_speed, |address, index| {
            let value = self.rb_unrestricted(address);
            self.ppu.oam[index] = value;
        });
        self.dma = dma;
        dots
    }

    pub fn is_cpu_stalled(&self) -> bool {
        self.vram_dma.as_ref().unwrap().is_cpu_blocked()
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
        self.ppu.cgb = self.mode == HardwareMode::Cgb;
        self.dma = DMA::new();
        self.vram_dma = VramDMA::new().into();
        self.dot_remainder = 0;
        if self.has_bios {
            let key0 = self.io[IO_KEY0 as usize];
            self.io = [0; IO_SIZE];
            self.io[IO_KEY0 as usize] = key0;
            self.timer = Timer::new();
            self.ie = 0x00;
            self.cpu_external_stall = 0;
            return;
        }
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
        self.io[0x46] = 0xFF;
        self.wb(0xFF47, 0xFC);
        self.wb(0xFF48, 0x00);
        self.wb(0xFF49, 0x00);
        self.wb(0xFF4A, 0x00);
        self.wb(0xFF4B, 0x00);
        self.io[0x4C] = 0x00; // don't want the side effects to kick in
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
