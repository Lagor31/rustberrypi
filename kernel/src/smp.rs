use core::{arch::asm, cell::UnsafeCell};

use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::ReadWrite,
};

use crate::{
    drivers::common::MMIODerefWrapper,
    memory::{Address, Virtual},
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
pub unsafe fn start_core(core_id: u8) {
    /*  info!(
           "Core {} starting with function at address {}",
           core_id,
           _start_secondary.get() as u64
       );
    */
    let addr = _start_secondary.get() as u64;
    //   let fn_addr = (&func as *const _) as u64;
    match core_id {
        1 => CORES.ONE.set(addr),
        _ => panic!("Can't start other cores"),
    }

    let _r = CORES.ONE.get();

    const ADD: u64 = 0xFFFF_FFFF_c18b_0000 + 0xE0;
    unsafe {
        asm!(
            "dc civac, {arg}",
            arg =  in(reg) ADD,
            options(nomem, nostack, preserves_flags)
        );
    }

    aarch64_cpu::asm::sev();
}
