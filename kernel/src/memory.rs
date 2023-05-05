// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! Memory Management.
pub mod heap_alloc;
pub mod map;
pub mod mmu;
use crate::{common, memory};
use core::{
    fmt,
    marker::PhantomData,
    ops::{Add, Sub},
};

use core::cell::UnsafeCell;

use self::mmu::types::PageAddress;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

// Symbols from the linker script.
extern "Rust" {
    static __code_start: UnsafeCell<()>;
    static __code_end_exclusive: UnsafeCell<()>;

    static __data_start: UnsafeCell<()>;
    static __data_end_exclusive: UnsafeCell<()>;

    static __heap_start: UnsafeCell<()>;
    static __heap_end_exclusive: UnsafeCell<()>;

    static __mmio_remap_start: UnsafeCell<()>;
    static __mmio_remap_end_exclusive: UnsafeCell<()>;

    static __boot_core_stack_start: UnsafeCell<()>;
    static __boot_core_stack_end_exclusive: UnsafeCell<()>;
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// Start page address of the code segment.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn virt_code_start() -> PageAddress<Virtual> {
    PageAddress::from(unsafe { __code_start.get() as usize })
}

/// Size of the code segment.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn code_size() -> usize {
    unsafe { (__code_end_exclusive.get() as usize) - (__code_start.get() as usize) }
}

/// Start page address of the data segment.
#[inline(always)]
fn virt_data_start() -> PageAddress<Virtual> {
    PageAddress::from(unsafe { __data_start.get() as usize })
}

/// Size of the data segment.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn data_size() -> usize {
    unsafe { (__data_end_exclusive.get() as usize) - (__data_start.get() as usize) }
}

/// Start page address of the heap segment.
#[inline(always)]
fn virt_heap_start() -> PageAddress<Virtual> {
    PageAddress::from(unsafe { __heap_start.get() as usize })
}

/// Size of the heap segment.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn heap_size() -> usize {
    unsafe { (__heap_end_exclusive.get() as usize) - (__heap_start.get() as usize) }
}

/// Start page address of the MMIO remap reservation.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn virt_mmio_remap_start() -> PageAddress<Virtual> {
    PageAddress::from(unsafe { __mmio_remap_start.get() as usize })
}

/// Size of the MMIO remap reservation.
///
/// # Safety
///
/// - Value is provided by the linker script and must be trusted as-is.
#[inline(always)]
fn mmio_remap_size() -> usize {
    unsafe { (__mmio_remap_end_exclusive.get() as usize) - (__mmio_remap_start.get() as usize) }
}

/// Start page address of the boot core's stack.
#[inline(always)]
fn virt_boot_core_stack_start() -> PageAddress<Virtual> {
    PageAddress::from(unsafe { __boot_core_stack_start.get() as usize })
}

