mod controller;
#[cfg(feature = "device")]
mod device_fb;
#[cfg(feature = "device")]
mod device_power;

slint::include_modules!();

use slint::ComponentHandle;

#[cfg(feature = "device")]
static REGULAR_FONT: &[u8] = include_bytes!("../fonts/LiberationSans-Regular.ttf");
#[cfg(feature = "device")]
static BOLD_FONT: &[u8] = include_bytes!("../fonts/LiberationSans-Bold.ttf");

fn wire_quit(app: &AppWindow) {
    app.global::<AppState>().on_request_quit(|| {
        let _ = slint::quit_event_loop();
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "device")]
    let kindle_backend =
        slint_backend_kindle::install(REGULAR_FONT).expect("failed to install Kindle backend");

    let app = AppWindow::new()?;
    wire_quit(&app);

    #[cfg(feature = "device")]
    if let Some((width, height)) = device_fb::visible_pixel_size() {
        let metrics = device_fb::display_metrics(width, height);
        let factor = device_fb::apply_theme_scale(&app, &metrics);
        eprintln!(
            "kinstaller: display {}x{} px (~{:.0} mm wide), ui-scale {factor:.2}",
            metrics.width_px,
            metrics.height_px,
            metrics.width_mm
        );
    }

    #[cfg(feature = "device")]
    kindle_backend
        .register_font_from_memory(BOLD_FONT)
        .expect("failed to register bold font");

    #[cfg(feature = "device")]
    device_power::spawn_power_monitor(kindle_backend.display_control());

    match kpm::client::start_default_client() {
        Ok(client) => {
            let ctrl = controller::Controller::new(client, app.as_weak());
            ctrl.wire(&app);
            ctrl.refresh_home();
            app.run()?;
        }
        Err(kpm::Error::Incompatible(msg)) | Err(kpm::Error::Unavailable(msg)) => {
            controller::set_compat_error(&app, "KPM version not supported", &msg);
            app.run()?;
        }
        Err(e) => return Err(format!("could not start KPM backend: {e}").into()),
    }

    Ok(())
}
