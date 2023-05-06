// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2020-2022 Andre Richter <andre.o.richter@gmail.com>

//! Memory Management Unit.

#[path = "../aarch64/mmu.rs"]
pub mod arch_mmu;

mod mapping_record;
mod page_alloc;
mod translation_table;

pub mod types;

use crate::{
    memory,
    memory::{Address, Physical, Virtual},
    synchronization::{self, interface::Mutex},
};
use core::{fmt, num::NonZeroUsize};

pub use types::*;

use crate::{
    memory::mmu::{self as generic_mmu, AttributeFields, MemoryRegion, PageAddress},
    synchronization::InitStateLock,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------
pub struct TranslationGranule<const GRANULE_SIZE: usize>;

pub struct AddressSpace<const AS_SIZE: usize>;
/// Intended to be implemented for [`AddressSpace`].
pub trait AssociatedTranslationTable {
    /// A translation table whose address range is:
    ///
    /// [u64::MAX, (u64::MAX - AS_SIZE) + 1]
    type TableStartFromTop;

    /// A translation table whose address range is:
    ///
    /// [AS_SIZE - 1, 0]
    type TableStartFromBottom;
}

type KernelTranslationTable =
    <KernelVirtAddrSpace as AssociatedTranslationTable>::TableStartFromTop;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// The translation granule chosen by this BSP. This will be used everywhere else in the kernel to
/// derive respective data structures and their sizes. For example, the `crate::memory::mmu::Page`.
pub type KernelGranule = TranslationGranule<{ 64 * 1024 }>;

/// The kernel's virtual address space defined by this BSP.
pub type KernelVirtAddrSpace = AddressSpace<{ kernel_virt_addr_space_size() }>;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

/// The kernel translation tables.
///
/// It is mandatory that InitStateLock is transparent.
///
/// That is, `size_of(InitStateLock<KernelTranslationTable>) == size_of(KernelTranslationTable)`.
/// There is a unit tests that checks this porperty.
#[link_section = ".data"]
#[no_mangle]
static KERNEL_TABLES: InitStateLock<KernelTranslationTable> =
    InitStateLock::new(KernelTranslationTable::new_for_precompute());

/// This value is needed during early boot for MMU setup.
///
/// This will be patched to the correct value by the "translation table tool" after linking. This
/// given value here is just a dummy.
#[link_section = ".text._start_arguments"]
#[no_mangle]
static PHYS_KERNEL_TABLES_BASE_ADDR: u64 = 0xCCCCAAAAFFFFEEEE;

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// This is a hack for retrieving the value for the kernel's virtual address space size as a
/// constant from a common place, since it is needed as a compile-time/link-time constant in both,
/// the linker script and the Rust sources.
#[allow(clippy::needless_late_init)]
const fn kernel_virt_addr_space_size() -> usize {
    let __kernel_virt_addr_space_size;

    include!("../kernel_virt_addr_space_size.ld");

    __kernel_virt_addr_space_size
}

/// Helper function for calculating the number of pages the given parameter spans.
const fn size_to_num_pages(size: usize) -> usize {
    assert!(size > 0);
    assert!(size % KernelGranule::SIZE == 0);

    size >> KernelGranule::SHIFT
}

/// The data pages of the kernel binary.
fn virt_data_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::data_size());

    let start_page_addr = super::virt_data_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

// There is no reason to expect the following conversions to fail, since they were generated offline
// by the `translation table tool`. If it doesn't work, a panic due to the unwraps is justified.
fn kernel_virt_to_phys_region(virt_region: MemoryRegion<Virtual>) -> MemoryRegion<Physical> {
    let phys_start_page_addr =
        generic_mmu::try_kernel_virt_page_addr_to_phys_page_addr(virt_region.start_page_addr())
            .unwrap();

    let phys_end_exclusive_page_addr = phys_start_page_addr
        .checked_offset(virt_region.num_pages() as isize)
        .unwrap();

    MemoryRegion::new(phys_start_page_addr, phys_end_exclusive_page_addr)
}

