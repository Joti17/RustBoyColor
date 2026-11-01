#![allow(unused_variables, non_snake_case, dead_code, unused)]
use std::time::{Duration, Instant};
use gilrs::{Button, Event};
use crate::initialize::Initializable;
use crate::mmu::MMU;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use rand::prelude::IndexedRandom;
use crate::fpstracker::FpsTracker;
use crate::input::get_input;
use crate::mode::ClockSpeed;
use crate::ppu::PPU;

mod cpu;
mod mmu;
mod mbc;
mod mode;
mod initialize;
mod ppu;
mod input;
mod dma;
mod fpstracker;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

fn main() {
    println!("Hello, world!");
    // let mut mmu = MMU::new(true, "roms/whichboot.gb", "bootroms/cgb_boot.bin", None);
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/09-op r,r.gb", "bootroms/cgb_boot.bin");
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/cpu_instrs.gb", "");
    // let mut mmu = MMU::new(true, "roms/tests/bully.gb", "bootroms/cgb_boot.bin", None);
    // let mut mmu = MMU::new(true, "roms/blargg/interrupt_time/interrupt_time.gb", "bootroms/cgb_boot.bin", None);
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/10-bit ops.gb", "");
    // let mut mmu = MMU::new(true, "roms/blargg/halt_bug.gb", "bootroms/cgb_boot.bin", None);
    // let mut mmu = MMU::new(true, "roms/blargg/instr_timing/instr_timing.gb", "");
    // let mut mmu = MMU::new(true, "roms/LOZ.gb", "bootroms/cgb_boot.bin", "roms/LOZ.sav".to_string().into());
    // let mut mmu = MMU::new(true, "roms/pm.gb", "bootroms/cgb_boot.bin", "roms/pm.sav".to_string().into());
    // let mut mmu = MMU::new(true, "roms/smb.gbc", "bootroms/cgb_boot.bin", "roms/smb.sav".to_string().into());
    let mut mmu = MMU::new(true, "roms/badapple.gbc", "bootroms/cgb_boot.bin", "".to_string().into());
    // let mut mmu = MMU::new(true, "roms/blargg/cpu_instrs/individual/02-interrupts.gb", "");


    mmu.init();
    let mut cpu = cpu::Cpu::new(true, true);
    
    let start_time = Instant::now();
    let timeout = Duration::from_secs(60);

    let mut window = Window::new(
            "Game Boy Color",
            WIDTH * 4,
            HEIGHT * 4,
            WindowOptions::default(),
        ).unwrap();


    let mut buffer: Vec<u32> = vec![0xFFFFFFFF; WIDTH * HEIGHT];
    let mut overflow_cycles = 0usize;
    // window.set_target_fps(60);
    window.limit_update_rate(Some(Duration::from_nanos((1_000_000_000.0 / 59.7275) as u64)));
    let mut input = input::Pads::new();

    let mut cycles = 0u8;
    

    let mut frame = 0u32;
    // let mut tracker = FpsTracker::new();
    while window.is_open() {
        let cycles_per_frame;
        if cpu.speed == ClockSpeed::Double{
            cycles_per_frame = 70224*2;
        } else {
            cycles_per_frame = 70224;
        }
        let mut cycles = overflow_cycles;
        frame += 1;
        // tracker.update();
        if frame & 1023 == 0{ // ~ once every 
            mmu.save();
            // println!("{} fps", tracker.get_fps());
            println!("{} fps", window.get_fps());
        }
        input.poll();
        while cycles < cycles_per_frame {

            get_input(&mut mmu, &mut window, &mut input);


            let c = cpu.execute(&mut mmu) as usize;
            cycles += c;
            mmu.tick_timer(c as u16);

            let dots = mmu.tick(cpu.speed == ClockSpeed::Double, c as u16);
            if mmu.tick_ppu(dots) {
                mmu.vram_dma.as_mut().unwrap().on_hblank();
            }
        }
        overflow_cycles = cycles.saturating_sub(cycles_per_frame);

        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            mmu.ppu.dump_vram();
        }

        for (dst, &src) in buffer.iter_mut().zip(mmu.ppu.framebuffer.iter()) {
            *dst = PPU::rgb555_to_xrgb8888(src);
        }
        let lit = mmu.ppu.framebuffer.iter().filter(|&&p| p != 0).count();
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
        input.end_frame();
    }

}
