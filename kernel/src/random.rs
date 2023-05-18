use rand::{ rngs::SmallRng, SeedableRng, RngCore };
use crate::{ time::time_manager, synchronization::{ SpinLock, interface::Mutex } };

pub fn next_u64() -> u64 {
    SmallRng::seed_from_u64(time_manager().uptime().as_millis() as u64).next_u64()
}