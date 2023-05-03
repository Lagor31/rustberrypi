// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! System console.

mod buffer_console {
    // SPDX-License-Identifier: MIT OR Apache-2.0
    //
    // Copyright (c) 2022 Andre Richter <andre.o.richter@gmail.com>

    //! A console that buffers input during the init phase.

    use super::interface;
    use crate::{console, info, synchronization, synchronization::InitStateLock};
    use core::fmt;

    //--------------------------------------------------------------------------------------------------
    // Private Definitions
    //--------------------------------------------------------------------------------------------------

    const BUF_SIZE: usize = 1024 * 64;

    pub struct BufferConsoleInner {
        buf: [char; BUF_SIZE],
        write_ptr: usize,
    }

    //--------------------------------------------------------------------------------------------------
    // Public Definitions
    //--------------------------------------------------------------------------------------------------

    pub struct BufferConsole {
        inner: InitStateLock<BufferConsoleInner>,
    }

    //--------------------------------------------------------------------------------------------------
    // Global instances
    //--------------------------------------------------------------------------------------------------

    pub static BUFFER_CONSOLE: BufferConsole = BufferConsole {
        inner: InitStateLock::new(BufferConsoleInner {
            // Use the null character, so this lands in .bss and does not waste space in the binary.
            buf: ['\0'; BUF_SIZE],
            write_ptr: 0,
        }),
    };

    //--------------------------------------------------------------------------------------------------
    // Private Code
    //--------------------------------------------------------------------------------------------------

    impl BufferConsoleInner {
        fn write_char(&mut self, c: char) {
            if self.write_ptr < (BUF_SIZE - 1) {
                self.buf[self.write_ptr] = c;
                self.write_ptr += 1;
            }
        }
    }

    impl fmt::Write for BufferConsoleInner {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for c in s.chars() {
                self.write_char(c);
            }

            Ok(())
        }
    }

    //--------------------------------------------------------------------------------------------------
    // Public Code
    //--------------------------------------------------------------------------------------------------
    use synchronization::interface::ReadWriteEx;

    impl BufferConsole {
        /// Dump the buffer.
        ///
        /// # Invariant
        ///
        /// It is expected that this is only called when self != crate::console::console().
        pub fn dump(&self) {
            self.inner.read(|inner| {
                console::console().write_array(&inner.buf[0..inner.write_ptr]);

                if inner.write_ptr == (BUF_SIZE - 1) {
                    info!("Pre-UART buffer overflowed");
                } else if inner.write_ptr > 0 {
                    info!("End of pre-UART buffer")
                }
            });
        }
    }

    impl interface::Write for BufferConsole {
        fn write_char(&self, c: char) {
            self.inner.write(|inner| inner.write_char(c));
        }

        fn write_array(&self, _a: &[char]) {}

        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result {
            self.inner.write(|inner| fmt::Write::write_fmt(inner, args))
        }

        fn flush(&self) {}
    }

    impl interface::Read for BufferConsole {
        fn clear_rx(&self) {}
    }

    impl interface::Statistics for BufferConsole {}
    impl interface::All for BufferConsole {}
}

use crate::synchronization;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Console interfaces.
pub mod interface {
    use core::fmt;

    /// Console write functions.
    pub trait Write {
        /// Write a single character.
        fn write_char(&self, c: char);

        /// Write a slice of characters.
        fn write_array(&self, a: &[char]);

        /// Write a Rust format string.
        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;

        /// Block until the last buffered character has been physically put on the TX wire.
        fn flush(&self);
    }

    /// Console read functions.
    pub trait Read {
        /// Read a single character.
        fn read_char(&self) -> char {
            ' '
        }

        /// Clear RX buffers, if any.
        fn clear_rx(&self);
    }

    /// Console statistics.
    pub trait Statistics {
        /// Return the number of characters written.
        fn chars_written(&self) -> usize {
            0
        }

        /// Return the number of characters read.
        fn chars_read(&self) -> usize {
            0
        }
    }

    /// Trait alias for a full-fledged console.
    pub trait All: Write + Read + Statistics {}
}

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
