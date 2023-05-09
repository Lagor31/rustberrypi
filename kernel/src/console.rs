// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! System console.

mod buffer_console;

use crate::synchronization;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Console interfaces.
pub mod interface;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static CUR_CONSOLE: InitStateLock<&'static (dyn interface::All + Sync)> =
    InitStateLock::new(&buffer_console::BUFFER_CONSOLE);

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
use synchronization::{interface::ReadWriteEx, InitStateLock};

/// Register a new console.
pub fn register_console(new_console: &'static (dyn interface::All + Sync)) {
    CUR_CONSOLE.write(|con| *con = new_console);

    static FIRST_SWITCH: InitStateLock<bool> = InitStateLock::new(true);
    FIRST_SWITCH.write(|first| {
        if *first {
            *first = false;

            buffer_console::BUFFER_CONSOLE.dump();
        }
    });
}

/// Return a reference to the currently registered console.
///
/// This is the global console used by all printing macros.
pub fn console() -> &'static dyn interface::All {
    CUR_CONSOLE.read(|con| *con)
}
