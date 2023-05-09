/// Device Driver functions.
pub trait DeviceDriver {
    /// Different interrupt controllers might use different types for IRQ number.
    type IRQNumberType: super::fmt::Display;

    /// Return a compatibility string for identifying the driver.
    fn compatible(&self) -> &'static str;

    /// Called by the kernel to bring up the device.
    ///
    /// # Safety
    ///
    /// - During init, drivers might do stuff with system-wide impact.
    unsafe fn init(&self) -> Result<(), &'static str> {
        Ok(())
    }

    /// Called by the kernel to register and enable the device's IRQ handler.
    ///
    /// Rust's type system will prevent a call to this function unless the calling instance
    /// itself has static lifetime.
    fn register_and_enable_irq_handler(
        &'static self,
        irq_number: &Self::IRQNumberType,
    ) -> Result<(), &'static str> {
        panic!(
            "Attempt to enable IRQ {} for device {}, but driver does not support this",
            irq_number,
            self.compatible()
        )
    }
}
