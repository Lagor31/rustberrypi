// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2020-2022 Andre Richter <andre.o.richter@gmail.com>

//! GICv2 Driver - ARM Generic Interrupt Controller v2.

mod gicc;
mod gicd;

use crate::{
    cpu, driver,
    drivers::common::BoundedUsize,
    exception,
    memory::{Address, Virtual},
    synchronization,
    synchronization::InitStateLock,
};

use alloc::vec::Vec;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

type HandlerTable = Vec<Option<exception::asynchronous::IRQHandlerDescriptor<IRQNumber>>>;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Used for the associated type of trait [`exception::asynchronous::interface::IRQManager`].
pub type IRQNumber = BoundedUsize<{ GICv2::MAX_IRQ_NUMBER }>;

/// Representation of the GIC.
pub struct GICv2 {
    /// The Distributor.
    gicd: gicd::GICD,

    /// The CPU Interface.
    gicc: gicc::GICC,

    /// Stores registered IRQ handlers. Writable only during kernel init. RO afterwards.
    handler_table: InitStateLock<HandlerTable>,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl GICv2 {
    const MAX_IRQ_NUMBER: usize = 1019;
    ///Driver name
    pub const COMPATIBLE: &'static str = "GICv2 (ARM Generic Interrupt Controller v2)";

    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(
        gicd_mmio_start_addr: Address<Virtual>,
        gicc_mmio_start_addr: Address<Virtual>,
    ) -> Self {
        Self {
            gicd: gicd::GICD::new(gicd_mmio_start_addr),
            gicc: gicc::GICC::new(gicc_mmio_start_addr),
            handler_table: InitStateLock::new(Vec::new()),
        }
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::ReadWriteEx;

impl driver::interface::DeviceDriver for GICv2 {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }

    unsafe fn init(&self) -> Result<(), &'static str> {
        self.handler_table
            .write(|table| table.resize(IRQNumber::MAX_INCLUSIVE + 1, None));

        if cpu::BOOT_CORE_ID == cpu::core_id() {
            self.gicd.boot_core_init();
        }

        self.gicc.priority_accept_all();
        self.gicc.enable();

        Ok(())
    }
}

impl exception::asynchronous::interface::IRQManager for GICv2 {
    type IRQNumberType = IRQNumber;

    fn register_handler(
        &self,
        irq_handler_descriptor: exception::asynchronous::IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str> {
        self.handler_table.write(|table| {
            let irq_number = irq_handler_descriptor.number().get();

            if table[irq_number].is_some() {
                return Err("IRQ handler already registered");
            }

            table[irq_number] = Some(irq_handler_descriptor);

            Ok(())
        })
    }

    fn enable(&self, irq_number: &Self::IRQNumberType) {
        self.gicd.enable(irq_number);
    }

    fn handle_pending_irqs<'irq_context>(
        &'irq_context self,
        ic: &exception::asynchronous::IRQContext<'irq_context>,
    ) {
        // Extract the highest priority pending IRQ number from the Interrupt Acknowledge Register
        // (IAR).
        let irq_number = self.gicc.pending_irq_number(ic);

        // Guard against spurious interrupts.
        if irq_number > GICv2::MAX_IRQ_NUMBER {
            return;
        }

        // Call the IRQ handler. Panic if there is none.
        self.handler_table.read(|table| {
            match table[irq_number] {
                None => panic!("No handler registered for IRQ {}", irq_number),
                Some(descriptor) => {
                    // Call the IRQ handler. Panics on failure.
                    descriptor.handler().handle().expect("Error handling IRQ");
                }
            }
        });

        // Signal completion of handling.
        self.gicc.mark_comleted(irq_number as u32, ic);
    }

    fn print_handler(&self) {
        use crate::info;

        info!("      Peripheral handler:");

        self.handler_table.read(|table| {
            for (i, opt) in table.iter().skip(32).enumerate() {
                if let Some(handler) = opt {
                    info!("            {: >3}. {}", i + 32, handler.name());
                }
            }
        });
    }
}
