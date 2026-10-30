use std::ops::{BitOr, BitOrAssign};
use bytemuck::{Pod, Zeroable};
use crate::cpu::Cpu;
use crate::input::get_bit;
use crate::mmu::MMU;
use crate::mode::{Interrupt, PPUMode};

const DOTS_PER_FRAME: usize = 70224;
const SCAN_LINES: usize = 154;
const _FPS: f64 = 59.737;
const DOTS_PER_SCAN_LINE: usize = 456;


struct LcdcRegister {
    pub LCDC: u8,
}

impl LcdcRegister {
    fn new(val: u8) -> LcdcRegister {
        LcdcRegister {
            LCDC: val,
        }
    }


    // disabling allows full write access to vram, OAM, etc.
    fn is_enabled(&mut self) -> bool {
        ((self.LCDC >> 7) & 0x01) != 0
    }

    // 0: 0x9800-0x9BFF
    // 1: 0x9C00-0x9FFF
    fn window_tile_map(&mut self) -> u16 {
        if (self.LCDC >> 6) & 0x01 == 0{
            0x9800
        } else {
            0x9C00
        }
    }

    fn window_enabled(&mut self) -> bool {
        (self.LCDC >> 5) & 0x01 != 0
    }

    // 0: 0x8800-0x8BFF (0x9000 as base pointer)
    // 1: 0x8C00-0x8FFF
    fn bg_and_window_tile_data_area(&mut self) -> u16 {
        if self.LCDC >> 4 & 0x01 == 0{
            0x9000 // signed addressing
        } else {
            0x8000
        }
    }

    fn bg_tile_map_area(&mut self) -> u16 {
        if self.LCDC >> 3 & 0x01 == 0{
            0x9800
        } else {
            0x9C00
        }
    }

    // 0: 8x8
    // 1: 8x16
    fn bg_obj_size(&mut self) -> u8 {
        self.LCDC >> 2 & 0x01
    }

    fn obj_enabled(&mut self) -> u8 {
        self.LCDC >> 1 & 0x01
    }

    fn bg_enabled(&mut self) -> u8 {
        self.LCDC & 0x01
    }
}

impl Into<u8> for LcdcRegister {
    fn into(self) -> u8 {
        self.LCDC
    }
}

impl From<u8> for LcdcRegister {
    fn from(val: u8) -> LcdcRegister {
        LcdcRegister::new(val)
    }
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Object {
    y: u8,
    x: u8,
    idx: u8,
    attribute: u8,
    oam_index: u8,
}
impl Object {
    fn new() -> Object{
        Self {
            y: 0,
            x: 0,
            idx: 0,
            attribute: 0,
            oam_index: 0,
        }
    }
}

struct BgTile {
    idx: u8,
    attribute: u8,
}

impl BgTile {
    pub fn new(idx: u8, attribute: u8) -> BgTile {
        Self{
            idx,
            attribute,
        }
    }
}

pub struct PPU {
    pub lcdc: LcdcRegister,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,

    pub oam: [u8; 0xA0],
    pub mode: PPUMode,
    mode_clock: u16,
    pub vram: [[u8; 0x2000]; 2],
    pub vram_bank: u8,

    mode_3_dots: u16,
    startup: bool,


    framebuffer: [[u16; 160]; 144],

    objects: [Object; 40],
}

struct PPUReturn {
    Vblank: bool,
    interrupts: u8,
}

impl PPUReturn {
    pub fn new(Vblank: bool, interrupts: u8) -> PPUReturn {
        Self {
            Vblank,
            interrupts,
        }
    }
}

impl BitOrAssign<Interrupt> for u8 {
    fn bitor_assign(&mut self, rhs: Interrupt) {
        *self |= rhs as u8;
    }
}

impl BitOr<Interrupt> for u8 {
    type Output = u8;

