use std::collections::VecDeque;
use std::ops::{BitOr, BitOrAssign};
use bytemuck::{Pod, Zeroable};
use crate::input::get_bit;
use crate::mode::{Interrupt, PPUMode};

pub const DOTS_PER_FRAME: usize = 70224;
pub const SCAN_LINES: usize = 154;
pub const _FPS: f64 = 59.737;
pub const DOTS_PER_SCAN_LINE: usize = 456;

const DMG_SHADES: [u16; 4] = [0x7FFF, 0x56B5, 0x294A, 0x0000];


pub struct LcdcRegister {
    pub LCDC: u8,
}

impl LcdcRegister {
    fn new(val: u8) -> LcdcRegister {
        LcdcRegister {
            LCDC: val,
        }
    }

    fn is_enabled(&self) -> bool {
        ((self.LCDC >> 7) & 0x01) != 0
    }

    // 0: 0x9800-0x9BFF
    // 1: 0x9C00-0x9FFF
    fn window_tile_map(&self) -> u16 {
        if (self.LCDC >> 6) & 0x01 == 0 {
            0x9800
        } else {
            0x9C00
        }
    }

    fn window_enabled(&self) -> bool {
        (self.LCDC >> 5) & 0x01 != 0
    }

    // 0: 8x8
    // 1: 8x16
    fn bg_obj_size(&self) -> u8 {
        self.LCDC >> 2 & 0x01
    }

    fn obj_enabled(&self) -> u8 {
        self.LCDC >> 1 & 0x01
    }

