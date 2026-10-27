from pathlib import Path

WIDTH = 160
HEIGHT = 144
VRAM_SIZE = 0x2000

def render(vram_path, oam_path, registers=None):
    vram = Path(vram_path).read_bytes()
    oam = Path(oam_path).read_bytes()

    if len(vram) != VRAM_SIZE:
        raise ValueError(f"Expected {VRAM_SIZE} VRAM bytes, got {len(vram)}")
    if len(oam) != 160:
        raise ValueError(f"Expected 160 OAM bytes, got {len(oam)}")

    registers = registers or {}
    lcdc = registers.get("LCDC", 0x93)
    scy = registers.get("SCY", 0)
    scx = registers.get("SCX", 0)
    wy = registers.get("WY", 0)
    wx = registers.get("WX", 0)
    bgp = registers.get("BGP", 0xE4)
    obp0 = registers.get("OBP0", 0xE4)
    obp1 = registers.get("OBP1", 0x1B)

    framebuffer = [[0] * WIDTH for _ in range(HEIGHT)]

    def palette(color, value):
        return (value >> (color * 2)) & 3

    def tile_pixel(tile, x, y, signed=False):
        if signed and tile < 128:
            tile_address = 0x1000 + tile * 16
        elif signed:
            tile_address = 0x1000 + (tile - 256) * 16
        else:
            tile_address = tile * 16

        address = tile_address + y * 2
        low = vram[address]
        high = vram[address + 1]
        bit = 7 - x

        return ((high >> bit) & 1) << 1 | ((low >> bit) & 1)

    # Background and window
    if lcdc & 0x80:
        for y in range(HEIGHT):
            for x in range(WIDTH):
                use_window = (
                    lcdc & 0x20
                    and y >= wy
                    and x >= wx - 7
                )

                if use_window:
                    pixel_x = x - (wx - 7)
                    pixel_y = y - wy
                    map_base = 0x1C00 if lcdc & 0x40 else 0x1800
                else:
                    pixel_x = (x + scx) & 0xFF
                    pixel_y = (y + scy) & 0xFF
                    map_base = 0x1C00 if lcdc & 0x08 else 0x1800

                map_x = (pixel_x // 8) & 31
                map_y = (pixel_y // 8) & 31
                tile = vram[map_base + map_y * 32 + map_x]

                signed = not (lcdc & 0x10)
                color = tile_pixel(tile, pixel_x & 7, pixel_y & 7, signed)
                framebuffer[y][x] = palette(color, bgp)

    # Sprites: select the first ten sprites intersecting each scanline.
    # DMG rendering priority is lower X first, then lower OAM index.
    if lcdc & 0x02:
        height = 16 if lcdc & 0x04 else 8

        for y in range(HEIGHT):
            selected = []

            for index in range(40):
                offset = index * 4
                sprite_y, sprite_x, tile, attr = oam[offset:offset + 4]
                screen_y = sprite_y - 16
                screen_x = sprite_x - 8

                if screen_y <= y < screen_y + height:
                    selected.append(
                        (index, screen_x, screen_y, tile, attr)
                    )
                    if len(selected) == 10:
                        break

            selected.sort(key=lambda sprite: (sprite[1], sprite[0]))

            for index, screen_x, screen_y, tile, attr in selected:
                row = y - screen_y

                if attr & 0x40:
                    row = height - 1 - row

                if height == 16:
                    tile &= 0xFE
                    tile += row // 8
                    row %= 8

                for px in range(8):
                    x = screen_x + px

                    if not 0 <= x < WIDTH:
                        continue

                    tile_x = 7 - px if attr & 0x20 else px
                    color = tile_pixel(tile, tile_x, row)

                    # Color 0 is transparent.
                    if color == 0:
                        continue

                    # Behind-BG sprites are hidden over nonzero BG colors.
                    if attr & 0x80 and framebuffer[y][x] != palette(0, bgp):
                        continue

                    palette_value = obp1 if attr & 0x10 else obp0
                    framebuffer[y][x] = palette(color, palette_value)

    return framebuffer


def print_rust_array(framebuffer, name="FRAMEBUFFER"):
    print(f"const {name}: [[u8; {WIDTH}]; {HEIGHT}] = [")
    for row in framebuffer:
        print("    [" + ", ".join(map(str, row)) + "],")
    print("];")


if __name__ == "__main__":
    framebuffer = render(
        "ppu_test_data/dmg_vram.bin",
        "ppu_test_data/oam_sprites.bin",
        {
            "LCDC": 0x93,
            "BGP": 0xE4,
            "OBP0": 0xE4,
            "OBP1": 0x1B,
        },
    )

    print_rust_array(framebuffer, "FRAMEBUFFER_OAM_SPRITES")

    # Optional: save a readable color-index dump.
    Path("framebuffer.txt").write_text(
        "\n".join("".join(str(pixel) for pixel in row) for row in framebuffer)
    )