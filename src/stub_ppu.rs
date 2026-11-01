use std::fs::File;
use bytemuck::{Pod, Zeroable};
use minifb::{Window, WindowOptions};
use crate::input::get_bit;



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

struct PPU {
    pub vram: [[u8; 0x2000]; 2],
    pub vram_bank: u8,
    pub oam: [u8; 0xA0],
    pub framebuffer: [[u16; 160]; 144],
    objects: [Object; 40],
    lcdc: u8,
    window_enabled: bool,
    obj_enabled: bool,
    scx: u8,
    scy: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    bcps: u8,
    ocps: u8,
    opri: u8,
    bg_palette_ram: [u8; 64],
    object_scanlines: Vec<u8>
}

pub enum Layer{
    Back,
    Normal,
    Top,
}

impl PPU{
    #[cfg(target_os = "linux")]
    fn random_array(input: &mut [u8]) {
        if let Ok(mut file) = File::open("/dev/urandom") {
            let _ = file.read_exact(input);
        } else {
            crate::mmu::random(input);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn random_array(input: &mut [u8]) {
        crate::mmu::random(input);
    }
    pub fn new(vram: [[u8; 0x2000]; 2], oam: [u8; 0xA0]) -> PPU {
        // let mut bg_palette_ram = [0; 64];
        // Self::random_array(&mut bg_palette_ram);
        let bg_palette_ram = [
        0xFF, 0x7F, // Color 0: White
        0xAD, 0x56, // Color 1: Light gray
        0x52, 0x29, // Color 2: Dark gray
        0x00, 0x00, // Color 3: Black

        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00,
        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00,
        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00,
        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00,
        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00,
        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00,
        0xFF, 0x7F, 0xAD, 0x56, 0x52, 0x29, 0x00, 0x00];
        Self {
            vram,
            vram_bank: 0x00,
            oam,
            framebuffer: [[0x7FFF; 160]; 144],
            objects: [Object::new(); 40],
            lcdc: 0x91,
            window_enabled: false,
            obj_enabled: false,
            scx: 0x00,
            scy: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xE4,
            obp0: 0xE4,
            obp1: 0x1B,
            wy: 0x00,
            wx: 0x07,
            bcps: 0x80,
            ocps: 0x80,
            opri: 0x00,
            bg_palette_ram,
            object_scanlines: vec![0x00; 144]
        }
    }

    pub fn render(&mut self) -> [[u16; 160]; 144]{
        if !get_bit(self.lcdc, 7){
            return [[0; 160]; 144];
            // ppu is off
        }
        // bit 6:
        /*
        Window Tile map area:
            - 0: 0x9800-9BFF
            - 1: 0x9C00-9FFF
         */

        self.window_enabled = get_bit(self.lcdc, 5);

        // bit 4:
        /*
        BG & Window Tile data area:
            - 0: 8800–97FF
            - 1: 8000–8FFF
         */



        // bit 2:
        /*
        OBJ Size:
            - 0: 8x8
            - 1: 8x16
         */

        self.obj_enabled = get_bit(self.lcdc, 1);

        // bit 0:
        /*
        • Set (1):      BG and window priorities operate normally via OAM and BG Map attributes.
        • Cleared (0):  BG and window lose master priority, forcing sprites (OBJ) always on top regardless of individual priority flags.
         */

        self.render_background();
        self.render_window();
        self.oam_to_objects();
        self.order_objects();
        self.render_objects();





        self.framebuffer
    }

    fn oam_to_objects(&mut self){
        for i in 0..40 {
            self.objects[i].y = self.oam[i*4];
            self.objects[i].x = self.oam[i*4+1];
            self.objects[i].idx = self.oam[i*4+2];
            self.objects[i].attribute = self.oam[i*4+3];
            self.objects[i].oam_index = i as u8 * 4;
        }
        for obj in self.objects {
            if obj.x == 0 && obj.y == 0 && obj.idx == 0 && obj.attribute == 0 {
                break;
            }

            println!(
                "Object:\n  X:         0x{:02x}\n  Y:         0x{:02x}\n  idx:       0x{:02x}\n  attribute: 0x{:08b}\n",
                obj.x, obj.y, obj.idx, obj.attribute
            );
        }
    }

    fn order_objects(&mut self) {
        if get_bit(self.opri, 0){
            self.objects.sort_by(|a, b| {
                a.x.cmp(&b.x)
                    .then_with(|| b.oam_index.cmp(&a.oam_index))
            });
        } else {
            self.objects.sort_by(|a, b| a.oam_index.cmp(&b.oam_index));
        }
    }

    fn render_objects(&mut self) {
        self.object_scanlines = vec![0; 144];
        for obj in self.objects {
            let y_cord = obj.y as i16 - 16;
            let x_cord = obj.x as i16 - 8;
            if y_cord < -7 || y_cord >= 144 || obj.x == 0 || obj.x >= 168 {
                // skip offscreen
                if y_cord > 0 && y_cord < 144 {
                    self.object_scanlines[y_cord as usize] += 1;
                }
                continue;
            }
            self.object_scanlines[y_cord as usize] += 1;
            if self.object_scanlines[y_cord as usize] > 10 {
                continue;
            }
            self.draw_8x8_sprite(&obj, obj.idx as u16 * 16, x_cord, y_cord);


        }
        self.ly = 144;
    }

    fn render_window(&mut self) {
        if (0..167).contains(&(self.wx as u32)){
            if (0..144).contains(&(self.wy as u32)){
                // wy: start scanline
                // wx-7: end scanline
                let map_base = if get_bit(self.lcdc, 6) { 0x1C00 } else { 0x1800 };
                let mut window_tiles = self.get_tile_vector(map_base);

                let wx_offset = self.wx as i32 - 7;
                let wy_offset = self.wy as usize;
                for tile_y in 0..18{
                    let screen_y = wy_offset + (tile_y * 8);
                    if screen_y >= 144 {break;}

                    let map_row = (tile_y & 31) * 32;

                    for tile_x in 0..20 {
                        let screen_x = wx_offset + (tile_x as i32 * 8);
                        let map_col = tile_x & 31;
                        let tile = &window_tiles[map_row + map_col as usize];

                        let vram_addr = if get_bit(self.lcdc, 4){
                            (tile.idx as u16) * 16
                        } else {
                            let signed_idx = tile.idx as i8 as i32;
                            (0x1000_i32 + signed_idx * 16) as u16
                        };

                        self.draw_8x8(tile, vram_addr, screen_x, screen_y);
                    }
                }


            } else {
                return
            }
        } else {
            return
        }
    }

    fn get_tile_vector(&mut self, map_base: usize) -> Vec<BgTile>{
        let mut tiles = Vec::with_capacity(0x400);
        for i in 0..0x400 {
            let idx = self.vram[0][map_base+i];
            let attr = self.vram[1][map_base+i];
            let window_tile = BgTile::new(idx, attr);
            tiles.push(window_tile);
        }
        tiles
    }

    fn draw_8x8_sprite(&mut self, obj: &Object, vram_addr: u16, screen_x: i16, screen_y: i16){
        let bank = get_bit(obj.attribute, 3) as usize;
        let flip_x = get_bit(obj.attribute, 5);
        let flip_y = get_bit(obj.attribute, 6);
        for y in 0..8 {
            for x in 0..8 {
                let source_x = if flip_x { 7 - x } else { x };
                let source_y = if flip_y { 7 - y } else { y };

                let lo = self.vram[bank][(vram_addr + source_y as u16 * 2) as usize];
                let hi = self.vram[bank][(vram_addr + source_y as u16 * 2 + 1) as usize];

                let bit = 7 - source_x;
                let color = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
                if color == 0{
                    continue;
                }

                let px = screen_x + x as i16;
                let py = screen_y + y as i16;

                if !(0..160).contains(&px) || !(0..144).contains(&py) {
                    continue;
                }
                if (0..160).contains(&px) && py < 144 {
                    let palette_addr = (obj.attribute as usize & 0x07) * 8 + color as usize * 2;
                    let rgb555 = u16::from_le_bytes([
                        self.bg_palette_ram[palette_addr],
                        self.bg_palette_ram[palette_addr + 1],
                    ]);

                    self.framebuffer[py as usize][px as usize] = rgb555;
                }

            }


        }
    }

    fn draw_8x8(&mut self, tile: &BgTile, vram_addr: u16, tile_x: i32, tile_y: usize) {
        let bank = get_bit(tile.attribute, 3) as usize;
        let flip_x = get_bit(tile.attribute, 5);
        let flip_y = get_bit(tile.attribute, 6);

        for y in 0..8 {
            for x in 0..8 {
                let source_x = if flip_x { 7 - x } else { x };
                let source_y = if flip_y { 7 - y } else { y };

                let lo = self.vram[bank][(vram_addr + source_y * 2) as usize];
                let hi = self.vram[bank][(vram_addr + source_y * 2 + 1) as usize];

                let bit = 7 - source_x;
                let color = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);

                let px = tile_x + x;
                let py = tile_y + y as usize;

                if (0..160).contains(&px) && py < 144 {
                    let palette_addr = (tile.attribute as usize & 0x07) * 8 + color as usize * 2;
                    let rgb555 = u16::from_le_bytes([
                        self.bg_palette_ram[palette_addr],
                        self.bg_palette_ram[palette_addr + 1],
                    ]);

                    self.framebuffer[py][px as usize] = rgb555;
                }
            }
        }
    }

    fn render_background(&mut self){
        // bit 3:
        /*
        BG & tile map area:
            - 0: 9800–9BFF
            - 1: 9C00–9FFF
         */

        self.framebuffer = [[0; 160]; 144];
        // bank 0 is for tile_idx
        // bank 1 is for attributes
        let map_base = if get_bit(self.lcdc, 3) { 0x1C00 } else { 0x1800 };

        let mut tiles = self.get_tile_vector(map_base);

        for tile_y in 0..18 {
            for tile_x in 0..20 {
                let tile = &tiles[tile_y * 32 + tile_x];
                let tile_x = tile_x * 8;
                let tile_y = tile_y * 8;
                let vram_addr = self.get_tile_addr(tile.idx);


                self.draw_8x8(tile, vram_addr, tile_x as i32, tile_y);
                /*
                               for y in 0..8 {
                    for x in 0..8 {
                        let lo = self.vram[get_bit(tile.attribute, 3) as usize][(vram_addr + y * 2) as usize];
                        let hi = self.vram[get_bit(tile.attribute, 3) as usize][(vram_addr + y * 2 + 1) as usize];

                        let bit = 7 - x;
                        let color = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);

                        let px = tile_x + x;
                        let py = tile_y + y as usize;

                        if px < 160 && py < 144 {
                            let palette_addr = (tile.attribute as usize & 0x07) * 8 + color as usize * 2;
                            let rgb555 = u16::from_le_bytes([
                                self.bg_palette_ram[palette_addr],
                                self.bg_palette_ram[palette_addr + 1],
                            ]);

                            self.framebuffer[py][px] = rgb555;
                        }
                    }
                }
                 */
            }
        }
    }

    fn get_tile_addr(&mut self, idx: u8) -> u16{
        0x0000 + idx as u16 * 16
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
}


#[cfg(test)]
mod tests {
    use std::fs;
    use minifb::{Window, WindowOptions};
    use crate::stub_ppu::PPU;

    #[test]
    fn it_works() {
        let vram = [fs::read("ppu_test_data/cgb_vram_bank0.bin").unwrap().try_into().unwrap(), fs::read("ppu_test_data/cgb_vram_bank1.bin").unwrap().try_into().unwrap()];
        let oam = fs::read("ppu_test_data/oam_10_plus_1.bin").unwrap().try_into().unwrap();
        let mut ppu = PPU::new(vram, oam);

        ppu.lcdc = 0x93;
        ppu.scx  = 0x00;
        ppu.scy  = 0x00;
        ppu.bgp  = 0xE4;
        ppu.obp0 = 0xE4;
        ppu.obp1 = 0x1B;


        const WIDTH: usize = 160;
        const HEIGHT: usize = 144;

        let mut window = Window::new(
            "OAM",
            WIDTH, HEIGHT,
            WindowOptions {
                scale: minifb::Scale::X4,
                ..WindowOptions::default()
            },
        ).unwrap();
        let mut fb = ppu.render();
        let mut buffer = vec![0u32; 160 * 144];

        for y in 0..144 {
            for x in 0..160 {
                buffer[y * 160 + x] = PPU::rgb555_to_xrgb8888(fb[y][x]);
            }
        }
        window.update_with_buffer(&buffer, WIDTH, HEIGHT);
        while true{
            window.update();
        }
    }

    /*
    #[test]
    fn just_render(){
        use minifb::{Key, Window, WindowOptions};

        const WIDTH: usize = 160;
        const HEIGHT: usize = 144;

        let mut window = Window::new(
            "OAM Sprites",
            WIDTH,
            HEIGHT,
            WindowOptions {
                scale: minifb::Scale::X4,
                ..WindowOptions::default()
            },
        ).unwrap();

        let mut buffer = vec![0xFFFFFFFFu32; WIDTH * HEIGHT];

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let color = match FRAMEBUFFER_OAM_SPRITES[y][x] {
                    0 => 0xFFFFFFFF,
                    1 => 0xFFAAAAAA,
                    2 => 0xFF555555,
                    _ => 0xFF000000,
                };

                buffer[y * WIDTH + x] = color;
            }
        }

        while window.is_open() && !window.is_key_down(Key::Escape) {
            window
                .update_with_buffer(&buffer, WIDTH, HEIGHT)
                .unwrap();
        }
    }
     */
}