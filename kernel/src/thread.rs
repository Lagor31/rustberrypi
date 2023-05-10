use core::sync::atomic::{AtomicU64, Ordering};

use aarch64_cpu::registers::{ESR_EL1, SPSR_EL1};
use tock_registers::registers::InMemoryRegister;

use crate::exception::arch_exception::{EsrEL1, ExceptionContext, SpsrEL1};

pub struct Thread {
    pid: u64,
    context: ExceptionContext,
}

static PID: AtomicU64 = AtomicU64::new(0);

impl Thread {
    pub fn new(entry_point: u64) -> Self {
        let out = Thread {
            pid: PID.fetch_add(1, Ordering::Acquire),
            context: Self::make_context(entry_point),
        };
        //let _x = out.context.esr_el1.0.is_set(ESR_EL1::IL);
        out
    }

    pub fn get_ex_context(&mut self) -> &mut ExceptionContext {
        &mut self.context
    }

    pub fn get_pid(&self) -> u64 {
        self.pid
    }

    fn make_context(entry_point: u64) -> ExceptionContext {
        ExceptionContext {
            gpr: [0; 30],
            lr: entry_point,
            elr_el1: entry_point,
            spsr_el1: SpsrEL1 {
                0: InMemoryRegister::<u64, SPSR_EL1::Register>::new(0),
            },
            esr_el1: EsrEL1 {
                0: InMemoryRegister::<u64, ESR_EL1::Register>::new(0),
            },
        }
    }
}
