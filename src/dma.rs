pub struct DMA {
    source_address: u16,
    transfer_index: u16,
    dot_ticks: u16,
    speed_remainder: u8,
    active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VramTransferMode {
    General,
    HBlank,
}

pub struct VramDMA {
    source: u16,
    destination: u16,
    remaining_blocks: u8,
    dot_ticks: u32,
    transfer_mode: Option<VramTransferMode>,
    block_active: bool,
    cancelled: bool,
}

impl VramDMA {
    pub fn new() -> Self {
        Self {
            source: 0,
            destination: 0x8000,
            remaining_blocks: 0,
            dot_ticks: 0,
            transfer_mode: None,
            block_active: false,
            cancelled: false,
        }
    }

    pub fn read_register(&self, register: u8) -> u8 {
        match register {
            0x51 => (self.source >> 8) as u8,
            0x52 => self.source as u8 & 0xF0,
            0x53 => 0xE0 | ((self.destination >> 8) as u8 & 0x1F),
            0x54 => self.destination as u8 & 0xF0,
            0x55 if self.transfer_mode.is_some() => self.remaining_blocks.saturating_sub(1) & 0x7F,
            0x55 if self.cancelled => 0x80 | self.remaining_blocks.saturating_sub(1),
            0x55 => 0xFF,
            _ => 0xFF,
        }
    }

    pub fn write_register(
        &mut self,
        register: u8,
        value: u8,
        lcd_enabled: bool,
        in_hblank: bool,
    ) {
        if register == 0x55 {
            if self.transfer_mode == Some(VramTransferMode::HBlank) && value & 0x80 == 0 {
                self.transfer_mode = None;
                self.block_active = false;
                self.cancelled = true;
            } else if self.transfer_mode.is_none() {
                self.remaining_blocks = (value & 0x7F) + 1;
                self.dot_ticks = 0;
                self.cancelled = false;
                self.transfer_mode = Some(if value & 0x80 != 0 && lcd_enabled {
                    VramTransferMode::HBlank
                } else {
                    VramTransferMode::General
                });
                self.block_active = self.transfer_mode == Some(VramTransferMode::General);
                if self.transfer_mode == Some(VramTransferMode::HBlank) && in_hblank {
                    self.block_active = true;
                }
            }
            return;
        }

        if self.transfer_mode.is_some() {
            return;
        }

        match register {
            0x51 => self.source = (self.source & 0x00FF) | ((value as u16) << 8),
            0x52 => self.source = (self.source & 0xFF00) | (value as u16 & 0x00F0),
            0x53 => {
                self.destination =
                    0x8000 | ((value as u16 & 0x1F) << 8) | (self.destination & 0x00FF)
            }
            0x54 => {
                self.destination =
                    (self.destination & 0xFF00) | (value as u16 & 0x00F0)
            }
            _ => {}
        }
    }

    pub fn is_cpu_blocked(&self) -> bool {
        self.transfer_mode == Some(VramTransferMode::General) || self.block_active
    }

    pub fn on_hblank(&mut self) {
        if self.transfer_mode == Some(VramTransferMode::HBlank) && !self.block_active {
            self.block_active = true;
        }
    }

    pub fn on_lcd_disabled(&mut self) {
        if self.transfer_mode == Some(VramTransferMode::HBlank) {
            self.transfer_mode = Some(VramTransferMode::General);
            self.block_active = true;
        }
    }

    pub fn tick<F>(&mut self, dots: u16, mut transfer_byte: F)
    where
        F: FnMut(u16, u16),
    {
        if !self.block_active {
            return;
        }

        self.dot_ticks += dots as u32;
        while self.dot_ticks >= 2 && self.block_active {
            self.dot_ticks -= 2;
            transfer_byte(self.source, self.destination);
            self.source = self.source.wrapping_add(1);
            self.destination = 0x8000 | (self.destination.wrapping_add(1) & 0x1FFF);
            let bytes_in_block = (self.destination & 0x000F) as u8;

            if bytes_in_block == 0 {
                self.remaining_blocks -= 1;
                if self.remaining_blocks == 0 {
                    self.transfer_mode = None;
                    self.block_active = false;
                } else if self.transfer_mode == Some(VramTransferMode::HBlank) {
                    self.block_active = false;
                }
            }
        }
    }
}

impl DMA {
    pub fn new() -> DMA {
        DMA {
            source_address: 0,
            transfer_index: 0,
            dot_ticks: 0,
            speed_remainder: 0,
            active: false,
        }
    }

    pub fn start_transfer(&mut self, source: u8) {
        self.source_address = (source as u16) << 8;
        self.transfer_index = 0;
        self.dot_ticks = 0;
        self.speed_remainder = 0;
        self.active = true;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn tick<F>(
        &mut self,
        t_cycles: u16,
        is_double_speed: bool,
        mut transfer_byte: F,
    )
    where
        F: FnMut(u16, usize),
    {
        if !self.active {
            return;
        }

        let dots = if is_double_speed {
            let total_cycles = t_cycles as u32 + self.speed_remainder as u32;
            let dots = (total_cycles / 2) as u16;
            self.speed_remainder = (total_cycles % 2) as u8;
            dots
        } else {
            t_cycles
        };

        self.dot_ticks += dots;

        while self.dot_ticks >= 4 && self.active {
            self.dot_ticks -= 4;

            let index = self.transfer_index as usize;
            transfer_byte(self.source_address + self.transfer_index, index);

            self.transfer_index += 1;

            if self.transfer_index >= 0xA0 {
                self.active = false;
            }
        }
    }
}