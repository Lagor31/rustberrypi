use crate::{
    console, cpu, driver,
    drivers::common::MMIODerefWrapper,
    exception::{self, arch_exception::ExceptionContext, asynchronous::IRQNumber},
    info,
    memory::{Address, Virtual},
    scheduler::reschedule_from_context,
    synchronization,
    synchronization::IRQSafeLock,
    time::time_manager,
};
use core::{fmt, time::Duration};

use spin::mutex::SpinMutex;
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

pub struct SGIHandler {}

impl SGIHandler {
    pub const COMPATIBLE: &'static str = "SGI Handler";

    pub const fn new() -> Self {
        Self {}
    }
}

impl driver::interface::DeviceDriver for SGIHandler {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }

    unsafe fn init(&self) -> Result<(), &'static str> {
        Ok(())
    }

    fn register_and_enable_irq_handler(
        &'static self,
        irq_number: &Self::IRQNumberType,
    ) -> Result<(), &'static str> {
        use exception::asynchronous::{irq_manager, IRQHandlerDescriptor};

        let descriptor = IRQHandlerDescriptor::new(*irq_number, Self::COMPATIBLE, self);

        irq_manager().register_handler(descriptor)?;
        irq_manager().enable(irq_number);

        Ok(())
    }
}

impl exception::asynchronous::interface::IRQHandler for SGIHandler {
    fn handle(&self, e: &mut ExceptionContext) -> Result<(), &'static str> {
        //let coreid: usize = cpu::core_id();
        //info!("Called SGI Handler 9 on Core{}", coreid);
        reschedule_from_context(e);
        Ok(())
    }
}
