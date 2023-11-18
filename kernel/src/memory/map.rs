//! Memory map

use super::{Address, Physical};

/// Physical devices.
pub mod mmio {
    use crate::memory::{Address, Physical};

    pub const MAILBOX_START: Address<Physical> = Address::new(0xFE00_B880);
    pub const MAILBOX_SIZE: usize = 0x24;

    pub const GPIO_START: Address<Physical> = Address::new(0xFE20_0000);
    pub const GPIO_SIZE: usize = 0xA0;

    pub const PL011_UART_START: Address<Physical> = Address::new(0xFE20_1000);
    pub const PL011_UART_SIZE: usize = 0x48;

    pub const GICD_START: Address<Physical> = Address::new(0xFF84_1000);
    pub const GICD_SIZE: usize = 0x824;

    pub const GICC_START: Address<Physical> = Address::new(0xFF84_2000);
    pub const GICC_SIZE: usize = 0x14;

    pub const END: Address<Physical> = Address::new(0xFF85_0000);
}

pub const END: Address<Physical> = mmio::END;
