#![no_std]
#![no_main]
use alloc::{string::String, vec::Vec};
use beskar_lib::io::keyboard;

beskar_lib::entry_point!(main);

fn main() {
    bashkar::video::init();

    loop {
        if let Some(event) = keyboard::poll_keyboard() {
            let line_complete = bashkar::video::tty::with_tty(|tty| tty.handle_key_event(&event));

            if line_complete {
                // Process the command
                bashkar::video::tty::with_tty(|tty| {
                    let input = tty.get_input_line();
                    let (command, args) = bashkar::commands::parse_command_line(input);

                    // FIXME: Cloning the args into Strings is necessary to avoid borrowing issues below,
                    // but it would be better to avoid this.
                    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();

                    let _exec_res = bashkar::commands::execute_command(&command, &args_ref, tty);
                    // match exec_res {
                    //     bashkar::commands::CommandResult::Success => {
                    //         // Success, just show the prompt again
                    //     }
                    //     bashkar::commands::CommandResult::Error(_msg) => {
                    //         // Error, what to do more?
                    //     }
                    // }

                    // Reset the input buffer
                    tty.reset_input();
                    tty.display_prompt();
                });
            }
        } else {
            core::hint::spin_loop();
        }
    }
}
