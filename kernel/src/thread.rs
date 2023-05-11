use core::{
    alloc::{GlobalAlloc, Layout, LayoutError},
    mem,
    sync::atomic::{AtomicU64, Ordering},
};

use aarch64_cpu::registers::{ESR_EL1, SPSR_EL1};
use tock_registers::{interfaces::Writeable, registers::InMemoryRegister};

use crate::{
    exception::arch_exception::{EsrEL1, ExceptionContext, SpsrEL1},
    memory,
};

pub struct Thread {
    pid: u64,
    context: ExceptionContext,
}

static PID: AtomicU64 = AtomicU64::new(1);

impl Thread {
    pub fn new(entry_point: u64) -> Self {
        let out = Thread {
            pid: PID.fetch_add(1, Ordering::Acquire),
            context: Self::make_context(entry_point),
        };
        out
    }

    pub fn get_ex_context(&mut self) -> &mut ExceptionContext {
        &mut self.context
    }

    pub fn get_pid(&self) -> u64 {
        self.pid
    }

    fn make_context(entry_point: u64) -> ExceptionContext {
        let p;
        unsafe {
            p = memory::heap_alloc::kernel_heap_allocator()
                .alloc(Layout::from_size_align(8192, 4096).unwrap());
        };

        let mut ptr = p as u64;
        ptr += 8192;

        let spsr_el1_init = 0x364;
        //USermode:
        //spsr_el1_init &= 0xFFF8;
        ExceptionContext {
            gpr: [0; 30],
            lr: entry_point,
            elr_el1: entry_point,
            spsr_el1: SpsrEL1 {
                0: InMemoryRegister::<u64, SPSR_EL1::Register>::new(spsr_el1_init),
            },
            esr_el1: EsrEL1 {
                0: InMemoryRegister::<u64, ESR_EL1::Register>::new(0),
            },
            _res_sp: 0,
            sp_el0: ptr,
        }
    }
}