fn kernel_page_attributes(virt_page_addr: PageAddress<Virtual>) -> AttributeFields {
    generic_mmu::try_kernel_page_attributes(virt_page_addr).unwrap()
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// The code pages of the kernel binary.
pub fn virt_code_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::code_size());

    let start_page_addr = super::virt_code_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// The heap pages.
pub fn virt_heap_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::heap_size());

    let start_page_addr = super::virt_heap_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// The boot core stack pages.
pub fn virt_boot_core_stack_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::boot_core_stack_size());

    let start_page_addr = super::virt_boot_core_stack_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// Return a reference to the kernel's translation tables.
pub fn kernel_translation_tables() -> &'static InitStateLock<KernelTranslationTable> {
    &KERNEL_TABLES
}

/// The MMIO remap pages.
pub fn virt_mmio_remap_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::mmio_remap_size());

    let start_page_addr = super::virt_mmio_remap_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// Add mapping records for the kernel binary.
///
/// The actual translation table entries for the kernel binary are generated using the offline
/// `translation table tool` and patched into the kernel binary. This function just adds the mapping
/// record entries.
pub fn kernel_add_mapping_records_for_precomputed() {
    let virt_code_region = virt_code_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel code and RO data",
        &virt_code_region,
        &kernel_virt_to_phys_region(virt_code_region),
        &kernel_page_attributes(virt_code_region.start_page_addr()),
    );

    let virt_data_region = virt_data_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel data and bss",
        &virt_data_region,
        &kernel_virt_to_phys_region(virt_data_region),
        &kernel_page_attributes(virt_data_region.start_page_addr()),
    );

    let virt_heap_region = virt_heap_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel heap",
        &virt_heap_region,
        &kernel_virt_to_phys_region(virt_heap_region),
        &kernel_page_attributes(virt_heap_region.start_page_addr()),
    );

    let virt_boot_core_stack_region = virt_boot_core_stack_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel boot-core stack",
        &virt_boot_core_stack_region,
        &kernel_virt_to_phys_region(virt_boot_core_stack_region),
        &kernel_page_attributes(virt_boot_core_stack_region.start_page_addr()),
    );
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// MMU enable errors variants.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum MMUEnableError {
    AlreadyEnabled,
    Other(&'static str),
}

/// Memory Management interfaces.
pub mod interface {
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
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------
use interface::MMU;
use synchronization::interface::ReadWriteEx;
use translation_table::interface::TranslationTable;

/// Map a region in the kernel's translation tables.
///
/// No input checks done, input is passed through to the architectural implementation.
///
/// # Safety
///
/// - See `map_at()`.
/// - Does not prevent aliasing.
unsafe fn kernel_map_at_unchecked(
    name: &'static str,
    virt_region: &MemoryRegion<Virtual>,
    phys_region: &MemoryRegion<Physical>,
    attr: &AttributeFields,
) -> Result<(), &'static str> {
    memory::mmu::kernel_translation_tables()
        .write(|tables| tables.map_at(virt_region, phys_region, attr))?;

    kernel_add_mapping_record(name, virt_region, phys_region, attr);

    Ok(())
}

/// Try to translate a kernel virtual address to a physical address.
///
/// Will only succeed if there exists a valid mapping for the input address.
fn try_kernel_virt_addr_to_phys_addr(
    virt_addr: Address<Virtual>,
) -> Result<Address<Physical>, &'static str> {
    memory::mmu::kernel_translation_tables()
        .read(|tables| tables.try_virt_addr_to_phys_addr(virt_addr))
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl fmt::Display for MMUEnableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MMUEnableError::AlreadyEnabled => write!(f, "MMU is already enabled"),
            MMUEnableError::Other(x) => write!(f, "{}", x),
        }
    }
}

impl<const GRANULE_SIZE: usize> TranslationGranule<GRANULE_SIZE> {
    /// The granule's size.
    pub const SIZE: usize = Self::size_checked();

    /// The granule's mask.
    pub const MASK: usize = Self::SIZE - 1;

