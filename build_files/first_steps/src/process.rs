// Starting other programs from the apps, shared by the tour, Tools and
// Parental controls.

use std::process::Command;

/// Starts a program without waiting for it (a browser, another app)
pub fn spawn(program: &str, args: &[&str]) {
    spawn_with(program, args, None);
}

/// The same, with one more environment variable for the program
pub fn spawn_with(program: &str, args: &[&str], env: Option<(&str, &str)>) {
    let mut command = Command::new(program);
    command.args(args);
    if let Some((key, value)) = env {
        command.env(key, value);
    }
    match command.spawn() {
        // Waited for aside, so that a program that ends before this window
        // does not stay in the process list as a zombie
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("Could not start {program}: {error}"),
    }
}
