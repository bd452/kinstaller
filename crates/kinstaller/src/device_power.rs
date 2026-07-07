//! Pause e-ink output while the device sleeps so partial refreshes do not draw
//! over the stock lock screen.
//!
//! The Slint Kindle backend draws directly to `/dev/fb0`. If we keep refreshing
//! while powerd shows the screensaver / swipe-to-unlock UI, partial e-ink
//! updates ghost on unlock. KUAL-style apps stay alive in the background; we
//! release the framebuffer and hand the panel back to the stock UI until wake.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const POWER_SOURCE: &str = "com.lab126.powerd";

/// LIPC events that mean the stock screensaver / lock UI is taking over.
const PAUSE_EVENTS: &[&str] = &[
    "goingToScreenSaver",
    "readyToSuspend",
    "suspending",
];

/// LIPC events that mean kinstaller should reclaim the panel.
const RESUME_EVENTS: &[&str] = &["outOfScreenSaver", "wakeupFromSuspend"];

/// Device shutdown — exit so launch.sh can restore the stock UI.
const QUIT_EVENTS: &[&str] = &["userShutdown"];

static DISPLAY_SUSPENDED: AtomicBool = AtomicBool::new(false);

/// Start watching powerd for sleep/wake transitions.
pub fn spawn_power_monitor(display: slint_backend_kindle::DisplayControl) {
    thread::spawn(move || {
        loop {
            match wait_for_power_event() {
                Ok(PowerEvent::Pause(event)) => {
                    if DISPLAY_SUSPENDED
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        eprintln!("kinstaller: {event}, pausing display");
                        resume_stock_ui();
                        display.set_paused(true);
                    }
                }
                Ok(PowerEvent::Resume(event)) => {
                    if DISPLAY_SUSPENDED
                        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        eprintln!("kinstaller: {event}, resuming display");
                        pause_stock_ui();
                        display.set_paused(false);
                    }
                }
                Ok(PowerEvent::Quit(event)) => {
                    eprintln!("kinstaller: {event}, exiting");
                    let _ = slint::quit_event_loop();
                    break;
                }
                Err(e) => {
                    eprintln!("kinstaller: power monitor: {e}");
                    if screensaver_active() {
                        if DISPLAY_SUSPENDED
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            eprintln!("kinstaller: screensaver active, pausing display");
                            resume_stock_ui();
                            display.set_paused(true);
                        }
                    } else if DISPLAY_SUSPENDED.load(Ordering::SeqCst) {
                        eprintln!("kinstaller: screensaver cleared, resuming display");
                        if DISPLAY_SUSPENDED
                            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            pause_stock_ui();
                            display.set_paused(false);
                        }
                    }
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    });
}

enum PowerEvent {
    Pause(&'static str),
    Resume(&'static str),
    Quit(&'static str),
}

fn screensaver_active() -> bool {
    lipc_prop_equals(POWER_SOURCE, "state", "screenSaver")
}

fn lipc_prop_equals(source: &str, prop: &str, expected: &str) -> bool {
    lipc_get_prop(source, prop)
        .map(|value| value.trim() == expected)
        .unwrap_or(false)
}

fn lipc_get_prop(source: &str, prop: &str) -> Option<String> {
    Command::new("lipc-get-prop")
        .args([source, prop])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn wait_for_power_event() -> Result<PowerEvent, Box<dyn std::error::Error>> {
    let mut child = Command::new("lipc-wait-event")
        .args(["-m", POWER_SOURCE, "*"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout piped");
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        let line = line?;
        if let Some(event) = power_event_name(&line) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(event);
        }
    }

    let status = child.wait()?;
    Err(format!("lipc-wait-event exited: {status}").into())
}

fn power_event_name(line: &str) -> Option<PowerEvent> {
    for event in QUIT_EVENTS {
        if line.split_whitespace().any(|part| part == *event) {
            return Some(PowerEvent::Quit(event));
        }
    }
    for event in PAUSE_EVENTS {
        if line.split_whitespace().any(|part| part == *event) {
            return Some(PowerEvent::Pause(event));
        }
    }
    for event in RESUME_EVENTS {
        if line.split_whitespace().any(|part| part == *event) {
            return Some(PowerEvent::Resume(event));
        }
    }
    None
}

/// Hand the panel back to the stock UI (inverse of launch.sh startup).
///
/// Do not STOP/CONT awesome/cvm/KPPMainApp — a stopped process keeps its
/// EVIOCGRAB on the touch device and kinstaller cannot grab it on resume.
fn resume_stock_ui() {
    let _ = Command::new("lipc-set-prop")
        .args(["com.lab126.pillow", "disableEnablePillow", "enable"])
        .status();
}

/// Reclaim the panel from the stock UI (mirrors launch.sh startup).
fn pause_stock_ui() {
    let _ = Command::new("lipc-set-prop")
        .args(["com.lab126.pillow", "disableEnablePillow", "disable"])
        .status();
    thread::sleep(Duration::from_millis(300));
}

#[cfg(test)]
mod tests {
    use super::power_event_name;

    #[test]
    fn parses_lipc_power_event_lines() {
        use super::PowerEvent;

        assert!(matches!(
            power_event_name("com.lab126.powerd goingToScreenSaver 2"),
            Some(PowerEvent::Pause("goingToScreenSaver"))
        ));
        assert!(matches!(
            power_event_name("readyToSuspend 0"),
            Some(PowerEvent::Pause("readyToSuspend"))
        ));
        assert!(matches!(
            power_event_name("outOfScreenSaver 1"),
            Some(PowerEvent::Resume("outOfScreenSaver"))
        ));
        assert!(matches!(
            power_event_name("wakeupFromSuspend 0"),
            Some(PowerEvent::Resume("wakeupFromSuspend"))
        ));
        assert!(matches!(
            power_event_name("userShutdown 0"),
            Some(PowerEvent::Quit("userShutdown"))
        ));
        assert!(power_event_name("charging 1").is_none());
    }
}
