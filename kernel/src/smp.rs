use core::{arch::asm, cell::UnsafeCell, time::Duration};

use rand::{rngs::SmallRng, RngCore, SeedableRng};
use tock_registers::{interfaces::Writeable, register_structs, registers::ReadWrite};

use crate::{
    cpu::core_id,
    drivers::common::MMIODerefWrapper,
    exception::{self, asynchronous::local_irq_unmask},
    info,
    memory::{Address, Virtual, __core_activation_address},
    time::time_manager,
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

    let cid = core_id::<u64>();
    let mut small_rng = SmallRng::seed_from_u64(cid);
    loop {
        info!(
            "Hi from core {} with RNG: {:#x}",
            core_id::<u64>(),
            small_rng.next_u64() % 1000
        );

        time_manager().spin_for(Duration::from_secs(cid + 10));
    }

    //    wait_forever();
    /*     loop {
        info!(
            "Hi from core {} with RNG: {:#x}",
            cid,
            small_rng.next_u64() % 1000
        );
        //spin_for(Duration::from_micros(core_id * 10));
    } */
    //wait_forever();
}

#[no_mangle]
pub unsafe fn start_core(core_id: u8) {
    let mut addr = _start_secondary.get() as usize;

    info!(
        "Core {} starting with function at address {:#x}",
        core_id, addr
    );

    let mut core_wakeup_addr: u64 = unsafe { __core_activation_address.get() as u64 } + 0xE0;
    info!("Core Wakeup addr: {:#x}", core_wakeup_addr);
    let cores: Registers = Registers::new(Address::<Virtual>::new(core_wakeup_addr as usize));

    // TODO: Virtual to phys translation done by hand is ugly and error prone
    addr &= 0xFFFF_FFFF;
    addr -= 0xBFF8_0000;

    match core_id {
        1 => {
            cores.ONE.set(addr as u64);
        }
        2 => {
            cores.TWO.set(addr as u64);
            core_wakeup_addr += 0x8;
        }
        3 => {
            cores.THREE.set(addr as u64);
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
    aarch64_cpu::asm::barrier::dmb(aarch64_cpu::asm::barrier::SY);
    aarch64_cpu::asm::barrier::isb(aarch64_cpu::asm::barrier::SY);
    aarch64_cpu::asm::barrier::dsb(aarch64_cpu::asm::barrier::SY);

    aarch64_cpu::asm::sev();
}
