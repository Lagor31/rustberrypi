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
#![allow(dead_code, unused_imports)]
#![feature(linked_list_remove)]

use core::{ cell::UnsafeCell, panic, time::Duration };

use crate::scheduler::{ reschedule_from_context, SLEEPING };
use crate::thread::{ thread, wait_thread, Thread };
use alloc::boxed::Box;
use exception::arch_exception::ExceptionContext;
use tock_registers::interfaces::Readable;

use crate::{
    board::version,
    cpu::{ core_id, wait_forever },
    exception::asynchronous::{ local_irq_mask_save, local_irq_restore },
    scheduler::{ CURRENT, RUNNING },
    smp::start_core,
    time::time_manager,
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
pub mod scheduler;
pub mod smp;
pub mod state;
pub mod symbols;
pub mod thread;
pub mod time;
pub mod random;

extern "Rust" {
    static __test_me: UnsafeCell<()>;
}
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
    for _ in 0..10 {
        info!("\n");
    }

    info!("{}", version());
    info!("Booting on: {}", board::board_name());

    info!("MMU online:");
    memory::mmu::kernel_print_mappings();

    let (_, privilege_level) = exception::current_privilege_level();
    info!("Current privilege level: {}", privilege_level);

    info!("Exception handling state:");
    exception::asynchronous::print_state();

    info!("Architectural timer resolution: {} ns", time::time_manager().resolution().as_nanos());

    info!("Drivers loaded:");
    driver::driver_manager().enumerate();

    info!("Registered IRQ handlers:");
    exception::asynchronous::irq_manager().print_handler();

    info!("Kernel heap:");
    memory::heap_alloc::kernel_heap_allocator().print_usage();
    info!("Echoing input now");

    info!("Enabling other cores");
    (1..=3).for_each(|i| unsafe { start_core(i) });

    state::state_manager().transition_to_multi_core_main();
    info!("Kernel Init:\n");
    info!("SPSel={}", aarch64_cpu::registers::SPSel.get());
    info!("SP_EL0={:#x}", aarch64_cpu::registers::SP_EL0.get());
    info!("\tSP={:#x}", aarch64_cpu::registers::SP.get());

    let core: usize = core_id();

    let entry_point = thread as *const () as u64;
    let idle_thread_ep = wait_thread as *const () as u64;

    let idle_thread = Thread::new(idle_thread_ep);
    RUNNING[core].add(idle_thread);

    for i in 0..4 {
        for _ in 0..10 {
            let new_thread = Thread::new(entry_point);
            RUNNING[core + i].add(new_thread);
        }
    }

    info!("Running Thread list for Core{}:\n{}", core, RUNNING[core]);

    //Setting the scheduler timer interrupt
    time_manager().set_timeout_periodic(
        Duration::from_millis(2),
        Box::new(|_ec| {
            reschedule_from_context(_ec);
        })
    );

    time_manager().set_timeout_periodic(
        Duration::from_secs(5),
        Box::new(|_ec| {
            let core = core_id::<usize>();
            debug!("Hi from core {}", core);
            let entry_point = thread as *const () as u64;

            let num_new_threads = (random::next_u64() % 10) + 1;
            debug!("Creating {} new threads", num_new_threads);
            for _ in 0..num_new_threads {
                let new_thread = Thread::new(entry_point);
                RUNNING[core].add(new_thread);
            }

            debug!("RUNNING Q Core{}:\n{}", core, RUNNING[core]);
            if SLEEPING.size() > 0 {
                SLEEPING.clear();
            }
            debug!("SLEEPING Q Core{}:\n{}", core, SLEEPING);

            memory::heap_alloc::kernel_heap_allocator().print_usage();
        })
    );
    wait_forever();
}