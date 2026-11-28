// Windows that one shortcut opens and closes (the keyboard shortcuts sheet,
// Super + F1; the quick menu, Super + Alt + Space): the second press closes
// the open one instead of opening another.

use std::path::PathBuf;

/// The window already open (its process number in the runtime folder, still
/// running with `flag`): close it and say so; otherwise note this one
pub fn close_if_open(name: &str, flag: &str) -> bool {
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return false;
    };
    let file = PathBuf::from(runtime).join(format!("{name}.pid"));
    let open = std::fs::read_to_string(&file)
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .filter(|pid| {
            *pid != std::process::id()
                && std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|cmdline| {
                    cmdline
                        .split(|byte| *byte == 0)
                        .any(|arg| arg == flag.as_bytes())
                })
        });
    if let Some(pid) = open {
        let closed = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status()
            .is_ok_and(|status| status.success());
        if closed {
            return true;
        }
    }
    let _ = std::fs::write(&file, std::process::id().to_string());
    false
}
