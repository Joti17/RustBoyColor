pub struct DMA {
    source_address: u16,
    transfer_index: u16,
    dot_ticks: u16,
    speed_remainder: u8,
    active: bool,
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

#[cfg(test)]
mod tests {
    use super::DMA;

    #[test]
    fn transfers_160_bytes_over_640_dots_at_normal_speed() {
        let mut dma = DMA::new();
        dma.start_transfer(0xC0);

        let mut first_batch = Vec::new();
        dma.tick(636, false, |address, index| {
            first_batch.push((index, address as u8));
        });
        assert_eq!(first_batch.len(), 159);
        assert!(dma.is_active());

        let mut final_batch = Vec::new();
        dma.tick(4, false, |address, index| {
            final_batch.push((index, address as u8));
        });
        assert_eq!(final_batch, vec![(159, 0x9F)]);
        assert!(!dma.is_active());
    }

    #[test]
    fn double_speed_keeps_half_dot_remainder_between_ticks() {
        let mut dma = DMA::new();
        dma.start_transfer(0xC0);

        let mut first_batch = Vec::new();
        dma.tick(1279, true, |address, index| {
            first_batch.push((index, address as u8));
        });
        assert_eq!(first_batch.len(), 159);
        assert!(dma.is_active());

        let mut final_batch = Vec::new();
        dma.tick(1, true, |address, index| {
            final_batch.push((index, address as u8));
        });
        assert_eq!(final_batch, vec![(159, 0x9F)]);
        assert!(!dma.is_active());
    }
}