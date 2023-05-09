// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! The `kernel` binary.

#![feature(format_args_nl)]
#![no_main]
#![no_std]
#![allow(clippy::upper_case_acronyms)]
#![allow(incomplete_features)]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(const_option)]
#![feature(core_intrinsics)]
#![feature(generic_const_exprs)]
#![feature(int_roundings)]
#![feature(is_sorted)]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(step_trait)]
#![feature(trait_alias)]
#![feature(unchecked_math)]
#![feature(never_type)]

use core::time::Duration;

use alloc::boxed::Box;
use rand::{rngs::SmallRng, RngCore, SeedableRng};

/* use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;
use crate::smp::start_core;

 */
use crate::{
    board::version,
    cpu::{core_id, wait_forever},
    smp::start_core,
};

extern crate alloc;
extern crate rand;
extern crate spin;
mod panic_wait;
mod synchronization;

pub mod backtrace;
pub mod board;
pub mod common;
pub mod console;
pub mod cpu;
pub mod driver;
pub mod drivers;
pub mod exception;
pub mod memory;
pub mod print;
pub mod smp;
pub mod state;
pub mod symbols;
pub mod time;

/// Early init code.
///
/// When this code runs, virtual memory is already enabled.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - Printing will not work until the respective driver's MMIO is remapped.
#[no_mangle]
unsafe fn kernel_init() -> ! {
    exception::handling_init();
    memory::init();

    // Initialize the timer subsystem.
    if let Err(x) = time::init() {
        panic!("Error initializing timer subsystem: {}", x);
    }

    // Initialize the BSP driver subsystem.
    if let Err(x) = drivers::init() {
        panic!("Error initializing BSP driver subsystem: {}", x);
    }

    // Initialize all device drivers.
    driver::driver_manager().init_drivers_and_irqs();

    memory::mmu::kernel_add_mapping_records_for_precomputed();

    // Unmask interrupts on the boot CPU core.
    exception::asynchronous::local_irq_unmask();

    // Announce conclusion of the kernel_init() phase.
    state::state_manager().transition_to_single_core_main();

    // Transition from unsafe to safe.
    kernel_main()
}

/// The main function running after the early init.
fn kernel_main() -> ! {
    info!("{}", version());
    info!("Booting on: {}", board::board_name());

    info!("MMU online:");
    memory::mmu::kernel_print_mappings();

    let (_, privilege_level) = exception::current_privilege_level();
    info!("Current privilege level: {}", privilege_level);

    info!("Exception handling state:");
    exception::asynchronous::print_state();

    info!(
        "Architectural timer resolution: {} ns",
        time::time_manager().resolution().as_nanos()
    );

    info!("Drivers loaded:");
    driver::driver_manager().enumerate();

    info!("Registered IRQ handlers:");
    exception::asynchronous::irq_manager().print_handler();

    info!("Kernel heap:");
    memory::heap_alloc::kernel_heap_allocator().print_usage();

    /*
    time::time_manager().set_timeout_once(Duration::from_secs(5), Box::new(|| info!("Once 5")));
    time::time_manager().set_timeout_once(Duration::from_secs(3), Box::new(|| info!("Once 2")));
    time::time_manager()
        .set_timeout_periodic(Duration::from_secs(1), Box::new(|| info!("Periodic 1 sec")));
     */

    info!("Echoing input now");
    //spin_for(Duration::from_secs(3));

    use alloc::collections::BTreeMap;
    let mut treemap = BTreeMap::new();
    treemap.insert(33, "trentatre");
    treemap.insert(2, "due");
    treemap.insert(5, "cinque");
    treemap.insert(88, "ottootto");
    treemap.insert(3, "tre");

    info!(
        "first K={} V={}",
        treemap.first_key_value().unwrap().0,
        treemap.first_key_value().unwrap().1
    );

    info!(
        "last K={} V={}",
        treemap.last_key_value().unwrap().0,
        treemap.last_key_value().unwrap().1
    );

    info!("Whole thing");
    for r in treemap {
        info!(
            "
         K={} V={}",
            r.0, r.1
        );
    }

    info!("Enabling other cores");
    (1..=3).for_each(|i| unsafe { start_core(i) });

    state::state_manager().transition_to_multi_core_main();

    let cid: u64 = core_id::<u64>();

    time::time_manager().set_timeout_periodic(
        Duration::from_secs(cid + 10),
        Box::new(|_ec| {
            let cid = core_id::<u64>() + 1;
            let mut small_rng = SmallRng::seed_from_u64(cid);
            info!(
                "Hi from core {} with RNG: {:#x}\n\n{}",
                core_id::<u64>(),
                small_rng.next_u64() % 1000,
                _ec
            );
            _ec.gpr[0] += 2;
        }),
    );
    wait_forever();

    /*     loop {
        use crate::cpu::core_id;
        let core_id = core_id::<u64>();
        let mut small_rng = SmallRng::seed_from_u64(core_id);
        loop {
            info!(
                "Hi from core {} with RNG: {:#x}",
                core_id,
                small_rng.next_u64() % 1000
            );
        }
    } */
}
