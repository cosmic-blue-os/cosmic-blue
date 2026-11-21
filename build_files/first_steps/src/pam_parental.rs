// pam_cosmic_blue_parental.so: the parental controls' login check, as a
// PAM account module of the login screen (see build_files/26-parental-controls.sh).
//
// It runs `cosmic-blue-parental-daemon login-check` for the account logging
// in. When the rules refuse it (bedtime, or the day's screen time used up),
// the login screen shows the reason, in the system's language, as a PAM
// error message (pam_exec could only show its own "... failed: exit code 1",
// and the login screen does not show the information messages pam_exec's
// "stdout" option sends). Accounts without rules are not affected, and an
// unexpected failure of the check lets the login go on: the service closes
// the session anyway if the rules say so.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::process::Command;

const PAM_SUCCESS: c_int = 0;
const PAM_PERM_DENIED: c_int = 6;
const PAM_IGNORE: c_int = 25;
const PAM_ERROR_MSG: c_int = 3;
const LOG_NOTICE: c_int = 5;

const CHECK: &str = "/usr/libexec/cosmic-blue-parental-daemon";

// From libpam, which loads this module (no link: the symbols are the ones of
// the libpam of the program that logs in)
unsafe extern "C" {
    fn pam_get_user(pamh: *mut c_void, user: *mut *const c_char, prompt: *const c_char) -> c_int;
    // (pam_error() is a macro over it)
    fn pam_prompt(pamh: *mut c_void, style: c_int, response: *mut *mut c_char, fmt: *const c_char, ...) -> c_int;
    fn pam_syslog(pamh: *mut c_void, priority: c_int, fmt: *const c_char, ...);
}

/// What the check says about this login: None to let it in, or the message
/// saying why not
fn refusal(user: &str) -> Option<String> {
    let output = Command::new(CHECK)
        .arg("login-check")
        .env_clear()
        .env("PAM_USER", user)
        .env("PATH", "/usr/bin:/usr/sbin")
        .output()
        .ok()?;
    if output.status.code() != Some(1) {
        return None;
    }
    let message = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !message.is_empty() {
        return Some(message);
    }
    // No reason given: the general message
    let output = Command::new(CHECK).arg("refusal").env_clear().output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// # Safety
/// Called by libpam with a valid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pam_sm_acct_mgmt(
    pamh: *mut c_void,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    let mut user: *const c_char = std::ptr::null();
    // SAFETY: valid handle from libpam; the user name stays libpam's
    if unsafe { pam_get_user(pamh, &mut user, std::ptr::null()) } != PAM_SUCCESS || user.is_null() {
        return PAM_IGNORE;
    }
    // SAFETY: a NUL-terminated string from libpam
    let user = unsafe { CStr::from_ptr(user) }.to_string_lossy().into_owned();
    let Ok(refused) = std::panic::catch_unwind(|| refusal(&user)) else {
        return PAM_IGNORE;
    };
    let Some(message) = refused else {
        return PAM_SUCCESS;
    };
    let message = CString::new(message.replace('\0', "")).unwrap_or_default();
    let log = CString::new(format!("login refused for {user}").replace('\0', "")).unwrap_or_default();
    // SAFETY: "%s" with one NUL-terminated string
    unsafe {
        pam_syslog(pamh, LOG_NOTICE, c"%s".as_ptr(), log.as_ptr());
        if !message.as_bytes().is_empty() {
            pam_prompt(pamh, PAM_ERROR_MSG, std::ptr::null_mut(), c"%s".as_ptr(), message.as_ptr());
        }
    }
    PAM_PERM_DENIED
}
