use core::{arch::asm, cell::UnsafeCell, time::Duration};

use rand::{rngs::SmallRng, RngCore, SeedableRng};
use tock_registers::{interfaces::Writeable, register_structs, registers::ReadWrite};

use crate::{
    cpu::core_id,
    drivers::common::MMIODerefWrapper,
    exception::{self, asynchronous::local_irq_unmask},
    info,
    memory::{Address, Virtual},
    time::arch_time::spin_for,
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

static CORE_ACTIVATION_BASE_ADDR: u64 = 0xFFFF_FFFF_c18b_0000 + 0xE0;

type Registers = MMIODerefWrapper<RegisterBlock>;

pub static CORES: Registers =
    unsafe { Registers::new(Address::<Virtual>::new(CORE_ACTIVATION_BASE_ADDR as usize)) };

extern "Rust" {
    static _start_secondary: UnsafeCell<()>;
}

#[no_mangle]
unsafe fn kernel_init_secondary() -> ! {
    exception::handling_init();

    // Unmask interrupts on the boot CPU core.
    local_irq_unmask();

    let cid = core_id::<u64>() + 1;
    let mut small_rng = SmallRng::seed_from_u64(cid);
    loop {
        info!(
            "Hi from core {} with RNG: {:#x}",
            core_id::<u64>(),
            small_rng.next_u64() % 1000
        );

        spin_for(Duration::from_millis(cid * 10));
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
    info!(
        "Core {} starting with function at address {:#x}",
        core_id,
        _start_secondary.get() as u64
    );

    let mut addr = _start_secondary.get() as usize;

    addr &= 0xFFFF_FFFF;
    addr -= 0xBFF8_0000;

    let mut core_wakeup_addr: u64 = 0xFFFF_FFFF_c18b_0000;
    match core_id {
        1 => {
            CORES.ONE.set(addr as u64);
            core_wakeup_addr += 0xE0;
        }
        2 => {
            CORES.TWO.set(addr as u64);
            core_wakeup_addr += 0xE8;
        }
        3 => {
            CORES.THREE.set(addr as u64);
            core_wakeup_addr += 0xF0;
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