    fn bitor(self, rhs: Interrupt) -> Self::Output {
        self | (rhs as u8)
    }
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            lcdc: 0.into(),
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            mode: PPUMode::HBlank,
            mode_clock: 0,
            mode_3_dots: 456 - 160,
            framebuffer: [[0; 160]; 144],
            vram: [[0; 0x2000]; 2],
            vram_bank: 0,
            oam: [0; 0xA0],
            startup: false,
            objects: [Object::new(); 40],
        }
    }

    pub fn read_lcdc(& self) -> u8 {
        self.lcdc.LCDC
    }

    pub fn write_lcdc(&mut self, val: u8) {
        let old_lcd_enable = self.lcdc.LCDC & 0x80;
        let new_lcd_enable = val & 0x80;
        if old_lcd_enable != new_lcd_enable {
            if new_lcd_enable != 0 {
                self.startup = true;
                self.ly = 0;
                self.mode = PPUMode::HBlank;
                self.mode_clock = 0;

            }
        }
        self.lcdc.LCDC = val;
    }

    /// returns true, if in HBlank switch
    pub fn tick(&mut self, mut dots: u16, mmu: &mut MMU) -> PPUReturn{
        if !self.lcdc.is_enabled() {
            return PPUReturn::new(false, 0);
        }
        let mut interrupts = 0u8;
        self.mode_clock += dots;
        if self.mode == PPUMode::HBlank {
            let time = self.get_HBlank_time();
            if self.mode_clock >= time{
                self.mode_clock -= time;
                self.mode = PPUMode::VBlank;
                interrupts |= Interrupt::VBlank;
                interrupts = self.check_stat_interrupt(true, false, mmu, interrupts);
            }
        }
        if self.mode == PPUMode::VBlank {
            const VBLANK_LENGTH: u16 = 456;
            if self.mode_clock >= (self.ly as u16 - 144) * VBLANK_LENGTH{
                self.mode_clock -= VBLANK_LENGTH;
                self.ly += 1;
                if self.ly >= 154{
                    self.ly = 0;
                    self.mode = PPUMode::OAM;
                    interrupts = self.check_stat_interrupt(true, true, mmu, interrupts);
                } else {
                    interrupts = self.check_stat_interrupt(false, true, mmu, interrupts);
                }
            }
        }


        PPUReturn::new(false, interrupts)
    }

    fn check_stat_interrupt(&mut self, mode_switch: bool, lyc_change: bool, mmu: &mut MMU, current_interrupts: u8) -> u8 {
        if get_bit(self.stat, 6) && lyc_change {
            if self.lyc == self.ly {
                return current_interrupts | Interrupt::VBlank;
            }
        }
        if mode_switch {
            if get_bit(self.stat, 5){
                if self.mode == PPUMode::OAM{
                    return current_interrupts | Interrupt::VBlank;
                }
            }
            if get_bit(self.stat, 4){
                if self.mode == PPUMode::VBlank{
                    return current_interrupts | Interrupt::VBlank;
                }
            }
            if get_bit(self.stat, 3){
                if self.mode == PPUMode::HBlank{
                    return current_interrupts | Interrupt::VBlank;
                }
            }
        }
        current_interrupts
    }

    fn oam_to_objects(&mut self, start_dot: u16, end_dot: u16) {
        let start_dot = start_dot as usize;
        let end_dot = end_dot as usize;
        for i in start_dot/2..end_dot/2 {
            self.objects[i].y = self.oam[i*4];
            self.objects[i].x = self.oam[i*4+1];
            self.objects[i].idx = self.oam[i*4+2];
            self.objects[i].attribute = self.oam[i*4+3];
            self.objects[i].oam_index = i as u8 * 4;
        }

    }

    fn get_stat(&mut self) -> u8 {
        if self.lcdc.is_enabled() {
            let mut res = self.stat & !0x07;
            if self.lyc == self.ly {
                res |= 0x4
            }
            res |= self.mode as u8;
            res | 0x80
        } else {
            (self.stat & !0x03) | 0x80 // bit 0 and 1 read as 0
        }
    }

    /// flips startup bool
    fn get_HBlank_time(&mut self) -> u16 {
        if self.startup{
            self.startup = false;
            80
        } else {
            456 - self.mode_3_dots - 80
        }
    }



    pub fn lcd_enabled(&mut self) -> bool {
        self.lcdc.is_enabled()
    }
    pub fn rgb555_to_xrgb8888(rgb555: u16) -> u32 {
        let r5 = (rgb555 >> 10) & 0x1F;
        let g5 = (rgb555 >> 5) & 0x1F;
        let b5 = rgb555 & 0x1F;

        let r8 = ((r5 << 3) | (r5 >> 2)) as u32;
        let g8 = ((g5 << 3) | (g5 >> 2)) as u32;
        let b8 = ((b5 << 3) | (b5 >> 2)) as u32;

        (r8 << 16) | (g8 << 8) | b8
    }

    fn get_tile_addr(&mut self, idx: u8) -> u16{
        0x0000 + idx as u16 * 16
    }

    fn decode_tile_row(&self, tile_index: usize, row: u8, bank: usize) -> [u8; 8] {
        if row > 7 {
            panic!("Invalid tile row");
        }
        let tile_offset = (tile_index * 16) + (row as usize * 2);
        let row_data = (self.vram[bank][tile_offset], self.vram[bank][tile_offset + 1]);
        let mut pixels = [0; 8];
        for i in 0..8 {
            let bit = 7 - i;
            pixels[i] = (((row_data.1 >> bit) & 0x01) << 1) | ((row_data.0 >> bit) & 0x01);
        }


        pixels
    }

    pub fn dump_vram(&self) {
        use std::fs;

        for bank in 0..1 {
            let path = format!("vram_bank_{bank}.bin");
            fs::write(&path, &self.vram[bank][..0x1800]).unwrap();
        }
    }
}