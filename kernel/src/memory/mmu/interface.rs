use super::*;

/// MMU functions.
pub trait MMU {
    /// Turns on the MMU for the first time and enables data and instruction caching.
    ///
    /// # Safety
    ///
    /// - Changes the HW's global state.
    unsafe fn enable_mmu_and_caching(
        &self,
        phys_tables_base_addr: Address<Physical>,
    ) -> Result<(), MMUEnableError>;

    /// Returns true if the MMU is enabled, false otherwise.
    fn is_enabled(&self) -> bool;
}
