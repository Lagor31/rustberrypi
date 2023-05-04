// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! Device driver.

mod arm;
mod bcm;
mod common;

pub use arm::*;
pub use bcm::*;
