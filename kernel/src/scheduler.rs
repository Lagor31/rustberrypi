use core::fmt;
use core::sync::atomic::{ AtomicU64, Ordering };
use core::{ borrow::BorrowMut, cell::UnsafeCell };

use alloc::collections::linked_list::Iter;
use alloc::collections::LinkedList;
use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;
use core::sync::atomic;
// Create small, cheap to initialize and fast RNG with a random seed.
// The randomness is supplied by the operating system.
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    collections::{ btree_map::{ IterMut, ValuesMut }, BTreeMap },
};
use spin::{ mutex::SpinMutex, rwlock::RwLock };

use crate::cpu::core_id;
use crate::exception::arch_exception::ExceptionContext;
use crate::info;
use crate::synchronization::IRQSafeLock;
use crate::time::time_manager;
use crate::{ synchronization::{ interface::Mutex, SpinLock }, thread::Thread };

pub static CURRENT: [IRQSafeLock<Option<u64>>; 4] = [
    IRQSafeLock::new(Option::None), //CORE0
    IRQSafeLock::new(Option::None), //CORE1
    IRQSafeLock::new(Option::None), //CORE2
    IRQSafeLock::new(Option::None), //CORE3
];

pub static RUNNING: [ThreadQueue; 4] = [
    ThreadQueue::new(), //CORE0
    ThreadQueue::new(), //CORE1
    ThreadQueue::new(), //CORE2
    ThreadQueue::new(), //CORE3
];

pub static SLEEPING: ThreadQueue = ThreadQueue::new();

pub struct ThreadQueue {
    irq_lock: IRQSafeLock<SpinLock<LinkedList<Thread>>>,
}

impl ThreadQueue {
    const fn new() -> Self {
        Self {
            irq_lock: IRQSafeLock::new(SpinLock::new(LinkedList::new())),
        }
    }

    pub fn next(&self) -> Option<&mut Thread> {
        self.irq_lock.lock(|spin_lock| {
            spin_lock.lock(|threads| {
                let mut r = SmallRng::seed_from_u64(time_manager().uptime().as_millis() as u64);
                let len = threads.len();
                let r = (r.next_u64() as usize) % len;
                for (t, p) in threads.iter_mut().enumerate() {
                    if t == r {
                        return Some(p);
                    }
                }
                Option::None
            })
        })
    }

    pub fn add(&self, t: Thread) {
        self.irq_lock.lock(|spin_lock| {
            spin_lock.lock(|threads| {
                threads.push_back(t);
            })
        })
    }

    pub fn pop(&self) -> Thread {
        self.irq_lock.lock(|spin_lock| {
            spin_lock.lock(|threads| { threads.pop_front().unwrap() })
        })
    }

    pub fn remove(&self, pid: u64) -> Option<Thread> {
        self.irq_lock.lock(|spin_lock| {
            spin_lock.lock(|threads| {
                let mut pos: Option<usize> = None;
                for (i, t) in threads.iter().enumerate() {
                    if t.get_pid() == pid {
                        pos = Some(i);
                    }
                }
                if pos.is_some() {
                    Some(threads.remove(pos.unwrap()))
                } else {
                    Option::None
                }
            })
        })
    }

    pub fn get_by_pid(&self, pid: u64) -> Option<&mut Thread> {
        self.irq_lock.lock(|spin_lock| {
            spin_lock.lock(|threads| {
                for p in threads.iter_mut() {
                    if p.get_pid() == pid {
                        return Some(p);
                    }
                }
                Option::None
            })
        })
    }
}

impl fmt::Display for ThreadQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.irq_lock.lock(|spinlock| {
            spinlock.lock(|threads| {
                for t in threads {
                    writeln!(f, "{}", t)?;
                }
                write!(f, "")
            })
        })
    }
}

fn store_context(s: &mut ExceptionContext, d: &mut ExceptionContext) {
    d.elr_el1 = s.elr_el1;
    d.esr_el1 = s.esr_el1;
    d.gpr = s.gpr;
    d.lr = s.lr;
    d.sp_el0 = s.sp_el0;
    d.spsr_el1 = s.spsr_el1;
}

pub fn reschedule_from_context(_ec: &mut ExceptionContext) {
    let core: usize = core_id();
    CURRENT[core].lock(|cur_pid| {
        if cur_pid.is_some() {
            let _cur_thread = RUNNING[core].get_by_pid(cur_pid.unwrap()).unwrap_or_else(||
                panic!("[IRQ] Cannot find PID={} in RUNNING[{}]", cur_pid.unwrap(), core)
            );
            store_context(_ec, _cur_thread.get_ex_context());
        } else {
            info!("Current = None");
        }

        let next_thread: &mut Thread = RUNNING[core].next().expect("No next thread found!");

        *cur_pid = Some(next_thread.get_pid());
        store_context(next_thread.get_ex_context(), _ec);
    })
}