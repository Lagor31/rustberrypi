use core::{
    alloc::{ GlobalAlloc, Layout, LayoutError },
    mem,
    sync::atomic::{ AtomicU64, Ordering },
    time::Duration,
    fmt,
    ptr::addr_of_mut,
};

use aarch64_cpu::registers::{ DAIF, ESR_EL1, SPSR_EL1 };
use rand::{ rngs::SmallRng, SeedableRng, RngCore };
use tock_registers::{ interfaces::{ Readable, Writeable }, registers::InMemoryRegister };

use crate::{
    cpu::{ wait_forever, core_id },
    exception::{
        arch_exception::{ EsrEL1, ExceptionContext, SpsrEL1 },
        asynchronous::{ is_local_irq_masked, local_irq_mask_save, local_irq_restore, print_state },
    },
    info,
    memory::{ self, heap_alloc::kernel_heap_allocator },
    scheduler::{ CURRENT, RUNNING, SLEEPING },
    time::time_manager,
    synchronization::interface::Mutex,
    debug,
    random,
};

pub struct Thread {
    pid: u64,
    context: ExceptionContext,
    original_stack: usize,
}

static PID: AtomicU64 = AtomicU64::new(1);

const STACK_SIZE: usize = 8192;
const STACK_ALIGN: usize = 4096;

impl Thread {
    pub fn new(entry_point: u64) -> Self {
        let (c, stack) = Self::make_context(entry_point);
        let out = Thread {
            pid: PID.fetch_add(1, Ordering::Acquire),
            context: c,
            original_stack: stack as usize,
        };
        out
    }

    pub fn get_ex_context(&mut self) -> &mut ExceptionContext {
        &mut self.context
    }

    pub fn get_pid(&self) -> u64 {
        self.pid
    }

    fn make_context(entry_point: u64) -> (ExceptionContext, *mut u8) {
        let stack_pointer_low_end;
        unsafe {
            stack_pointer_low_end = memory::heap_alloc
                ::kernel_heap_allocator()
                .alloc(Layout::from_size_align(STACK_SIZE, STACK_ALIGN).unwrap());
        }

        let mut sp_value = stack_pointer_low_end as u64;
        sp_value += 8192;

        let spsr_el1_init = 0x364;
        //USermode:
        //spsr_el1_init &= 0xFFF8;
        (
            ExceptionContext {
                gpr: [0; 30],
                lr: entry_point,
                elr_el1: entry_point,
                spsr_el1: spsr_el1_init,
                esr_el1: 0,
                _res_sp: 0,
                sp_el0: sp_value,
            },
            stack_pointer_low_end,
        )
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        //info!("Deallocating thread {}", self.pid);
        unsafe {
            kernel_heap_allocator().dealloc(
                self.original_stack as *mut u8,
                Layout::from_size_align(STACK_SIZE, STACK_ALIGN).unwrap()
            )
        }
    }
}

impl fmt::Display for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PID={}", self.pid)
    }
}
extern "C" {
    pub fn __switch_to(current: &mut ExceptionContext, next: &mut ExceptionContext);
}

pub fn thread() {
    let mut c: u64 = 0;
    let stop_me: u64 = (random::next_u64() % 20) + 1;
    loop {
        let core: usize = core_id();
        let my_pid = CURRENT[core].lock(|c| c);
        info!("Hello from thread with PID={}! C={} @Core{}", my_pid.unwrap(), c, core);
        /*  debug!("\tSPSel={}", aarch64_cpu::registers::SPSel.get());
        debug!("\tSP={:#x}", aarch64_cpu::registers::SP.get()); */
        c += 1;
        if c > stop_me {
            info!(
                "Thread with PID={}! C={} @Core{} is going to sleep...",
                my_pid.unwrap(),
                c,
                core
            );
            sleep();
        }
        reschedule();
        time_manager().spin_for(Duration::from_millis(500));
    }
}

pub fn wait_thread() {
    wait_forever();
}

pub fn sleep() {
    let core: usize = core_id();

    CURRENT[core].lock(|cur| {
        let mut _my_thread = RUNNING[core].remove(cur.unwrap()).unwrap();
        let next_thread = RUNNING[core].next().expect("No next thread found!");
        //debug!("[SLEEP] Switching to thread {}...", next_thread.get_pid());
        let int_not_masked = !is_local_irq_masked();
        //TODO: give abstraction to SPSR_EL1
        if int_not_masked {
            _my_thread.get_ex_context().spsr_el1 |= 0x80;
        } else {
            _my_thread.get_ex_context().spsr_el1 &= 0b11111111111111111111111101111111;
        }
        SLEEPING.add(_my_thread);
        let _my_thread = SLEEPING.get_by_pid(cur.unwrap()).unwrap_or_else(||
            panic!("Cannot find PID={} in SLEEPING[{}]", cur.unwrap(), core)
        );
        *cur = Some(next_thread.get_pid());
        unsafe { __switch_to(_my_thread.get_ex_context(), next_thread.get_ex_context()) }
    });
}

pub fn reschedule() {
    let core: usize = core_id();

    let next_thread = RUNNING[core].next().expect("No next thread found!");
    //debug!("[RESCHEDULE] Switching to thread {}...", next_thread.get_pid());

    CURRENT[core].lock(|cur| {
        let _my_thread = RUNNING[core].get_by_pid(cur.unwrap()).unwrap_or_else(||
            panic!("Cannot find PID={} in RUNNING[{}]", cur.unwrap(), core)
        );
        let int_not_masked = !is_local_irq_masked();
        //TODO: give abstraction to SPSR_EL1
        if int_not_masked {
            _my_thread.get_ex_context().spsr_el1 |= 0x80;
        } else {
            _my_thread.get_ex_context().spsr_el1 &= 0b11111111111111111111111101111111;
        }
        *cur = Some(next_thread.get_pid());
        unsafe { __switch_to(_my_thread.get_ex_context(), next_thread.get_ex_context()) }
    });
}