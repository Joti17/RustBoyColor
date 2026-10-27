#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::time::{Duration, Instant};
use crate::initialize::Initializable;
use crate::mmu::MMU;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use rand::prelude::IndexedRandom;
use crate::input::get_input;
use crate::mode::ClockSpeed;

mod cpu;
mod mmu;
mod mbc;
mod mode;
mod initialize;
mod ppu;
mod input;
mod dma;
mod stub_ppu;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

fn main() {
    println!("Hello, world!");
    let is_ppu_finished = true;
    // let mut mmu = MMU::new(true, "roms/whichboot.gb", "bootroms/cgb_boot.bin");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/09-op r,r.gb", "bootroms/cgb_boot.bin");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/cpu_instrs.gb", "");
    // let mut mmu = MMU::new(true, "roms/tests/tobu.gb", "");
    // let mut mmu = MMU::new(true, "roms/tests/moneye/interrupt_time/interrupt_time.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/10-bit ops.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/halt_bug.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/instr_timing/instr_timing.gb", "");
    // let mut mmu = MMU::new(true, "roms/LOZ.gb", "roms/gb_bios.bin");
    /*use stub_ppu;
    test!(just_render);
    */
    return;

    let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/02-interrupts.gb", "");


    mmu.init();
    let mut cpu = cpu::Cpu::new(true, false);
    
    let start_time = Instant::now();
    let timeout = Duration::from_secs(60);

    let mut window = Window::new(
            "Game Boy Color",
            WIDTH * 4,
            HEIGHT * 4,
            WindowOptions::default(),
        ).unwrap();


    let mut buffer: Vec<u32> = vec![0xFFFFFFFF; WIDTH * HEIGHT * 4];
    let mut input = gilrs::Gilrs::new().unwrap();

    let mut cycles = 0u8;
    

    while window.is_open() {
        let cycles_per_frame;
        if cpu.speed == ClockSpeed::Double{
            cycles_per_frame = 70224*2;
        } else {
            cycles_per_frame = 70224;
        }
        let mut overflow_cycles = 0;
        let mut cycles = overflow_cycles;
        let mut cycles_this_instruction = 0;
        while cycles < cycles_per_frame {
            get_input(&mut mmu, &mut window, &mut input);

            if start_time.elapsed() > timeout {
                println!("Timeout reached, exiting.");
                break;
            }

            cycles_this_instruction = cpu.execute(&mut mmu) as usize;
            cycles += cycles_this_instruction;
            mmu.tick_timer(cycles_this_instruction as u16);
            mmu.tick(
                cpu.speed == ClockSpeed::Double,
                cycles_this_instruction as u16,
            );
        }
        overflow_cycles = cycles - cycles_per_frame;

        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            mmu.ppu.dump_vram();
        }

        if is_ppu_finished {
            window.update_with_buffer(buffer.as_slice(), WIDTH, HEIGHT).unwrap();
        }
    }

}
