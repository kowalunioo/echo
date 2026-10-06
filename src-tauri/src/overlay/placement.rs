//! Where the Overlay goes (`overlay.md` rules 13–15): centred on the monitor under the mouse
//! pointer, 12 logical pixels above the bottom of its work area (or just below the top), sized in
//! logical pixels times the monitor's scaling and the Windows "Text size" factor. All values here
//! are physical pixels unless named logical.

use super::OverlayPosition;

/// The compact Overlay's window, in logical pixels before the text-size factor. The pill inside
/// is 256 × 44 and narrower when transcribing; the window is wider so that messages fit,
/// and the window region keeps clicks beside the pill going to the windows beneath (rule 11).
pub const WINDOW_WIDTH: f64 = 520.0;
pub const WINDOW_HEIGHT: f64 = 44.0;
/// Gap between the Overlay and the edge of the work area, in logical pixels (rule 14).
pub const BOTTOM_MARGIN: f64 = 12.0;
pub const TOP_MARGIN: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// One monitor as Windows reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    pub bounds: Rect,
    /// The monitor minus the taskbar and other app bars.
    pub work_area: Rect,
    /// Display scaling: 1.0 at 100 %, 1.5 at 150 %.
    pub scale: f64,
}

/// The monitor containing the pointer (rule 13); the first monitor (Windows lists the primary
/// first) if the pointer is somehow on none of them.
pub fn monitor_at(monitors: &[Monitor], pointer: (i32, i32)) -> Option<Monitor> {
    monitors
        .iter()
        .find(|m| m.bounds.contains(pointer.0, pointer.1))
        .or_else(|| monitors.first())
        .copied()
}

/// The Overlay window's rectangle on `monitor`. `text_scale` is the Windows "Text size" factor
/// (1.0 at 100 %).
pub fn place(monitor: &Monitor, position: OverlayPosition, text_scale: f64) -> Rect {
    let factor = monitor.scale * text_scale;
    let area = monitor.work_area;
    let width = ((WINDOW_WIDTH * factor).round() as u32).min(area.width);
    let height = ((WINDOW_HEIGHT * factor).round() as u32).min(area.height);
    let x = area.x + (area.width - width) as i32 / 2;
    let y = match position {
        OverlayPosition::Bottom => {
            area.bottom() - height as i32 - (BOTTOM_MARGIN * monitor.scale).round() as i32
        }
        OverlayPosition::Top => area.y + (TOP_MARGIN * monitor.scale).round() as i32,
    };
    Rect {
        x,
        y: y.max(area.y),
        width,
        height,
    }
}

/// The window region that clips the Overlay to its pill (rule 11), in physical pixels relative
/// to the window: GDI's exclusive `right`/`bottom`, and the corner ellipse's diameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PillRegion {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub corner: i32,
}

