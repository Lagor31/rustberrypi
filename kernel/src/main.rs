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
use core::{cell::UnsafeCell, time::Duration};

use alloc::boxed::Box;
use exception::arch_exception::ExceptionContext;
use tock_registers::interfaces::Readable;

/* use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;
use crate::smp::start_core;

 */
use crate::{
    board::version,
    cpu::{core_id, wait_forever},
    exception::asynchronous::local_irq_mask_save,
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

extern "C" {
    fn __switch_to(current: &mut ExceptionContext, next: &mut ExceptionContext);
}

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
        info!("\n")
    }

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
    info!("Echoing input now");

    /*     info!("Enabling other cores");
    (1..=3).for_each(|i| unsafe { start_core(i) });

    state::state_manager().transition_to_multi_core_main(); */
    info!("Kernel Init:\n");
    info!("SPSel={}", aarch64_cpu::registers::SPSel.get());
    info!("SP_EL0={:#x}", aarch64_cpu::registers::SP_EL0.get());
    info!("\tSP={:#x}", aarch64_cpu::registers::SP.get());

    time::time_manager().set_timeout_periodic(
        Duration::from_millis(500),
        Box::new(|_ec| {
            info!("\nTimer interrupt!");
            info!("\tSPSel={}", aarch64_cpu::registers::SPSel.get());
            info!("\tSP_EL0={:#x}", aarch64_cpu::registers::SP_EL0.get());
            info!("\tSP={:#x}", aarch64_cpu::registers::SP.get());

            //time_manager().spin_for(Duration::from_millis(500));
        }),
    );

    let entry_point = thread as *const () as u64;

    let mut wasted = thread::Thread::new(entry_point);
    let mut new_thread = thread::Thread::new(entry_point);
    info!("Switching to thread {}...", new_thread.get_pid());
    unsafe {
        __switch_to(wasted.get_ex_context(), new_thread.get_ex_context());
    }

    wait_forever();
}

pub fn thread() {
    let mut c = 0;
    loop {
        //let _saved = local_irq_mask_save();
        info!("\nHello from thread! C={}", c);
        info!("\tSPSel={}", aarch64_cpu::registers::SPSel.get());
        info!("\tSP={:#x}", aarch64_cpu::registers::SP.get());

        time_manager().spin_for(Duration::from_millis(2000));
        c += 1;
    }
}

pub fn wait_thread() {
    wait_forever();
}

/* pub fn store_context(s: &mut ExceptionContext, d: &mut ExceptionContext) {
    d.elr_el1 = s.elr_el1;
    //TODO: d.esr_el1 = s.esr_el1;
    d.gpr = s.gpr;
    d.lr = s.lr;
    d.sp_el0 = s.sp_el0;
    //TODO: d.spsr_el1 = s.spsr_el1;
}
 */
