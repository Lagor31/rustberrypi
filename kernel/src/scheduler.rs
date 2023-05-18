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

type ThreadMap = IRQSafeLock<LinkedList<Thread>>;

const NO_CUR: IRQSafeLock<Option<u64>> = IRQSafeLock::new(Option::None);
pub static CURRENT: [IRQSafeLock<Option<u64>>; 4] = [NO_CUR; 4];
pub static RUNNING: ThreadQueue = ThreadQueue::new();

pub struct ThreadQueue {
    tl: IRQSafeLock<LinkedList<Thread>>,
}

impl ThreadQueue {
    const fn new() -> Self {
        Self {
            tl: IRQSafeLock::new(LinkedList::new()),
        }
    }

    pub fn next(&self) -> Option<&mut Thread> {
        self.tl.lock(|m| {
            let mut r = SmallRng::seed_from_u64(time_manager().uptime().as_millis() as u64);
            let len = m.len() - 1;
            let r = (r.next_u64() as usize) % len;
            for (t, p) in m.iter_mut().enumerate() {
                if t == r {
                    return Some(p);
                }
            }
            Option::None
        })
    }

    pub fn add(&self, t: Thread) {
        self.tl.lock(|map| map.push_back(t));
    }

    pub fn pop(&self) -> Thread {
        self.tl.lock(|map| map.pop_front().unwrap())
    }

    pub fn get_by_pid(&self, pid: u64) -> Option<&mut Thread> {
        self.tl.lock(|m| {
            for p in m.iter_mut() {
                if p.get_pid() == pid {
                    return Some(p);
                }
            }
            Option::None
        })
    }

    pub fn iter(&self) -> Iter<'_, Thread> {
        self.tl.lock(|map| map.iter())
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

pub fn reschedule(_ec: &mut ExceptionContext) {
    let core: usize = core_id();
    CURRENT[core].lock(|cur_pid| {
        if cur_pid.is_some() {
            let _cur_thread = RUNNING.get_by_pid(cur_pid.unwrap()).unwrap();
            store_context(_ec, _cur_thread.get_ex_context());
        } else {
            info!("Current = None");
        }

        let next_thread: &mut Thread = RUNNING.next().expect("No next thread found!");

        *cur_pid = Some(next_thread.get_pid());
        store_context(next_thread.get_ex_context(), _ec);
    })
}