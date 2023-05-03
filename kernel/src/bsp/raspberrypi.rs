// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! Top-level BSP file for the Raspberry Pi 3 and 4.

pub mod cpu;
pub mod driver;
pub mod exception;
pub mod memory;

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Board identification.
pub fn board_name() -> &'static str {
    #[cfg(feature = "bsp_rpi4")]
    {
        "Raspberry Pi 4"
    }
}
