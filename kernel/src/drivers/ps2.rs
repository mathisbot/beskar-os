//! PS/2 Controller and Keyboard Driver
use beskar_core::drivers::DriverResult;
use core::sync::atomic::AtomicBool;
use hyperdrive::once::Once;

mod controller;
use controller::Ps2Controller;
mod error;
mod keyboard;
use keyboard::Ps2Keyboard;

static PS2_AVAILABLE: AtomicBool = AtomicBool::new(false);

static PS2_CONTROLLER: Ps2Controller = Ps2Controller::new();
static PS2_KEYBOARD: Once<Ps2Keyboard> = Once::uninit();

/// Initialize the PS/2 controller and keyboard.
///
/// # Errors
///
/// Returns an error if controller initialization or keyboard setup fails.
pub fn init() -> DriverResult<()> {
    PS2_CONTROLLER.initialize()?;
    let ps2_keyboard = Ps2Keyboard::new(&PS2_CONTROLLER)?;
    PS2_KEYBOARD.call_once(|| ps2_keyboard);
    video::info!("PS/2 controller initialized");
    Ok(())
}

/// Handle a keyboard interrupt.
///
/// This function is called from the keyboard IRQ handler.
/// It polls for key events and pushes them to the keyboard event queue.
pub fn handle_keyboard_interrupt() {
    let Some(keyboard) = PS2_KEYBOARD.get() else {
        return;
    };

    let Some(key_event) = keyboard.poll_key_event() else {
        return;
    };

    super::keyboard::with_keyboard_manager(|manager| {
        manager.push_event(key_event);
    });
}
