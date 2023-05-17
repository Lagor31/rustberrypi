use core::{ borrow::BorrowMut, cell::UnsafeCell };

use alloc::collections::linked_list::Iter;
use alloc::collections::LinkedList;
use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;

// Create small, cheap to initialize and fast RNG with a random seed.
// The randomness is supplied by the operating system.
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    collections::{ btree_map::{ IterMut, ValuesMut }, BTreeMap },
};
use spin::{ mutex::SpinMutex, rwlock::RwLock };

use crate::exception::arch_exception::ExceptionContext;
use crate::synchronization::IRQSafeLock;
use crate::time::time_manager;
use crate::{ synchronization::{ interface::Mutex, SpinLock }, thread::Thread };

type ThreadMap = IRQSafeLock<LinkedList<Thread>>;

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

pub static mut CURRENT: Option<u64> = Option::None;
pub static RUNNING: ThreadQueue = ThreadQueue::new();

pub fn store_context(s: &mut ExceptionContext, d: &mut ExceptionContext) {
    d.elr_el1 = s.elr_el1;
    d.esr_el1 = s.esr_el1;
    d.gpr = s.gpr;
    d.lr = s.lr;
    d.sp_el0 = s.sp_el0;
    d.spsr_el1 = s.spsr_el1;
}