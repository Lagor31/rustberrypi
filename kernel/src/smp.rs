use core::{ arch::asm, cell::UnsafeCell, time::Duration };

use aarch64_cpu::asm::barrier::{ dmb, dsb, isb };
use alloc::boxed::Box;
use rand::{ rngs::SmallRng, RngCore, SeedableRng };
use tock_registers::{ interfaces::Writeable, register_structs, registers::ReadWrite };

use crate::{
    cpu::{ core_id, wait_forever },
    drivers::common::MMIODerefWrapper,
    exception::{ self, asynchronous::local_irq_unmask },
    info,
    memory::{ Address, Virtual, __core_activation_address, mmu },
    time::time_manager,
    scheduler::{ RUNNING, SLEEPING, CURRENT, reschedule_from_context },
    debug,
    random,
    thread::{ reschedule, Thread, __switch_to, thread },
    synchronization::interface::Mutex,
};

register_structs! {
    #[allow(non_snake_case)]
    ///Regblock
    pub RegisterBlock {
        (0x00 => ONE: ReadWrite<u64>),
        (0x08 => TWO: ReadWrite<u64>),
        (0x10 => THREE: ReadWrite<u64>),
        (0x18 => @END),
    }
}

type Registers = MMIODerefWrapper<RegisterBlock>;

extern "Rust" {
    static _start_secondary: UnsafeCell<()>;
}

#[no_mangle]
unsafe fn kernel_init_secondary() -> ! {
    exception::handling_init();

    // Unmask interrupts on the current CPU core.
    local_irq_unmask();

    let entry_point = thread as *const () as u64;

    let core: usize = core_id();

    info!("Running Thread list for Core{}:\n{}", core, RUNNING[core]);

    CURRENT[core].lock(|cur_pid| {
        let mut wasted = Thread::new(entry_point);
        let next_thread: &mut Thread = RUNNING[core].next().expect("No next thread found!");

        *cur_pid = Some(next_thread.get_pid());
        info!(
            "Switching to thread PID={}\n {}",
            next_thread.get_pid(),
            next_thread.get_ex_context()
        );
        __switch_to(wasted.get_ex_context(), next_thread.get_ex_context());
    });

    wait_forever();

    /*    loop {
        info!("Hi from core {} with RNG: {:#x}", core, random::next_u64() % 10000);
        time_manager().spin_for(Duration::from_secs((core as u64) + 5));
        debug!("RUNNING Q Core{}:\n{}", core, RUNNING[core]);
        debug!("SLEEPING Q Core{}:\n{}", core, SLEEPING);
    } */
}

#[no_mangle]
pub unsafe fn start_core(core_id: u8) {
    let start_f_address = _start_secondary.get() as usize;

    info!("Core {} starting with function at address {:#x}", core_id, start_f_address);

    let mut core_wakeup_addr: u64 = (unsafe { __core_activation_address.get() as u64 }) + 0xe0;
    info!("Core Wakeup addr: {:#x}", core_wakeup_addr);
    let cores: Registers = Registers::new(Address::<Virtual>::new(core_wakeup_addr as usize));

    let phaddr = mmu
        ::try_kernel_virt_addr_to_phys_addr(Address::<Virtual>::new(start_f_address))
        .unwrap()
        .as_usize();

    info!("PhysAddr of startSecondary({:#x}) => {:#x}", start_f_address, phaddr);

    match core_id {
        1 => {
            cores.ONE.set(phaddr as u64);
        }
        2 => {
            cores.TWO.set(phaddr as u64);
            core_wakeup_addr += 0x8;
        }
        3 => {
            cores.THREE.set(phaddr as u64);
            core_wakeup_addr += 0x10;
        }
        _ => panic!("Can't start other cores"),
    }

    unsafe {
        asm!(
            "dc civac, {arg}",
            arg =  in(reg) core_wakeup_addr,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Probably overkill but weak memory ordering is hard...
    dmb(aarch64_cpu::asm::barrier::SY);
    isb(aarch64_cpu::asm::barrier::SY);
    dsb(aarch64_cpu::asm::barrier::SY);

    aarch64_cpu::asm::sev();
}