    fn bg_enabled(&self) -> u8 {
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

#[derive(Copy, Clone)]
struct BgTile {
    idx: u8,
    attribute: u8,
}

impl BgTile {
    pub fn new(idx: u8, attribute: u8) -> BgTile {
        Self {
            idx,
            attribute,
        }
    }
}

#[derive(Copy, Clone)]
struct FifoPx {
    color: u8,
    attr: u8,
}

#[derive(Copy, Clone)]
struct ObjPx {
    color: u8,
    palette: u8,
    bg_priority: bool,
    oam_index: u8,
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

    pub cgb: bool,

    pub oam: [u8; 0xA0],
    pub mode: PPUMode,
    pub vram: [[u8; 0x2000]; 2],
    pub vram_bank: u8,

    pub mode_3_dots: u16,
    startup: bool,

    pub framebuffer: Vec<u16>,

    opri: u8,
    lx: u8,
    line_dot: u16,
    stat_line: bool,

    window_line: u8,
    window_drawn: bool,
    win_active: bool,
    wy_latch: bool,

    line_objs: Vec<Object>,
    obj_done: [bool; 10],
    obj_pending: Option<usize>,
    obj_cur: usize,
    obj_fetch_left: u8,
    obj_buf: [Option<ObjPx>; 160],

    bg_fifo: VecDeque<FifoPx>,
    fetch_dot: u8,
    fetch_x: u8,
    fetch_row: u8,
    cur_tile: BgTile,
    fetch_lo: u8,
    fetch_hi: u8,
    discard: u8,
    first_fetch: bool,

    bg_pal: [u8; 64],
    obj_pal: [u8; 64],
    bcps: u8,
    ocps: u8,
}

pub struct PPUReturn {
    pub Vblank: bool,
    pub hblank: bool,
    pub interrupts: u8,
}

impl PPUReturn {
    pub fn new(Vblank: bool, interrupts: u8) -> PPUReturn {
        Self {
            Vblank,
            hblank: false,
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
            cgb: false,
            oam: [0; 0xA0],
            mode: PPUMode::HBlank,
            vram: [[0; 0x2000]; 2],
            vram_bank: 0,
            mode_3_dots: 0,
            startup: false,
            framebuffer: vec![0; 160 * 144],
            opri: 0,
            lx: 0,
            line_dot: 0,
            stat_line: false,
            window_line: 0,
            window_drawn: false,
            win_active: false,
            wy_latch: false,
            line_objs: Vec::with_capacity(10),
            obj_done: [false; 10],
            obj_pending: None,
            obj_cur: 0,
            obj_fetch_left: 0,
            obj_buf: [None; 160],
            bg_fifo: VecDeque::with_capacity(16),
            fetch_dot: 0,
            fetch_x: 0,
            fetch_row: 0,
            cur_tile: BgTile::new(0, 0),
            fetch_lo: 0,
            fetch_hi: 0,
            discard: 0,
            first_fetch: false,
            bg_pal: [0; 64],
            obj_pal: [0; 64],
            bcps: 0,
            ocps: 0,
        }
    }

    pub fn read_lcdc(&self) -> u8 {
        self.lcdc.LCDC
    }

    pub fn write_lcdc(&mut self, val: u8) {
        let was_on = self.lcdc.LCDC & 0x80 != 0;
        let now_on = val & 0x80 != 0;
        if was_on != now_on {
            self.ly = 0;
            self.line_dot = 0;
            self.window_line = 0;
            self.window_drawn = false;
            self.win_active = false;
            self.wy_latch = false;
            self.mode = PPUMode::HBlank;
            self.line_objs.clear();
            self.bg_fifo.clear();
            self.obj_pending = None;
            self.obj_fetch_left = 0;
            self.stat_line = false;
            self.startup = now_on;
        }
        self.lcdc.LCDC = val;
    }

    pub fn get_stat(&self) -> u8 {
        let mut res = (self.stat & 0x78) | 0x80;
        if self.lcdc.LCDC & 0x80 != 0 {
            if self.lyc == self.ly {
                res |= 0x04;
            }
            res |= self.mode as u8;
        }
        res
    }

    pub fn write_stat(&mut self, val: u8) {
        self.stat = val & 0x78;
    }

    pub fn read_opri(&self) -> u8 {
        self.opri | 0xFE
    }

    pub fn write_opri(&mut self, val: u8) {
        self.opri = val & 1;
    }

    // CGB palette registers (FF68-FF6B)
    pub fn read_bcps(&self) -> u8 {
        self.bcps | 0x40
    }

    pub fn write_bcps(&mut self, val: u8) {
        self.bcps = val & 0xBF;
    }

    pub fn read_bcpd(&self) -> u8 {
        self.bg_pal[(self.bcps & 0x3F) as usize]
    }

    pub fn write_bcpd(&mut self, val: u8) {
        self.bg_pal[(self.bcps & 0x3F) as usize] = val;
        if self.bcps & 0x80 != 0 {
            self.bcps = 0x80 | ((self.bcps + 1) & 0x3F);
        }
    }

    pub fn read_ocps(&self) -> u8 {
        self.ocps | 0x40
    }

    pub fn write_ocps(&mut self, val: u8) {
        self.ocps = val & 0xBF;
    }

    pub fn read_ocpd(&self) -> u8 {
        self.obj_pal[(self.ocps & 0x3F) as usize]
    }

    pub fn write_ocpd(&mut self, val: u8) {
        self.obj_pal[(self.ocps & 0x3F) as usize] = val;
        if self.ocps & 0x80 != 0 {
            self.ocps = 0x80 | ((self.ocps + 1) & 0x3F);
        }
    }

    pub fn framebuffer(&self) -> &[u16] {
        &self.framebuffer
    }
    pub fn tick(&mut self, dots: u16) -> PPUReturn {
        let mut ret = PPUReturn::new(false, 0);
        if !self.lcdc.is_enabled() {
            return ret;
        }
        for _ in 0..dots {
            self.step(&mut ret);
        }
        ret
    }

    fn step(&mut self, ret: &mut PPUReturn) {
        match self.mode {
            PPUMode::OAM => {
                if self.line_dot % 2 == 0 {
                    self.scan_object(self.line_dot as usize / 2);
                }
            }
            PPUMode::Drawing => self.step_drawing(),
            _ => {}
        }
        self.line_dot += 1;

        match self.mode {
            PPUMode::OAM if self.line_dot == 80 => {
                self.order_objects();
                self.begin_drawing();
            }
            PPUMode::Drawing if self.lx == 160 => {
                self.mode_3_dots = self.line_dot - 80;
                self.mode = PPUMode::HBlank;
                ret.hblank = true;
            }
            PPUMode::HBlank if self.startup && self.line_dot == 80 => {
                self.startup = false;
                self.begin_drawing();
            }
            _ => {}
        }
        if self.line_dot == 456 {
            self.end_line(ret);
        }
        self.update_stat_line(ret);
    }

    fn end_line(&mut self, ret: &mut PPUReturn) {
        self.line_dot = 0;
        if self.window_drawn {
            self.window_line = self.window_line.wrapping_add(1);
        }
        self.window_drawn = false;
        self.ly += 1;
        if self.ly == 144 {
            self.mode = PPUMode::VBlank;
            ret.interrupts |= Interrupt::VBlank;
            ret.Vblank = true;
        } else if self.ly == 154 {
            self.ly = 0;
            self.window_line = 0;
            self.wy_latch = false;
            self.mode = PPUMode::OAM;
            self.line_objs.clear();
        } else if self.ly < 144 {
            self.mode = PPUMode::OAM;
            self.line_objs.clear();
        }
    }

    fn update_stat_line(&mut self, ret: &mut PPUReturn) {
        let line = (get_bit(self.stat, 6) && self.ly == self.lyc)
            || (get_bit(self.stat, 5) && self.mode == PPUMode::OAM)
            || (get_bit(self.stat, 4) && self.mode == PPUMode::VBlank)
            || (get_bit(self.stat, 3) && self.mode == PPUMode::HBlank && !self.startup);
        if line && !self.stat_line {
            ret.interrupts |= Interrupt::LCDStat;
        }
        self.stat_line = line;
    }

    fn scan_object(&mut self, i: usize) {
        if self.line_objs.len() >= 10 {
            return;
        }
        let y = self.oam[i * 4];
        let h: u16 = if get_bit(self.lcdc.LCDC, 2) { 16 } else { 8 };
        let ly = self.ly as u16 + 16;
        if ly >= y as u16 && ly < y as u16 + h {
            self.line_objs.push(Object {
                y,
                x: self.oam[i * 4 + 1],
                idx: self.oam[i * 4 + 2],
                attribute: self.oam[i * 4 + 3],
                oam_index: i as u8,
            });
        }
    }

    fn coord_priority(&self) -> bool {
        !self.cgb || get_bit(self.opri, 0)
    }

    fn order_objects(&mut self) {
        if self.coord_priority() {
            self.line_objs.sort_by(|a, b| a.x.cmp(&b.x).then(a.oam_index.cmp(&b.oam_index)));
        } else {
            self.line_objs.sort_by_key(|o| o.oam_index);
        }
    }

    fn begin_drawing(&mut self) {
        self.mode = PPUMode::Drawing;
        self.lx = 0;
        self.fetch_x = 0;
        self.fetch_dot = 0;
        self.first_fetch = true;
        self.bg_fifo.clear();
        self.win_active = false;
        self.discard = self.scx % 8;
        self.obj_buf = [None; 160];
        self.obj_done = [false; 10];
        self.obj_pending = None;
        self.obj_fetch_left = 0;
        if self.ly == self.wy && get_bit(self.lcdc.LCDC, 5) {
            self.wy_latch = true;
        }
    }

    fn step_drawing(&mut self) {
        if self.obj_fetch_left > 0 {
            self.obj_fetch_left -= 1;
            if self.obj_fetch_left == 0 {
                self.finish_obj_fetch();
            }
            return;
        }

        let mut stall = false;
        if !self.win_active
            && get_bit(self.lcdc.LCDC, 5)
            && self.wy_latch
            && (self.lx + 7 == self.wx || (self.lx == 0 && self.wx < 7))
        {
            self.win_active = true;
            self.window_drawn = true;
            self.bg_fifo.clear();
            self.fetch_dot = 0;
            self.fetch_x = 0;
            self.first_fetch = false;
            self.discard = if self.wx < 7 { 7 - self.wx } else { 0 };
        }

        if self.obj_pending.is_none() && get_bit(self.lcdc.LCDC, 1) {
            self.obj_pending = self.find_sprite();
        }
        if let Some(i) = self.obj_pending {
            if self.fetch_dot >= 6 {
                self.obj_pending = None;
                self.obj_done[i] = true;
                self.obj_cur = i;
                self.obj_fetch_left = 5;
                return;
            }
        }

        match self.fetch_dot {
            0 => self.cur_tile = self.fetch_tile(self.fetch_x),
            2 => self.fetch_lo = self.tile_byte(false),
            4 => self.fetch_hi = self.tile_byte(true),
            _ => {}
        }
        if self.fetch_dot < 6 {
            self.fetch_dot += 1;
            if self.first_fetch && self.fetch_dot == 6 {
                self.first_fetch = false;
                self.fetch_dot = 0;
            }
        } else if self.bg_fifo.is_empty() {
            self.push_row();
            self.fetch_x = self.fetch_x.wrapping_add(8);
            self.fetch_dot = 0;
        }

        if self.obj_pending.is_some() || stall {
            return;
        }

        if let Some(px) = self.bg_fifo.pop_front() {
            if self.discard > 0 {
                self.discard -= 1;
                return;
            }
            let obj = self.obj_buf[self.lx as usize];
            let color = self.mix(px, obj);
            self.framebuffer[self.ly as usize * 160 + self.lx as usize] = color;
            self.lx += 1;
        }
    }

    fn find_sprite(&self) -> Option<usize> {
        for (i, o) in self.line_objs.iter().enumerate() {
            if !self.obj_done[i] && o.x.saturating_sub(8) == self.lx {
                return Some(i);
            }
        }
        None
    }

    fn finish_obj_fetch(&mut self) {
        let o = self.line_objs[self.obj_cur];
        let h: u8 = if get_bit(self.lcdc.LCDC, 2) { 16 } else { 8 };
        let mut row = (self.ly as u16 + 16 - o.y as u16) as u8;
        if get_bit(o.attribute, 6) {
            row = h - 1 - row;
        }
        let mut tile = o.idx;
        if h == 16 {
            tile &= 0xFE;
            if row >= 8 {
                tile |= 1;
                row -= 8;
            }
        }
        let bank = if self.cgb && get_bit(o.attribute, 3) { 1 } else { 0 };
        let addr = tile as usize * 16 + row as usize * 2;
        let lo = self.vram[bank][addr];
        let hi = self.vram[bank][addr + 1];
        let palette = if self.cgb { o.attribute & 7 } else { (o.attribute >> 4) & 1 };
        let x_flip = get_bit(o.attribute, 5);
        let by_coord = self.coord_priority();

        for i in 0..8u8 {
            let sx = o.x as i16 - 8 + i as i16;
            if sx < 0 || sx >= 160 {
                continue;
            }
            let bit = if x_flip { i } else { 7 - i };
            let color = (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1);
            if color == 0 {
                continue;
            }
            let px = ObjPx {
                color,
                palette,
                bg_priority: get_bit(o.attribute, 7),
                oam_index: o.oam_index,
            };
            let slot = &mut self.obj_buf[sx as usize];
            let replace = match *slot {
                None => true,
                Some(existing) => !by_coord && px.oam_index < existing.oam_index,
            };
            if replace {
                *slot = Some(px);
            }
        }
    }

    // bit 3: BG tile map (0: 9800-9BFF, 1: 9C00-9FFF)
    // bit 6: window tile map (0: 9800-9BFF, 1: 9C00-9FFF)
    fn fetch_tile(&mut self, x: u8) -> BgTile {
        let (map_base, tx, y): (u16, u16, u16) = if self.win_active {
            (
                if get_bit(self.lcdc.LCDC, 6) { 0x9C00 } else { 0x9800 },
                x as u16,
                self.window_line as u16,
            )
        } else {
            (
                if get_bit(self.lcdc.LCDC, 3) { 0x9C00 } else { 0x9800 },
                x.wrapping_add(self.scx) as u16,
                self.ly.wrapping_add(self.scy) as u16,
            )
        };
        self.fetch_row = (y % 8) as u8;
        let offset = (map_base - 0x8000 + (y / 8) * 32 + tx / 8) as usize;
        let attr = if self.cgb { self.vram[1][offset] } else { 0 };
        BgTile::new(self.vram[0][offset], attr)
    }

    // 0: 0x8800-0x8BFF (0x9000 as base pointer, signed ids)
    // 1: 0x8000-0x8FFF
    fn bg_and_window_tile_data_area(&self) -> u16 {
        if self.lcdc.LCDC >> 4 & 0x01 == 0 {
            0x9000
        } else {
            0x8000
        }
    }

    fn get_tile_addr(&self, tile_id: u8) -> u16 {
        let base_pointer = self.bg_and_window_tile_data_area();
        if base_pointer == 0x9000 {
            (base_pointer as i16 + (tile_id as i8 as i16 * 16)) as u16
        } else {
            base_pointer + tile_id as u16 * 16
        }
    }

    fn tile_byte(&self, hi: bool) -> u8 {
        let attr = self.cur_tile.attribute;
        let bank = if get_bit(attr, 3) { 1 } else { 0 };
        let row = if get_bit(attr, 6) { 7 - self.fetch_row } else { self.fetch_row };
        let addr = (self.get_tile_addr(self.cur_tile.idx) - 0x8000) as usize;
        self.vram[bank][addr + row as usize * 2 + hi as usize]
    }

    fn push_row(&mut self) {
        let attr = self.cur_tile.attribute;
        let x_flip = get_bit(attr, 5);
        for i in 0..8u8 {
            let bit = if x_flip { i } else { 7 - i };
            let color = (((self.fetch_hi >> bit) & 1) << 1) | ((self.fetch_lo >> bit) & 1);
            self.bg_fifo.push_back(FifoPx { color, attr });
        }
    }
    fn cgb_color(pal: &[u8; 64], palette: u8, color: u8) -> u16 {
        let i = palette as usize * 8 + color as usize * 2;
        let w = pal[i] as u16 | ((pal[i + 1] as u16) << 8);
        let r = w & 0x1F;
        let g = (w >> 5) & 0x1F;
        let b = (w >> 10) & 0x1F;
        (r << 10) | (g << 5) | b
    }

    fn mix(&self, bg: FifoPx, obj: Option<ObjPx>) -> u16 {
        let lcdc = self.lcdc.LCDC;
        let obj = if get_bit(lcdc, 1) { obj } else { None };
        let bg_master = get_bit(lcdc, 0);

        if !self.cgb {
            let bg_id = if bg_master { bg.color } else { 0 };
            if let Some(o) = obj {
                if !(o.bg_priority && bg_id != 0) {
                    let pal = if o.palette == 0 { self.obp0 } else { self.obp1 };
                    return DMG_SHADES[((pal >> (o.color * 2)) & 3) as usize];
                }
            }
            return DMG_SHADES[((self.bgp >> (bg_id * 2)) & 3) as usize];
        }

        if let Some(o) = obj {
            if bg.color == 0 || !bg_master || !(get_bit(bg.attr, 7) || o.bg_priority) {
                return Self::cgb_color(&self.obj_pal, o.palette, o.color);
            }
        }
        Self::cgb_color(&self.bg_pal, bg.attr & 7, bg.color)
    }

    pub fn bg_tile_map_area(&self) -> u16 {
        if self.lcdc.LCDC >> 3 & 0x01 == 0 {
            0x9800
        } else {
            0x9C00
        }
    }

    pub fn lcd_enabled(&self) -> bool {
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

    pub fn dump_vram(&self) {
        use std::fs;

        for bank in 0..2 {
            let path = format!("vram_bank_{bank}.bin");
            fs::write(&path, &self.vram[bank][..0x1800]).unwrap();
        }
    }
}