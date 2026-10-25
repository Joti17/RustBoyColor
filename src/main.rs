#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::time::{Duration, Instant};
use crate::initialize::Initializable;
use crate::mmu::MMU;
use minifb::{Window, WindowOptions};
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
    // let mut mmu = MMU::new(true, "roms/LOZ.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/02-interrupts.gb", "");

    let roms: Vec<_> = std::fs::read_dir("roms/blargg")
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "gb"))
                    .map(|e| e.path().to_path_buf())
                    .collect::<Vec<_>>()
            } else if path.extension().is_some_and(|ext| ext == "gb") {
                vec![path]
            } else {
                vec![]
            }
        })
        .collect();

    use rand::seq::SliceRandom;

    let rom = roms.choose(&mut rand::rng()).expect("No GB ROMs found");
    println!("Selected ROM: {}", rom.display());

    let mut mmu = MMU::new(true, rom.to_str().unwrap(), "");

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

        if is_ppu_finished {
            window.update_with_buffer(buffer.as_slice(), WIDTH, HEIGHT).unwrap();
        }
    }

}
