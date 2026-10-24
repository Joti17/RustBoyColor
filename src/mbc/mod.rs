#![allow(unused_variables, non_snake_case, dead_code, unused)]
pub(crate) mod mbc0;
pub(crate) mod mbc1;
pub(crate) mod mbc2;
pub(crate) mod mbc3;
pub(crate) mod mbc5;

pub(crate) struct Bios {
    data: Vec<u8>,
    enabled: bool,
}

impl Bios {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        assert!(
            data.len() == 0x100 || data.len() == 0x900,
            "BIOS length must be 0x100 or 0x900 bytes"
        );
        Self {
            data,
            enabled: true,
        }
    }

    pub(crate) fn read(&self, addr: u16) -> Option<u8> {
        if !self.enabled {
            return None;
        }

        let visible = addr <= 0x00FF
            || (self.data.len() == 0x900 && (0x0200..=0x08FF).contains(&addr));
        visible.then(|| self.data[addr as usize])
    }

    pub(crate) fn disable(&mut self) {
        self.enabled = false;
    }
}

pub trait MBC: Send {
    fn read_rom(&self, a: u16) -> u8;
    fn read_ram(&self, a: u16) -> u8;
    fn write_rom(&mut self, a: u16, v: u8);
    fn write_ram(&mut self, a: u16, v: u8);
    fn check_and_reset_ram_updated(&mut self) -> bool;

    fn is_battery_backed(&self) -> bool;
    fn load_ram(&mut self, ramdata: &[u8]);
    fn dump_ram(&self) -> Vec<u8>;
    fn disable_bios(&mut self);
}

/// # Arguments
/// `val` - the value at 0x148
/// # returns number of banks

pub fn ram_banks(val: u8) -> u8 {
    match val {
        1 => 1, // 2KiB, using 1 8KiB bank
        2 => 1,
        3 => 4,
        4 => 16,
        5 => 8,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::Bios;

    #[test]
    fn dmg_bios_is_visible_only_in_the_first_256_bytes() {
        let bios = Bios::new(vec![0xA5; 0x100]);

        assert_eq!(bios.read(0x0000), Some(0xA5));
        assert_eq!(bios.read(0x00FF), Some(0xA5));
        assert_eq!(bios.read(0x0100), None);
        assert_eq!(bios.read(0x0200), None);
    }

    #[test]
    fn cgb_bios_leaves_cartridge_header_visible() {
        let mut data = vec![0x5A; 0x900];
        data[0x0100] = 0xA5;
        let bios = Bios::new(data);

        assert_eq!(bios.read(0x00FF), Some(0x5A));
        assert_eq!(bios.read(0x0100), None);
        assert_eq!(bios.read(0x01FF), None);
        assert_eq!(bios.read(0x0200), Some(0x5A));
        assert_eq!(bios.read(0x08FF), Some(0x5A));
        assert_eq!(bios.read(0x0900), None);
    }

    #[test]
    fn disabled_bios_is_not_visible() {
        let mut bios = Bios::new(vec![0xA5; 0x100]);
        bios.disable();

        assert_eq!(bios.read(0x0000), None);
    }
}

// Same as ram_banks
pub fn rom_banks(val: u8) -> usize {
    if val <= 8 {
        2 << val
    }
    else {
        0
    }
}