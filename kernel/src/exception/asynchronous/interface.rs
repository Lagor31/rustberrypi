use crate::exception::arch_exception::ExceptionContext;

/// Implemented by types that handle IRQs.
pub trait IRQHandler {
    /// Called when the corresponding interrupt is asserted.
    fn handle(&self, e: &mut ExceptionContext) -> Result<(), &'static str>;
}

/// IRQ management functions.
///
/// The `BSP` is supposed to supply one global instance. Typically implemented by the
/// platform's interrupt controller.
pub trait IRQManager {
    /// The IRQ number type depends on the implementation.
    type IRQNumberType: Copy;

    /// Register a handler.
    fn register_handler(
        &self,
        irq_handler_descriptor: super::IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str>;

    /// Enable an interrupt in the controller.
    fn enable(&self, irq_number: &Self::IRQNumberType);

    /// Handle pending interrupts.
    ///
    /// This function is called directly from the CPU's IRQ exception vector. On AArch64,
    /// this means that the respective CPU core has disabled exception handling.
    /// This function can therefore not be preempted and runs start to finish.
    ///
    /// Takes an IRQContext token to ensure it can only be called from IRQ context.
    //#[allow(clippy::trivially_copy_pass_by_ref)]
    fn handle_pending_irqs<'irq_context>(
        &'irq_context self,
        ic: &super::IRQContext<'irq_context>,
        e: &mut ExceptionContext,
    );

    /// Print list of registered handlers.
    fn print_handler(&self) {}
}
