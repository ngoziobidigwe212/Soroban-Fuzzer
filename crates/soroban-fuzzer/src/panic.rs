use std::{
    any::Any,
    cell::Cell,
    panic::{self, AssertUnwindSafe},
    sync::Once,
};

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_HOOK: Once = Once::new();

/// Install (once per process) a panic hook that stays silent on threads that
/// are currently inside [`catch_quiet`]. Other threads keep the default
/// output, so unrelated tests running in parallel are unaffected.
fn install_hook() {
    INSTALL_HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !QUIET.with(Cell::get) {
                previous(info);
            }
        }));
    });
}

/// Run `f`, turning a panic into `Err(message)` without printing it.
///
/// Thousands of panics are expected while fuzzing and shrinking; printing
/// each one would bury the actual report.
pub fn catch_quiet<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    install_hook();
    let was_quiet = QUIET.with(|q| q.replace(true));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(was_quiet));
    result.map_err(payload_message)
}

fn payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}
