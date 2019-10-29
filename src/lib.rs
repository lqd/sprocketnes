//
// Author: Patrick Walton
//

#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate sdl2;
extern crate time;

// NB: This must be first to pick up the macro definitions. What a botch.
#[macro_use]
pub mod util;

pub mod apu;
pub mod audio;
#[macro_use]
pub mod cpu;
pub mod disasm;
pub mod gfx;
pub mod input;
pub mod mapper;
pub mod mem;
pub mod ppu;
pub mod rom;

// C library support
#[cfg(feature = "audio")]
pub mod speex;

use apu::Apu;
use cpu::Cpu;
use gfx::{Gfx, Scale};
use input::{Input, InputResult};
use mapper::Mapper;
use mem::MemMap;
use ppu::{Oam, Ppu, Vram};
use rom::Rom;
use util::Save;

use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::File;
use std::mem as smem;
use std::path::Path;
use std::rc::Rc;

#[derive(Default)]
struct Stats {
    last_time: f64,
    frames: usize,
    conditional_jumps: usize,
    conditional_jumps_s: usize,
    steps: usize,
    steps_s: usize,
    branches_taken: Vec<(u16, u16)>,
    branches_not_taken: Vec<(u16, u16)>,
    addrs_visited: HashSet<(u16, u16)>,
    addrs_not_visited: HashSet<(u16, u16)>,
    addrs_visited_old: HashSet<(u16, u16)>,
    addrs_not_visited_old: HashSet<(u16, u16)>,
    loads: Vec<(char, u16)>,
    stores: Vec<(char, u16)>,
    memory_reads: HashSet<u16>,
    memory_reads_old: HashSet<u16>,
    memory_writes: HashSet<u16>,
    memory_writes_old: HashSet<u16>,
}

impl Stats {
    fn new() -> Self {
        Self {
            last_time: time::precise_time_s(),
            ..Default::default()
        }
    }
}

fn record_fps(stats: &mut Stats) {
    if !true {
        return;
    }

    if !true {
        assert!(stats.addrs_visited.is_empty());
        assert!(stats.addrs_not_visited.is_empty());

        stats.addrs_visited.extend(stats.branches_taken.iter());
        stats
            .addrs_not_visited
            .extend(stats.branches_not_taken.iter());

        let addrs_visited_delta: HashSet<_> = stats
            .addrs_visited
            .intersection(&stats.addrs_visited_old)
            .collect();
        let addrs_not_visited_delta: HashSet<_> = stats
            .addrs_not_visited
            .intersection(&stats.addrs_not_visited_old)
            .collect();
        println!(
            "{} -> {} addresses visited, stable: {} {:.4}%, {} -> {} addresses not visited, stable: {}, {:.4}%",
            stats.addrs_visited_old.len(),
            stats.addrs_visited.len(),
            addrs_visited_delta.len(),
            100.0 * addrs_visited_delta.len() as f64 / stats.addrs_visited.len() as f64,
            stats.addrs_not_visited_old.len(),
            stats.addrs_not_visited.len(),
            addrs_not_visited_delta.len(),
            100.0 * addrs_not_visited_delta.len() as f64 / stats.addrs_not_visited.len() as f64,
        );

        stats.branches_taken.clear();
        stats.branches_not_taken.clear();

        stats.addrs_visited_old = smem::replace(&mut stats.addrs_visited, Default::default());
        stats.addrs_not_visited_old =
            smem::replace(&mut stats.addrs_not_visited, Default::default());
    }

    let now = time::precise_time_s();
    if now >= stats.last_time + 1f64 {
        if true {
            println!(
                "{} FPS - cond jumps: {:.4} /f - steps: {:.4} /f - cond jmps %: {:.4}% /f \
                 - branches taken: {} (uniq: {}), not taken: {} (uniq: {})",
                stats.frames,
                // stats.conditional_jumps,
                stats.conditional_jumps_s as f64 / stats.frames as f64,
                // stats.steps,
                stats.steps_s as f64 / stats.frames as f64,
                // 100.0 * stats.conditional_jumps as f64 / stats.steps as f64,
                100.0
                    * ((stats.conditional_jumps_s as f64 / stats.frames as f64)
                        / (stats.steps_s as f64 / stats.frames as f64)),
                stats.branches_taken.len(),
                stats.addrs_visited.len(),
                stats.branches_not_taken.len(),
                stats.addrs_not_visited.len(),
            );
        }
        stats.frames = 0;
        stats.conditional_jumps_s = 0;
        stats.steps_s = 0;
        stats.last_time = now;
    } else {
        stats.frames += 1;
    }

    if true {
        assert!(stats.memory_reads.is_empty());
        stats
            .memory_reads
            .extend(stats.loads.iter().map(|&(_, addr)| addr));

        assert!(stats.memory_writes.is_empty());
        stats
            .memory_writes
            .extend(stats.stores.iter().map(|&(_, addr)| addr));

        let memory_reads_delta: HashSet<_> = stats
            .memory_reads
            .intersection(&stats.memory_reads_old)
            .collect();

        let memory_writes_delta: HashSet<_> = stats
            .memory_writes
            .intersection(&stats.memory_writes_old)
            .collect();

        println!(
            "memory: reads {} -> {} , stable: {} {:.4}%, writes {} -> {} , stable: {} {:.4}%",
            stats.memory_reads_old.len(),
            stats.memory_reads.len(),
            memory_reads_delta.len(),
            100.0 * memory_reads_delta.len() as f64 / stats.memory_reads.len() as f64,
            stats.memory_writes_old.len(),
            stats.memory_writes.len(),
            memory_writes_delta.len(),
            100.0 * memory_writes_delta.len() as f64 / stats.memory_writes.len() as f64,
        );

        stats.loads.clear();
        stats.stores.clear();

        stats.memory_reads_old = smem::replace(&mut stats.memory_reads, Default::default());
        stats.memory_writes_old = smem::replace(&mut stats.memory_writes, Default::default());
    }
}

