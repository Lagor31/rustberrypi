use spin::mutex::SpinMutex;
use tock_registers::{
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite},
};

use crate::{
    driver,
    memory::{Address, Virtual},
    synchronization::{interface::Mutex, IRQSafeLock},
};

use super::{common::MMIODerefWrapper, IRQNumber};

register_bitfields! {
    u32,

    READ [
        CHANNEL OFFSET(0) NUMBITS(4) [],
        DATA OFFSET(4) NUMBITS(28) [],
    ],

    WRITE [
        CHANNEL OFFSET(0) NUMBITS(4) [],
        DATA OFFSET(4) NUMBITS(28) [],
    ],

    STATUS [
        READ_EMPTY OFFSET(30) NUMBITS(1) [],
        WRITE_FULL OFFSET(31) NUMBITS(1) [],
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    ///Regblock
    pub RegisterBlock {
        (0x00 => READ: ReadWrite<u32, READ::Register>),
        (0x04 => _reserved1),
        (0x18 => STATUS: ReadWrite<u32, STATUS::Register>),
        (0x1C => _reserved2),
        (0x20 => WRITE: ReadWrite<u32, WRITE::Register>),
        (0x24 => @END),
    }
}

type Registers = MMIODerefWrapper<RegisterBlock>;

pub struct Mailbox {
    inner: IRQSafeLock<SpinMutex<MailboxInner>>,
}

impl Mailbox {
    pub const COMPATIBLE: &'static str = "Mailbox";

    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        Self {
            inner: IRQSafeLock::new(SpinMutex::new(MailboxInner::new(mmio_start_addr))),
        }
    }
}

impl driver::interface::DeviceDriver for Mailbox {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }

    unsafe fn init(&self) -> Result<(), &'static str> {
        self.inner.lock(|inner| inner.lock().init());
        Ok(())
    }

    fn register_and_enable_irq_handler(
        &'static self,
        _irq_number: &Self::IRQNumberType,
    ) -> Result<(), &'static str> {
        Ok(())
    }
}

struct MailboxInner {
    registers: Registers,
}

impl MailboxInner {
    pub const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        Self {
            registers: Registers::new(mmio_start_addr),
        }
    }
    pub fn init(&mut self) {}
}
