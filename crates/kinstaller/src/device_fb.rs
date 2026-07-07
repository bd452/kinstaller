//! Framebuffer geometry and DPI-aware UI scale for on-device builds.

/// Visible panel metrics used to map the 600×800 design to physical e-ink pixels.
#[cfg(feature = "device")]
pub(crate) struct DisplayMetrics {
    pub width_px: u32,
    pub height_px: u32,
    /// Visible panel width in millimetres (short edge in portrait).
    pub width_mm: f32,
}

#[cfg(feature = "device")]
#[repr(C)]
#[derive(Default)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

/// Linux `fb_var_screeninfo` (uapi/linux/fb.h) — kernel writes the full struct.
#[cfg(feature = "device")]
#[repr(C)]
#[derive(Default)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[cfg(feature = "device")]
pub(crate) fn visible_pixel_size() -> Option<(u32, u32)> {
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;

    let file = File::open("/dev/fb0").ok()?;
    let fd = file.as_raw_fd();
    let mut vinfo = FbVarScreeninfo::default();
    // SAFETY: vinfo matches the kernel's fb_var_screeninfo layout for FBIOGET_VSCREENINFO.
    let rc = unsafe {
        libc::ioctl(
            fd,
            FBIOGET_VSCREENINFO,
            &mut vinfo as *mut FbVarScreeninfo as *mut libc::c_void,
        )
    };
    if rc == -1 {
        return None;
    }
    if vinfo.xres == 0 || vinfo.yres == 0 {
        return None;
    }
    Some((vinfo.xres, vinfo.yres))
}

/// Best-effort physical width for known KPM panel classes (portrait short edge).
#[cfg(feature = "device")]
fn panel_width_mm(width_px: u32, height_px: u32) -> f32 {
    let (w, h) = if width_px <= height_px {
        (width_px, height_px)
    } else {
        (height_px, width_px)
    };

    match (w, h) {
        // kindlehf (FW >= 5.16.3) — ~7" class panels at 1072×1448.
        (1072, 1448) => 108.0,
        // kindlepw2 / 6" class.
        (758, 1024) => 76.0,
        (600, 800) => 62.0,
        _ => {
            // ~250 DPI fallback when the panel is unknown.
            w as f32 / 250.0 * 25.4
        }
    }
}

#[cfg(feature = "device")]
pub(crate) fn display_metrics(width_px: u32, height_px: u32) -> DisplayMetrics {
    DisplayMetrics {
        width_px,
        height_px,
        width_mm: panel_width_mm(width_px, height_px),
    }
}

/// Scale Theme tokens to the device panel. Uses Theme.ui-scale (not Slint window
/// scale-factor) so touch coordinates from slint-backend-kindle stay aligned:
/// that backend maps touches to physical pixels and passes them as logical coords.
#[cfg(feature = "device")]
pub(crate) fn scale_factor(metrics: &DisplayMetrics) -> f32 {
    const DESIGN_W: f32 = 600.0;
    const DESIGN_H: f32 = 800.0;
    /// Reference density for the desktop mock / Slint logical units.
    const DESIGN_DPI: f32 = 160.0;

    let fill = (metrics.width_px as f32 / DESIGN_W).min(metrics.height_px as f32 / DESIGN_H);
    let device_dpi = metrics.width_px as f32 / (metrics.width_mm / 25.4);
    let density = (device_dpi / DESIGN_DPI).max(1.0);
    fill * density
}

#[cfg(feature = "device")]
pub(crate) fn apply_theme_scale(app: &crate::AppWindow, metrics: &DisplayMetrics) -> f32 {
    use slint::ComponentHandle;
    let factor = scale_factor(metrics);
    app.global::<crate::Theme>().set_ui_scale(factor);
    factor
}