    /// The granule's shift, aka log2(size).
    pub const SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(GRANULE_SIZE.is_power_of_two());

        GRANULE_SIZE
    }
}

impl<const AS_SIZE: usize> AddressSpace<AS_SIZE> {
    /// The address space size.
    pub const SIZE: usize = Self::size_checked();

    /// The address space shift, aka log2(size).
    pub const SIZE_SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(AS_SIZE.is_power_of_two());

        // Check for architectural restrictions as well.
        Self::arch_address_space_size_sanity_checks();

        AS_SIZE
    }
}

/// Query the BSP for the reserved virtual addresses for MMIO remapping and initialize the kernel's
/// MMIO VA allocator with it.
pub fn kernel_init_mmio_va_allocator() {
    let region = memory::mmu::virt_mmio_remap_region();

    page_alloc::kernel_mmio_va_allocator().lock(|allocator| allocator.init(region));
}

/// Add an entry to the mapping info record.
pub fn kernel_add_mapping_record(
    name: &'static str,
    virt_region: &MemoryRegion<Virtual>,
    phys_region: &MemoryRegion<Physical>,
    attr: &AttributeFields,
) {
    mapping_record::kernel_add(name, virt_region, phys_region, attr);
}

/// MMIO remapping in the kernel translation tables.
///
/// Typically used by device drivers.
///
/// # Safety
///
/// - Same as `kernel_map_at_unchecked()`, minus the aliasing part.
pub unsafe fn kernel_map_mmio(
    name: &'static str,
    mmio_descriptor: &MMIODescriptor,
) -> Result<Address<Virtual>, &'static str> {
    let phys_region = MemoryRegion::from(*mmio_descriptor);
    let offset_into_start_page = mmio_descriptor.start_addr().offset_into_page();

    // Check if an identical region has been mapped for another driver. If so, reuse it.
    let virt_addr = if let Some(addr) =
        mapping_record::kernel_find_and_insert_mmio_duplicate(mmio_descriptor, name)
    {
        addr
    // Otherwise, allocate a new region and map it.
    } else {
        let num_pages = match NonZeroUsize::new(phys_region.num_pages()) {
            None => return Err("Requested 0 pages"),
            Some(x) => x,
        };

        let virt_region =
            page_alloc::kernel_mmio_va_allocator().lock(|allocator| allocator.alloc(num_pages))?;

        kernel_map_at_unchecked(
            name,
            &virt_region,
            &phys_region,
            &AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        )?;

        virt_region.start_addr()
    };

    Ok(virt_addr + offset_into_start_page)
}

/// Try to translate a kernel virtual page address to a physical page address.
///
/// Will only succeed if there exists a valid mapping for the input page.
pub fn try_kernel_virt_page_addr_to_phys_page_addr(
    virt_page_addr: PageAddress<Virtual>,
) -> Result<PageAddress<Physical>, &'static str> {
    memory::mmu::kernel_translation_tables()
        .read(|tables| tables.try_virt_page_addr_to_phys_page_addr(virt_page_addr))
}

/// Try to get the attributes of a kernel page.
///
/// Will only succeed if there exists a valid mapping for the input page.
pub fn try_kernel_page_attributes(
    virt_page_addr: PageAddress<Virtual>,
) -> Result<AttributeFields, &'static str> {
    memory::mmu::kernel_translation_tables()
        .read(|tables| tables.try_page_attributes(virt_page_addr))
}

/// Human-readable print of all recorded kernel mappings.
pub fn kernel_print_mappings() {
    mapping_record::kernel_print()
}

/// Enable the MMU and data + instruction caching.
///
/// # Safety
///
/// - Crucial function during kernel init. Changes the the complete memory view of the processor.
#[inline(always)]
pub unsafe fn enable_mmu_and_caching(
    phys_tables_base_addr: Address<Physical>,
) -> Result<(), MMUEnableError> {
    arch_mmu::mmu().enable_mmu_and_caching(phys_tables_base_addr)
}
