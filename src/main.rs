#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::time::{Duration, Instant};
use crate::initialize::initializable;
use crate::mmu::MMU;

mod cpu;
mod mmu;
mod mbc;
mod mode;
mod initialize;
mod ppu;

fn main() {
    println!("Hello, world!");
    let mut mmu = MMU::new(true, "roms/whichboot.gb", "bootroms/cgb_boot.bin");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/09-op r,r.gb", "bootroms/cgb_boot.bin");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/cpu_instrs.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/10-bit ops.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/halt_bug.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/instr_timing/instr_timing.gb", "");
    mmu.init();
    let mut cpu = cpu::Cpu::new(true, false);
    
    let start_time = Instant::now();
    let timeout = Duration::from_secs(60);

    let mut cycles = 0u8;
    loop {
        if start_time.elapsed() > timeout {
            println!("Timeout reached, exiting.");
            break;
        }

        cycles = cpu.execute(&mut mmu);
        mmu.tick_timer(cycles as u16);
    }

}
