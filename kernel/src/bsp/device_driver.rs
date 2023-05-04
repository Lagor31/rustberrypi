// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2022 Andre Richter <andre.o.richter@gmail.com>

//! Device driver.

mod arm {
    pub mod gicv2;

    pub use gicv2::*;
}
mod bcm {

    mod bcm2xxx_gpio;
    mod bcm2xxx_pl011_uart;

    pub use bcm2xxx_gpio::*;
    pub use bcm2xxx_pl011_uart::*;
}
mod common {
    // SPDX-License-Identifier: MIT OR Apache-2.0
    //
    // Copyright (c) 2020-2022 Andre Richter <andre.o.richter@gmail.com>

    //! Common device driver code.

    use crate::memory::{Address, Virtual};
    use core::{fmt, marker::PhantomData, ops};

    //--------------------------------------------------------------------------------------------------
    // Public Definitions
    //--------------------------------------------------------------------------------------------------

    pub struct MMIODerefWrapper<T> {
        start_addr: Address<Virtual>,
        phantom: PhantomData<fn() -> T>,
    }

    /// A wrapper type for usize with integrated range bound check.
    #[derive(Copy, Clone)]
    pub struct BoundedUsize<const MAX_INCLUSIVE: usize>(usize);

    //--------------------------------------------------------------------------------------------------
    // Public Code
    //--------------------------------------------------------------------------------------------------

    impl<T> MMIODerefWrapper<T> {
        /// Create an instance.
        pub const unsafe fn new(start_addr: Address<Virtual>) -> Self {
            Self {
                start_addr,
                phantom: PhantomData,
            }
        }
    }

    impl<T> ops::Deref for MMIODerefWrapper<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { &*(self.start_addr.as_usize() as *const _) }
        }
    }

    impl<const MAX_INCLUSIVE: usize> BoundedUsize<{ MAX_INCLUSIVE }> {
        pub const MAX_INCLUSIVE: usize = MAX_INCLUSIVE;

        /// Creates a new instance if number <= MAX_INCLUSIVE.
        pub const fn new(number: usize) -> Self {
            assert!(number <= MAX_INCLUSIVE);

            Self(number)
        }

        /// Return the wrapped number.
        pub const fn get(self) -> usize {
            self.0
        }
    }

    impl<const MAX_INCLUSIVE: usize> fmt::Display for BoundedUsize<{ MAX_INCLUSIVE }> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }
}

pub use arm::*;
pub use bcm::*;
