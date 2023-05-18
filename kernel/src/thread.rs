use core::{
    alloc::{ GlobalAlloc, Layout, LayoutError },
    mem,
    sync::atomic::{ AtomicU64, Ordering },
    time::Duration,
    fmt,
};

use aarch64_cpu::registers::{ DAIF, ESR_EL1, SPSR_EL1 };
use tock_registers::{ interfaces::{ Readable, Writeable }, registers::InMemoryRegister };

use crate::{
    cpu::{ wait_forever, core_id },
    exception::{
        arch_exception::{ EsrEL1, ExceptionContext, SpsrEL1 },
        asynchronous::{ is_local_irq_masked, local_irq_mask_save, local_irq_restore, print_state },
    },
    info,
    memory,
    scheduler::{ CURRENT, RUNNING },
    time::time_manager,
    synchronization::interface::Mutex,
    debug,
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
            p = memory::heap_alloc
                ::kernel_heap_allocator()
                .alloc(Layout::from_size_align(8192, 4096).unwrap());
        }

        let mut ptr = p as u64;
        ptr += 8192;

        let spsr_el1_init = 0x364;
        //USermode:
        //spsr_el1_init &= 0xFFF8;
        ExceptionContext {
            gpr: [0; 30],
            lr: entry_point,
            elr_el1: entry_point,
            spsr_el1: spsr_el1_init,
            esr_el1: 0,
            _res_sp: 0,
            sp_el0: ptr,
        }
    }
}

impl fmt::Display for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PID={}", self.pid)
    }
}
extern "C" {
    fn __switch_to(current: &mut ExceptionContext, next: &mut ExceptionContext);
}

pub fn thread() {
    let mut c: i32 = 0;
    loop {
        let core: usize = core_id();
        let my_pid = CURRENT[core].lock(|c| c);
        info!("Hello from thread with PID={}! C={} @Core{}", my_pid.unwrap(), c, core);
        /*  debug!("\tSPSel={}", aarch64_cpu::registers::SPSel.get());
        debug!("\tSP={:#x}", aarch64_cpu::registers::SP.get()); */
        c += 1;
        yield_cpu();
        time_manager().spin_for(Duration::from_millis(1000));
    }
}

pub fn yield_cpu() {
    let core: usize = core_id();

    let next_thread = RUNNING[core].next().expect("No next thread found!");
    //debug!("[THREAD] Switching to thread {}...", next_thread.get_pid());

    CURRENT[core].lock(|cur| {
        let _my_thread = RUNNING[core].get_by_pid(cur.unwrap()).unwrap();
        *cur = Some(next_thread.get_pid());
        let int_not_masked = !is_local_irq_masked();
        //TODO: give abstraction to SPSR_EL1
        if int_not_masked {
            _my_thread.get_ex_context().spsr_el1 |= 0x80;
        } else {
            _my_thread.get_ex_context().spsr_el1 &= 0b11111111111111111111111101111111;
        }
        unsafe { __switch_to(_my_thread.get_ex_context(), next_thread.get_ex_context()) }
    });
}

pub fn wait_thread() {
    wait_forever();
}