#![no_std]
#![no_main]
use beskar_lib::io::keyboard;

beskar_lib::entry_point!(main);

fn main() {
    bashkar::video::init();

    loop {
        if let Some(event) = keyboard::poll_keyboard() {
            let line_complete = bashkar::video::tty::with_tty(|tty| tty.handle_key_event(&event));

            if line_complete {
                let line = bashkar::video::tty::with_tty(|tty| tty.drain_input_line());
                let (command, args) = bashkar::commands::parse_command_line(&line);

                let exec_res = bashkar::video::tty::with_tty(|tty| {
                    bashkar::commands::execute_command(&command, &args, tty)
                });

                bashkar::video::tty::with_tty(|tty| {
                    if let Err(err_msg) = exec_res {
                        tty.write_str(&format!("Error: {}\n", err_msg));
                    }

                    tty.reset_input();
                    tty.display_prompt();
                });
            }
        } else {
            core::hint::spin_loop();
        }
    }
}
