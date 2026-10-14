//! Right-edge geometry, shared by snaps, resize animations and cosmetic pulses.
//! Pure calculations are compiled on every host for regression tests.
use tauri::{LogicalPosition, LogicalSize, Monitor, PhysicalPosition, PhysicalSize};

pub(crate) const DEFAULT_EDGE_POSITION: f64 = 1.0 / 3.0;

pub(crate) fn normalize_edge_position(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        DEFAULT_EDGE_POSITION
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EdgeAnchor {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub ratio: f64,
    pub scale_factor: f64,
}

impl EdgeAnchor {
    pub fn for_monitor(monitor: &Monitor, ratio: f64) -> Self {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        Self {
            left: area.position.x as f64 / scale,
            top: area.position.y as f64 / scale,
            width: area.size.width as f64 / scale,
            height: area.size.height as f64 / scale,
            ratio: normalize_edge_position(ratio),
            scale_factor: scale,
        }
    }

    pub fn size(self, size: LogicalSize<f64>) -> LogicalSize<f64> {
        LogicalSize::new(
            size.width.min(self.width).max(1.0),
            size.height.min(self.height).max(1.0),
        )
    }

    pub fn position(self, size: LogicalSize<f64>) -> LogicalPosition<f64> {
        let size = self.size(size);
        let resting_y = self.top + self.ratio * (self.height - crate::MICRO_WINDOW_HEIGHT).max(0.0);
        LogicalPosition::new(
            self.left + self.width - size.width,
            resting_y
                .min(self.top + self.height - size.height)
                .max(self.top),
        )
    }

    pub fn dragged_ratio(self, start_ratio: f64, delta_physical_y: f64) -> f64 {
        let travel = (self.height - crate::MICRO_WINDOW_HEIGHT).max(1.0) * self.scale_factor;
        normalize_edge_position(start_ratio + delta_physical_y / travel)
    }

    pub fn physical_frame(
        self,
        y: f64,
        size: LogicalSize<f64>,
    ) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
        let size = self.size(size).to_physical::<u32>(self.scale_factor);
        let right = ((self.left + self.width) * self.scale_factor).round() as i32;
        let top = (self.top * self.scale_factor).round() as i32;
        let bottom = ((self.top + self.height) * self.scale_factor).round() as i32;
        let y = (y * self.scale_factor).round() as i32;
        (
            PhysicalPosition::new(
                right - size.width as i32,
                y.clamp(top, (bottom - size.height as i32).max(top)),
            ),
            size,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn area(ratio: f64) -> EdgeAnchor {
        EdgeAnchor {
            left: -1920.0,
            top: 40.0,
            width: 1920.0,
            height: 1040.0,
            ratio,
            scale_factor: 1.5,
        }
    }
    #[test]
    fn right_edge_stays_fixed_for_all_sizes_and_pulses() {
        for width in [72.0, 560.0, 680.0, 712.0] {
            let size = LogicalSize::new(width, 320.0);
            assert_eq!(area(0.33).position(size).x + width, 0.0);
        }
    }
    #[test]
    fn expansion_clamps_up_without_changing_saved_position() {
        let anchor = area(1.0);
        assert_eq!(anchor.position(LogicalSize::new(72.0, 32.0)).y, 1048.0);
        assert_eq!(anchor.position(LogicalSize::new(680.0, 680.0)).y, 400.0);
        assert_eq!(anchor.ratio, 1.0);
    }
    #[test]
    fn small_work_area_clamps_both_dimensions() {
        let anchor = EdgeAnchor {
            width: 480.0,
            height: 600.0,
            ..area(0.5)
        };
        let size = anchor.size(LogicalSize::new(680.0, 680.0));
        assert_eq!(size, LogicalSize::new(480.0, 600.0));
        assert_eq!(anchor.position(size), LogicalPosition::new(-1920.0, 40.0));
    }
    #[test]
    fn physical_drag_accounts_for_display_scale_and_limits() {
        let anchor = area(0.0);
        assert_eq!(anchor.dragged_ratio(0.0, 1512.0), 1.0);
        assert_eq!(anchor.dragged_ratio(0.0, -100.0), 0.0);
        assert_eq!(normalize_edge_position(f64::NAN), DEFAULT_EDGE_POSITION);
    }
    #[test]
    fn fractional_scale_frames_never_drift_or_cross_the_taskbar() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let anchor = EdgeAnchor {
                scale_factor: scale,
                ..area(1.0)
            };
            for width in [72.0, 90.5 / scale, 560.0, 680.0] {
                let (position, size) =
                    anchor.physical_frame(1080.0, LogicalSize::new(width, 320.5));
                assert_eq!(position.x + size.width as i32, 0);
                assert!(position.y + size.height as i32 <= (1080.0 * scale).round() as i32);
            }
        }
    }
}
