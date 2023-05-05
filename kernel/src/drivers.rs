// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! Conditional reexporting of Board Support Packages.

pub mod gicv2;

pub use gicv2::*;

mod bcm2xxx_gpio;
mod bcm2xxx_pl011_uart;

pub use bcm2xxx_gpio::*;
pub use bcm2xxx_pl011_uart::*;

mod common;

use super::{exception, memory::map::mmio};
use crate::{
    console, driver as generic_driver,
    drivers::{GICv2, PL011Uart},
    exception::{self as generic_exception},
    memory,
    memory::mmu::MMIODescriptor,
};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static mut PL011_UART: MaybeUninit<PL011Uart> = MaybeUninit::uninit();
static mut GPIO: MaybeUninit<GPIO> = MaybeUninit::uninit();

static mut INTERRUPT_CONTROLLER: MaybeUninit<GICv2> = MaybeUninit::uninit();

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_uart() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(mmio::PL011_UART_START, mmio::PL011_UART_SIZE);
    let virt_addr = memory::mmu::kernel_map_mmio(PL011Uart::COMPATIBLE, &mmio_descriptor)?;

    PL011_UART.write(PL011Uart::new(virt_addr));

    Ok(())
}

/// This must be called only after successful init of the UART driver.
unsafe fn post_init_uart() -> Result<(), &'static str> {
    console::register_console(PL011_UART.assume_init_ref());

    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_gpio() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(mmio::GPIO_START, mmio::GPIO_SIZE);
    let virt_addr = memory::mmu::kernel_map_mmio(GPIO::COMPATIBLE, &mmio_descriptor)?;

    GPIO.write(GPIO::new(virt_addr));

    Ok(())
}

/// This must be called only after successful init of the GPIO driver.
unsafe fn post_init_gpio() -> Result<(), &'static str> {
    GPIO.assume_init_ref().map_pl011_uart();
    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_interrupt_controller() -> Result<(), &'static str> {
    let gicd_mmio_descriptor = MMIODescriptor::new(mmio::GICD_START, mmio::GICD_SIZE);
    let gicd_virt_addr = memory::mmu::kernel_map_mmio("GICv2 GICD", &gicd_mmio_descriptor)?;

    let gicc_mmio_descriptor = MMIODescriptor::new(mmio::GICC_START, mmio::GICC_SIZE);
    let gicc_virt_addr = memory::mmu::kernel_map_mmio("GICV2 GICC", &gicc_mmio_descriptor)?;

    INTERRUPT_CONTROLLER.write(GICv2::new(gicd_virt_addr, gicc_virt_addr));

    Ok(())
}

/// This must be called only after successful init of the interrupt controller driver.
unsafe fn post_init_interrupt_controller() -> Result<(), &'static str> {
    generic_exception::asynchronous::register_irq_manager(INTERRUPT_CONTROLLER.assume_init_ref());

    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_uart() -> Result<(), &'static str> {
    instantiate_uart()?;

    let uart_descriptor = generic_driver::DeviceDriverDescriptor::new(
        PL011_UART.assume_init_ref(),
        Some(post_init_uart),
        Some(exception::asynchronous::irq_map::PL011_UART),
    );
    generic_driver::driver_manager().register_driver(uart_descriptor);

    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_gpio() -> Result<(), &'static str> {
    instantiate_gpio()?;

    let gpio_descriptor = generic_driver::DeviceDriverDescriptor::new(
        GPIO.assume_init_ref(),
        Some(post_init_gpio),
        None,
    );
    generic_driver::driver_manager().register_driver(gpio_descriptor);

    Ok(())
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_interrupt_controller() -> Result<(), &'static str> {
    instantiate_interrupt_controller()?;

    let interrupt_controller_descriptor = generic_driver::DeviceDriverDescriptor::new(
        INTERRUPT_CONTROLLER.assume_init_ref(),
        Some(post_init_interrupt_controller),
        None,
    );
    generic_driver::driver_manager().register_driver(interrupt_controller_descriptor);

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Initialize the driver subsystem.
///
/// # Safety
///
/// See child function calls.
pub unsafe fn init() -> Result<(), &'static str> {
    static INIT_DONE: AtomicBool = AtomicBool::new(false);
    if INIT_DONE.load(Ordering::Relaxed) {
        return Err("Init already done");
    }

    driver_uart()?;
    driver_gpio()?;
    driver_interrupt_controller()?;

    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}

/// Doc
pub mod raspberrypi {

    pub mod memory {
        // SPDX-License-Identifier: MIT OR Apache-2.0
        //
        // Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

        //! BSP Memory Management.
        pub mod mmu {
            // SPDX-License-Identifier: MIT OR Apache-2.0
            //
            // Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

            //! BSP Memory Management Unit.

            use crate::{
                memory::{
                    mmu::{
                        self as generic_mmu, AddressSpace, AssociatedTranslationTable,
                        AttributeFields, MemoryRegion, PageAddress, TranslationGranule,
                    },
                    Physical, Virtual,
                },
                synchronization::InitStateLock,
            };

            //--------------------------------------------------------------------------------------------------
            // Private Definitions
            //--------------------------------------------------------------------------------------------------

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

                include!("kernel_virt_addr_space_size.ld");

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
                let end_exclusive_page_addr =
                    start_page_addr.checked_offset(num_pages as isize).unwrap();

                MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
            }

            // There is no reason to expect the following conversions to fail, since they were generated offline
            // by the `translation table tool`. If it doesn't work, a panic due to the unwraps is justified.
            fn kernel_virt_to_phys_region(
                virt_region: MemoryRegion<Virtual>,
            ) -> MemoryRegion<Physical> {
                let phys_start_page_addr =
                    generic_mmu::try_kernel_virt_page_addr_to_phys_page_addr(
                        virt_region.start_page_addr(),
                    )
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
                let end_exclusive_page_addr =
                    start_page_addr.checked_offset(num_pages as isize).unwrap();

                MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
            }

            /// The heap pages.
            pub fn virt_heap_region() -> MemoryRegion<Virtual> {
                let num_pages = size_to_num_pages(super::heap_size());

                let start_page_addr = super::virt_heap_start();
                let end_exclusive_page_addr =
                    start_page_addr.checked_offset(num_pages as isize).unwrap();

                MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
            }

            /// The boot core stack pages.
            pub fn virt_boot_core_stack_region() -> MemoryRegion<Virtual> {
                let num_pages = size_to_num_pages(super::boot_core_stack_size());

                let start_page_addr = super::virt_boot_core_stack_start();
                let end_exclusive_page_addr =
                    start_page_addr.checked_offset(num_pages as isize).unwrap();

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
                let end_exclusive_page_addr =
                    start_page_addr.checked_offset(num_pages as isize).unwrap();

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
        }

        use crate::memory::{mmu::PageAddress, Physical, Virtual};
        use core::cell::UnsafeCell;

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
            unsafe {
                (__mmio_remap_end_exclusive.get() as usize) - (__mmio_remap_start.get() as usize)
            }
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
                (__boot_core_stack_end_exclusive.get() as usize)
                    - (__boot_core_stack_start.get() as usize)
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
    }

    //--------------------------------------------------------------------------------------------------
    // Public Code
    //--------------------------------------------------------------------------------------------------

    /// Board identification.
    pub fn board_name() -> &'static str {
        {
            "Raspberry Pi 4"
        }
    }
}

pub use raspberrypi::*;
