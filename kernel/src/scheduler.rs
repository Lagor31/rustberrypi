use core::{ cell::UnsafeCell, borrow::BorrowMut };

use alloc::collections::LinkedList;
use alloc::collections::linked_list::Iter;
use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;

// Create small, cheap to initialize and fast RNG with a random seed.
// The randomness is supplied by the operating system.
use alloc::{
    collections::{ BTreeMap, btree_map::{ IterMut, ValuesMut } },
    borrow::ToOwned,
    boxed::Box,
};
use spin::{ mutex::SpinMutex, rwlock::RwLock };

use crate::synchronization::IRQSafeNullLock;
use crate::time::time_manager;
use crate::{ thread::Thread, synchronization::{ SpinLock, interface::Mutex } };

type ThreadMap = LinkedList<Thread>;

pub struct ThreadQueue {
    tm: SpinLock<ThreadMap>,
}

impl ThreadQueue {
    const fn new() -> Self {
        Self {
            tm: SpinLock::new(ThreadMap::new()),
        }
    }

    pub fn next(&self) -> Option<&mut Thread> {
        self.tm.lock(|m| {
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
        self.tm.lock(|map| map.push_back(t));
    }

    pub fn pop(&self) -> Thread {
        self.tm.lock(|map| map.pop_front().unwrap())
    }

    pub fn get_by_pid(&self, pid: u64) -> Option<&mut Thread> {
        self.tm.lock(|m| {
            for p in m.iter_mut() {
                if p.get_pid() == pid {
                    return Some(p);
                }
            }
            Option::None
        })
    }

    pub fn iter(&self) -> Iter<'_, Thread> {
        self.tm.lock(|map| map.iter())
    }
}

pub static CURRENT: IRQSafeNullLock<Option<u64>> = IRQSafeNullLock::new(Option::None);
pub static RUNNING: ThreadQueue = ThreadQueue::new();