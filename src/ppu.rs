use crate::mode::PPUMode;

const DOTS_PER_FRAME: usize = 70224;
const SCAN_LINES: usize = 154;
const _FPS: f64 = 59.737;
const DOTS_PER_SCAN_LINE: usize = 456;


struct LcdcRegister {
    LCDC: u8,
}

impl LcdcRegister {
    fn new(val: u8) -> LcdcRegister {
        LcdcRegister {
            LCDC: val,
        }
    }


    // disabling allows full write access to vram, OAM, etc.
    fn is_enabled(&mut self) -> u8 {
        self.LCDC >> 7 & 0x01
    }

    // 0: 0x9800-0x9BFF
    // 1: 0x9C00-0x9FFF
    fn window_tile_map(&mut self) -> u8 {
        self.LCDC >> 6 & 0x01
    }

    fn window_enabled(&mut self) -> u8 {
        self.LCDC >> 5 & 0x01
    }

    // 0: 0x8800-0x8BFF
    // 1: 0x8C00-0x8FFF
    fn bg_and_window_tile_data_area(&mut self) -> u8 {
        self.LCDC >> 4 & 0x01
    }

    fn bg_tile_map_area(&mut self) -> u8 {
        self.LCDC >> 3 & 0x01
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

pub struct PPU {
    lcdc: LcdcRegister,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,

    pub oam: [u8; 0xA0],
    pub mode: PPUMode,
    mode_clock: u16,
    pub vram: [[u8; 0x2000]; 2],
    pub vram_bank: u8,


    framebuffer: [[u16; 160]; 144],
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
            framebuffer: [[0; 160]; 144],
            vram: [[0; 0x2000]; 2],
            vram_bank: 0,
            oam: [0; 0xA0],
        }
    }

    pub fn lcd_enabled(&self) -> bool {
        self.lcdc.LCDC & 0x80 != 0
    }

    pub fn ly(&self) -> u8 {
        self.ly
    }

    pub fn read_lcdc(&self) -> u8 {
        self.lcdc.LCDC
    }

    pub fn write_lcdc(&mut self, value: u8) {
        let was_enabled = self.lcd_enabled();
        self.lcdc.LCDC = value;
        let is_enabled = self.lcd_enabled();

        if was_enabled && !is_enabled {
            self.ly = 0;
            self.mode_clock = 0;
            self.mode = PPUMode::HBlank;
        } else if !was_enabled && is_enabled {
            self.ly = 0;
            self.mode_clock = 0;
            self.mode = PPUMode::OAM;
        }
    }

    pub fn tick(&mut self, dots: u16) -> bool {
        if !self.lcd_enabled() {
            return false;
        }

        let mut entered_hblank = false;
        for _ in 0..dots {
            self.mode_clock += 1;
            match self.mode {
                PPUMode::OAM if self.mode_clock == 80 => {
                    self.mode = PPUMode::VRAM;
                    self.mode_clock = 0;
                }
                PPUMode::VRAM if self.mode_clock == 172 => {
                    self.mode = PPUMode::HBlank;
                    self.mode_clock = 0;
                    entered_hblank = true;
                }
                PPUMode::HBlank if self.mode_clock == 204 => {
                    self.mode_clock = 0;
                    if self.ly == 143 {
                        self.ly = 144;
                        self.mode = PPUMode::VBlank;
                    } else {
                        self.ly += 1;
                        self.mode = PPUMode::OAM;
                    }
                }
                PPUMode::VBlank if self.mode_clock == 456 => {
                    self.mode_clock = 0;
                    if self.ly == 153 {
                        self.ly = 0;
                        self.mode = PPUMode::OAM;
                    } else {
                        self.ly += 1;
                    }
                }
                _ => {}
            }
        }

        entered_hblank
    }

    fn oam_to_screen_position(&mut self, oam_index: usize) -> (i16, i16) {
        let y = ((self.oam[oam_index] & 0xFF) as i16) - 16;
        let x = ((self.oam[oam_index] as u16 >> 8) & 0xFF) as i16 - 8;
        (x, y)
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

    /*
    fn tile_pixels_to_color(&self, tile_index: usize, mut pixels: [[u8; 8]; 8]) -> [[u16; 8]; 8]{

        pixels
    }
    */
}