/// Starts the emulator main loop with a ROM and window scaling. Returns when the user presses ESC.
pub fn start_emulator(rom: Rom, scale: Scale) {
    let rom = Box::new(rom);
    println!("Loaded ROM: {}", rom.header);

    let (mut gfx, sdl) = Gfx::new(scale);
    let audio_buffer = audio::open(&sdl);

    let mapper: Box<dyn Mapper + Send> = mapper::create_mapper(rom);
    let mapper = Rc::new(RefCell::new(mapper));
    let ppu = Ppu::new(Vram::new(mapper.clone()), Oam::new());
    let input = Input::new(sdl);
    let apu = Apu::new(audio_buffer);
    let memmap = MemMap::new(ppu, input, mapper, apu);
    let mut cpu = Cpu::new(memmap);

    // TODO: Add a flag to not reset for nestest.log
    cpu.reset();

    let mut stats = Stats::new();

    loop {
        cpu.step();

        stats.steps += 1;
        stats.steps_s += 1;

        if cpu.conditional_jump {
            stats.conditional_jumps += 1;
            stats.conditional_jumps_s += 1;
        }

        let ppu_result = cpu.mem.ppu.step(cpu.cy);
        if ppu_result.vblank_nmi {
            cpu.nmi();
        } else if ppu_result.scanline_irq {
            cpu.irq();
        }

        #[cfg(feature = "audio")]
        cpu.mem.apu.step(cpu.cy);

        if ppu_result.new_frame {
            std::mem::swap(&mut stats.branches_taken, &mut cpu.branches_taken);
            std::mem::swap(&mut stats.branches_not_taken, &mut cpu.branches_not_taken);

            std::mem::swap(&mut stats.stores, &mut cpu.stores);
            std::mem::swap(&mut stats.loads, &mut cpu.loads);

            gfx.tick();
            gfx.composite(&mut *cpu.mem.ppu.screen);

            record_fps(&mut stats);

            #[cfg(feature = "audio")]
            cpu.mem.apu.play_channels();

            match cpu.mem.input.check_input() {
                InputResult::Continue => {}
                InputResult::Quit => break,
                InputResult::SaveState => {
                    cpu.save(&mut File::create(&Path::new("state.sav")).unwrap());
                    gfx.status_line.set("Saved state".to_string());
                }
                InputResult::LoadState => {
                    cpu.load(&mut File::open(&Path::new("state.sav")).unwrap());
                    gfx.status_line.set("Loaded state".to_string());
                }
            }
        }
    }

    audio::close();
}
