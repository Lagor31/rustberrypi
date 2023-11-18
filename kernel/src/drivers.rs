// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! Conditional reexporting of Board Support Packages.

pub mod common;
pub mod gicv2;
pub mod mailbox;
pub mod sgi;

mod bcm2711_gpio;
mod bcm2711_pl011_uart;

pub use gicv2::*;

pub use bcm2711_gpio::*;
pub use bcm2711_pl011_uart::*;

use self::{mailbox::Mailbox, sgi::SGIHandler};

use super::{exception, memory::map::mmio};
use crate::{
    console, driver as generic_driver,
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
static mut SGI_HANDLER: MaybeUninit<SGIHandler> = MaybeUninit::uninit();
static mut INTERRUPT_CONTROLLER: MaybeUninit<GICv2> = MaybeUninit::uninit();
static mut MAILBOX: MaybeUninit<Mailbox> = MaybeUninit::uninit();

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

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_mailbox() -> Result<(), &'static str> {
    let mailbox_mmio_descriptor = MMIODescriptor::new(mmio::MAILBOX_START, mmio::MAILBOX_SIZE);
    let mailbox_virt_addr = memory::mmu::kernel_map_mmio("Mailbox", &mailbox_mmio_descriptor)?;

    MAILBOX.write(Mailbox::new(mailbox_virt_addr));

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
unsafe fn driver_sgi() -> Result<(), &'static str> {
    let sgi_descriptor = generic_driver::DeviceDriverDescriptor::new(
        SGI_HANDLER.assume_init_ref(),
        None,
        Some(exception::asynchronous::irq_map::SGI_9),
    );
    generic_driver::driver_manager().register_driver(sgi_descriptor);

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

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_mailbox() -> Result<(), &'static str> {
    instantiate_mailbox()?;

    let mailbox_descriptor =
        generic_driver::DeviceDriverDescriptor::new(MAILBOX.assume_init_ref(), None, None);
    generic_driver::driver_manager().register_driver(mailbox_descriptor);

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
    driver_sgi()?;
    driver_mailbox()?;
    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}
