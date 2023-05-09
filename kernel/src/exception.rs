// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2020-2022 Andre Richter <andre.o.richter@gmail.com>

//! Synchronous and asynchronous exception handling.

#[path = "aarch64/exception.rs"]
pub mod arch_exception;

pub mod asynchronous;

//--------------------------------------------------------------------------------------------------
// Architectural Public Reexports
//--------------------------------------------------------------------------------------------------
pub use arch_exception::{current_privilege_level, handling_init};

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Kernel privilege levels.
#[allow(missing_docs)]
#[derive(Eq, PartialEq)]
pub enum PrivilegeLevel {
    User,
    Kernel,
    Hypervisor,
    Unknown,
}