/// Size of the boot core's stack.
#[inline(always)]
fn boot_core_stack_size() -> usize {
    unsafe {
        (__boot_core_stack_end_exclusive.get() as usize) - (__boot_core_stack_start.get() as usize)
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Exclusive end address of the physical address space.
#[inline(always)]
pub fn phys_addr_space_end_exclusive_addr() -> PageAddress<Physical> {
    PageAddress::from(crate::memory::map::END)
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Metadata trait for marking the type of an address.
pub trait AddressType: Copy + Clone + PartialOrd + PartialEq + Ord + Eq {}

/// Zero-sized type to mark a physical address.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
pub enum Physical {}

/// Zero-sized type to mark a virtual address.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
pub enum Virtual {}

/// Generic address type.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Address<ATYPE: AddressType> {
    value: usize,
    _address_type: PhantomData<fn() -> ATYPE>,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl AddressType for Physical {}
impl AddressType for Virtual {}

impl<ATYPE: AddressType> Address<ATYPE> {
    /// Create an instance.
    pub const fn new(value: usize) -> Self {
        Self {
            value,
            _address_type: PhantomData,
        }
    }

    /// Convert to usize.
    pub const fn as_usize(self) -> usize {
        self.value
    }

    /// Align down to page size.
    #[must_use]
    pub const fn align_down_page(self) -> Self {
        let aligned = common::align_down(self.value, memory::mmu::KernelGranule::SIZE);

        Self::new(aligned)
    }

    /// Align up to page size.
    #[must_use]
    pub const fn align_up_page(self) -> Self {
        let aligned = common::align_up(self.value, memory::mmu::KernelGranule::SIZE);

        Self::new(aligned)
    }

    /// Checks if the address is page aligned.
    pub const fn is_page_aligned(&self) -> bool {
        common::is_aligned(self.value, memory::mmu::KernelGranule::SIZE)
    }

    /// Return the address' offset into the corresponding page.
    pub const fn offset_into_page(&self) -> usize {
        self.value & memory::mmu::KernelGranule::MASK
    }
}

impl<ATYPE: AddressType> Add<usize> for Address<ATYPE> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: usize) -> Self::Output {
        match self.value.checked_add(rhs) {
            None => panic!("Overflow on Address::add"),
            Some(x) => Self::new(x),
        }
    }
}

impl<ATYPE: AddressType> Sub<usize> for Address<ATYPE> {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: usize) -> Self::Output {
        match self.value.checked_sub(rhs) {
            None => panic!("Overflow on Address::sub"),
            Some(x) => Self::new(x),
        }
    }
}

impl<ATYPE: AddressType> Sub<Address<ATYPE>> for Address<ATYPE> {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Address<ATYPE>) -> Self::Output {
        match self.value.checked_sub(rhs.value) {
            None => panic!("Overflow on Address::sub"),
            Some(x) => Self::new(x),
        }
    }
}

impl Address<Virtual> {
    /// Checks if the address is part of the boot core stack region.
    pub fn is_valid_stack_addr(&self) -> bool {
        memory::mmu::virt_boot_core_stack_region().contains(*self)
    }

    /// Checks if the address is part of the kernel code region.
    pub fn is_valid_code_addr(&self) -> bool {
        memory::mmu::virt_code_region().contains(*self)
    }
}

impl fmt::Display for Address<Physical> {
    // Don't expect to see physical addresses greater than 40 bit.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let q3: u8 = ((self.value >> 32) & 0xff) as u8;
        let q2: u16 = ((self.value >> 16) & 0xffff) as u16;
        let q1: u16 = (self.value & 0xffff) as u16;

        write!(f, "0x")?;
        write!(f, "{:02x}_", q3)?;
        write!(f, "{:04x}_", q2)?;
        write!(f, "{:04x}", q1)
    }
}

impl fmt::Display for Address<Virtual> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let q4: u16 = ((self.value >> 48) & 0xffff) as u16;
        let q3: u16 = ((self.value >> 32) & 0xffff) as u16;
        let q2: u16 = ((self.value >> 16) & 0xffff) as u16;
        let q1: u16 = (self.value & 0xffff) as u16;

        write!(f, "0x")?;
        write!(f, "{:04x}_", q4)?;
        write!(f, "{:04x}_", q3)?;
        write!(f, "{:04x}_", q2)?;
        write!(f, "{:04x}", q1)
    }
}

/// Initialize the memory subsystem.
pub fn init() {
    mmu::kernel_init_mmio_va_allocator();
    heap_alloc::kernel_init_heap_allocator();
}

//--------------------------------------------------------------------------------------------------
// Testing
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use test_macros::kernel_test;

    /// Sanity of [Address] methods.
    #[kernel_test]
    fn address_type_method_sanity() {
        let addr = Address::<Virtual>::new(drivers::memory::mmu::KernelGranule::SIZE + 100);

        assert_eq!(
            addr.align_down_page().as_usize(),
            drivers::memory::mmu::KernelGranule::SIZE
        );

        assert_eq!(
            addr.align_up_page().as_usize(),
            drivers::memory::mmu::KernelGranule::SIZE * 2
        );

        assert!(!addr.is_page_aligned());

        assert_eq!(addr.offset_into_page(), 100);
    }
}