/// The region for a pill of `pill` (width, height) CSS pixels drawn at `factor` (monitor scaling
/// × "Text size") in a window of `window` (width, height) physical pixels: centred, with a pixel
/// to spare all round so the pill's anti-aliased edge stays visible, never beyond the window.
pub fn pill_region(pill: (f64, f64), factor: f64, window: (u32, u32)) -> PillRegion {
    let (window_width, window_height) = (window.0 as i32, window.1 as i32);
    let width = ((pill.0 * factor).ceil() as i32 + 2).min(window_width);
    let height = ((pill.1 * factor).ceil() as i32 + 2).min(window_height);
    let left = (window_width - width) / 2;
    let top = (window_height - height) / 2;
    PillRegion {
        left,
        top,
        right: left + width + 1,
        bottom: top + height + 1,
        corner: height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The region covers the scaled pill, centred in the window.
    fn assert_covers(region: PillRegion, pill: (f64, f64), factor: f64, window: Rect) {
        let (width, height) = (pill.0 * factor, pill.1 * factor);
        let centre = f64::from(window.width) / 2.0;
        assert!(f64::from(region.left) <= centre - width / 2.0, "{region:?}");
        assert!(f64::from(region.right) >= centre + width / 2.0, "{region:?}");
        assert!(region.right <= window.width as i32 + 1, "{region:?}");
        assert!(f64::from(region.bottom - region.top) >= height.min(f64::from(window.height)));
    }

    // Rule 11 with rule 15: at "Text size" 125 % the region grows with the window it is shown in.
    #[test]
    fn the_pill_region_follows_text_size() {
        let window = place(&PRIMARY, OverlayPosition::Bottom, 1.25);
        let region = pill_region((258.0, 44.0), 1.25, (window.width, window.height));
        assert_covers(region, (258.0, 44.0), 1.25, window);
        assert_eq!(region.left, (650 - 325) / 2);
        assert_eq!(region.right - region.left, 326);
    }

    // Rule 11 with rule 15: on a 150 % monitor the region matches that monitor's scaling.
    #[test]
    fn the_pill_region_follows_monitor_scaling() {
        let window = place(&LEFT, OverlayPosition::Bottom, 1.0);
        let region = pill_region((258.0, 44.0), 1.5, (window.width, window.height));
        assert_covers(region, (258.0, 44.0), 1.5, window);
        assert_eq!((region.left, region.right), (195, 195 + 389 + 1));
        assert_eq!((region.top, region.bottom, region.corner), (0, 67, 66));
    }

    #[test]
    fn the_pill_region_never_exceeds_the_window() {
        let region = pill_region((900.0, 80.0), 1.0, (520, 44));
        assert_eq!(
            region,
            PillRegion {
                left: 0,
                top: 0,
                right: 521,
                bottom: 45,
                corner: 44
            }
        );
    }

    const PRIMARY: Monitor = Monitor {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        work_area: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1032,
        },
        scale: 1.0,
    };
    /// A 2560×1440 monitor to the left of the primary at 150 %, taskbar 72 px high.
    const LEFT: Monitor = Monitor {
        bounds: Rect {
            x: -2560,
            y: 0,
            width: 2560,
            height: 1440,
        },
        work_area: Rect {
            x: -2560,
            y: 0,
            width: 2560,
            height: 1368,
        },
        scale: 1.5,
    };

    // Acceptance test 7.
    #[test]
    fn bottom_is_centred_and_above_the_taskbar_at_150_percent() {
        let r = place(&LEFT, OverlayPosition::Bottom, 1.0);
        assert_eq!(r.width, 780);
        assert_eq!(r.height, 66);
        assert_eq!(r.x - LEFT.work_area.x, LEFT.work_area.right() - r.right());
        assert_eq!(r.bottom(), LEFT.work_area.bottom() - 18);
        assert!(r.y >= LEFT.bounds.y);
    }

    #[test]
    fn top_sits_just_below_the_top_edge() {
        let r = place(&PRIMARY, OverlayPosition::Top, 1.0);
        assert_eq!(r.y, 8);
        assert_eq!(r.x, (1920 - 520) / 2);
    }

    // Rule 15: the Windows "Text size" setting scales the Overlay too.
    #[test]
    fn text_size_scales_the_window() {
        let r = place(&PRIMARY, OverlayPosition::Bottom, 1.5);
        assert_eq!((r.width, r.height), (780, 66));
        assert_eq!(r.bottom(), 1032 - 12);
    }

    // Acceptance test 6.
    #[test]
    fn the_monitor_under_the_pointer_is_chosen() {
        let monitors = [PRIMARY, LEFT];
        assert_eq!(monitor_at(&monitors, (-100, 700)), Some(LEFT));
        assert_eq!(monitor_at(&monitors, (100, 700)), Some(PRIMARY));
        assert_eq!(monitor_at(&monitors, (99_999, 0)), Some(PRIMARY));
        assert_eq!(monitor_at(&[], (0, 0)), None);
    }
}
