use super::*;
use tauri::LogicalSize;

/// A display has a camera housing ("notch") when the two menu-bar halves
/// (auxiliary top areas) don't span the full screen width — the gap between
/// them is the notch.
#[cfg(test)]
fn has_camera_housing(frame_width: f64, aux_left_width: f64, aux_right_width: f64) -> bool {
    aux_left_width > 0.0
        && aux_right_width > 0.0
        && aux_left_width + aux_right_width < frame_width - 1.0
}

/// Notch width in logical points, derived from the gap between the auxiliary
/// menu-bar areas (matches ping-island's detection). Falls back when the
/// auxiliary areas are unavailable.
#[cfg(test)]
fn notch_logical_width(
    frame_width: f64,
    aux_left_width: f64,
    aux_right_width: f64,
    fallback: f64,
) -> f64 {
    if aux_left_width > 0.0 && aux_right_width > 0.0 {
        let detected = (frame_width - aux_left_width - aux_right_width + 4.0).ceil();
        detected.max(fallback)
    } else {
        fallback
    }
}

#[test]
fn approval_notification_copy_summarizes_command_and_project() {
    let (title_en, body_en) = approval_notification_copy(
        "Claude",
        "git push --force origin main\nsecond line",
        "/Users/dev/Atoll",
        "en",
    );
    assert_eq!(title_en, "Claude requests approval");
    assert!(body_en.starts_with("git push --force origin main"));
    assert!(body_en.contains("Atoll"));

    let (title_zh, body_zh) =
        approval_notification_copy("Claude", "rm -rf /tmp/x", "/home/dev/Atoll", "zh-CN");
    assert_eq!(title_zh, "Claude 请求批准");
    assert!(body_zh.contains("rm -rf /tmp/x"));
}

#[test]
fn approval_notification_copy_truncates_long_commands() {
    let long_command = "echo ".repeat(80);
    let (_, body) = approval_notification_copy("Claude", &long_command, "/tmp/x", "en");
    assert!(body.chars().count() < 200);
    assert!(body.ends_with('…') || body.contains('…'));
}

#[test]
fn expanded_window_is_560_by_320() {
    let size = island_window_logical_size(
        IslandWindowMode::Expanded,
        COMPACT_WINDOW_WIDTH,
        NotchMetrics::default(),
        false,
        false,
        false,
    );

    assert_eq!(size, LogicalSize::new(560.0, 320.0));
}

#[test]
fn expanded_idle_window_is_shorter() {
    let size = island_window_logical_size(
        IslandWindowMode::Expanded,
        COMPACT_WINDOW_WIDTH,
        NotchMetrics::default(),
        true,
        false,
        false,
    );

    assert_eq!(size, LogicalSize::new(560.0, EXPANDED_IDLE_WINDOW_HEIGHT));
}

#[test]
fn expanded_plan_window_is_taller() {
    let size = island_window_logical_size(
        IslandWindowMode::Expanded,
        COMPACT_WINDOW_WIDTH,
        NotchMetrics::default(),
        false,
        true,
        false,
    );

    assert_eq!(
        size,
        LogicalSize::new(EXPANDED_PLAN_WINDOW_WIDTH, EXPANDED_PLAN_WINDOW_HEIGHT)
    );
}

#[test]
fn expanded_settings_window_is_larger() {
    let size = island_window_logical_size(
        IslandWindowMode::Expanded,
        COMPACT_WINDOW_WIDTH,
        NotchMetrics::default(),
        false,
        false,
        true,
    );

    assert_eq!(
        size,
        LogicalSize::new(
            EXPANDED_SETTINGS_WINDOW_WIDTH,
            EXPANDED_SETTINGS_WINDOW_HEIGHT,
        )
    );
}

#[test]
fn window_animation_interpolates_to_exact_endpoints() {
    assert_eq!(interpolate_f64(132.0, 560.0, ease_out_cubic(0.0)), 132.0);
    assert_eq!(interpolate_f64(132.0, 560.0, ease_out_cubic(1.0)), 560.0);
    assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(0.0)), 100.0);
    assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(1.0)), -20.0);
    assert!((ease_out_spring(0.0) - 0.0).abs() < 1e-9);
    assert!((ease_out_spring(1.0) - 1.0).abs() < 1e-9);
    // Mild overshoot: mid-late progress exceeds 1.0 briefly.
    assert!(ease_out_spring(0.75) > 1.0);
    assert!(ease_out_spring(0.75) < 1.08);
    // Monotonic approach after the overshoot peak, no re-dip below target
    // at the very end (single clean settle).
    assert!(ease_out_spring(0.95) >= 0.999);
    assert!(ease_out_spring(0.95) <= ease_out_spring(0.75) + 1e-6);
    // Fast launch: reaches half the distance in under a quarter of the
    // animation window, keeping the snappy start of the old back-ease.
    assert!(ease_out_spring(0.2) > 0.5);
}

#[test]
fn animation_duration_resolves_per_call_with_default_and_clamp() {
    // Default: the signature 420ms window spring.
    assert_eq!(resolve_animation_duration(None), Duration::from_millis(420));
    assert_eq!(
        resolve_animation_duration(Some(0)),
        Duration::from_millis(420)
    );
    // Per-call override (fast file-drag expansion).
    assert_eq!(
        resolve_animation_duration(Some(140)),
        Duration::from_millis(140)
    );
    // Absurd values are clamped so a bad caller cannot wedge the loop.
    assert_eq!(
        resolve_animation_duration(Some(10_000)),
        Duration::from_millis(2000)
    );
}

#[test]
fn shape_pulse_easing_hits_endpoints_for_both_phases() {
    // Gulp: spring out 基帧→峰值帧，cubic 峰值帧→基帧。两段各自插值，
    // 手点处必须连续（out 末端 = back 起点 = 峰值帧），末端精确回基帧。
    let out = Duration::from_millis(150);
    let back = Duration::from_millis(240);
    let start = 240.0f64;
    let peak = 252.0f64;
    let height_at = |elapsed: Duration| -> f64 {
        if elapsed < out {
            interpolate_f64(
                start,
                peak,
                ease_out_spring(elapsed.as_secs_f64() / out.as_secs_f64()),
            )
        } else {
            interpolate_f64(
                peak,
                start,
                ease_out_cubic(((elapsed - out).as_secs_f64() / back.as_secs_f64()).min(1.0)),
            )
        }
    };
    assert_eq!(height_at(Duration::ZERO), start);
    // Approaching the handoff from the out phase lands on the peak...
    assert!((height_at(out - Duration::from_nanos(1)) - peak).abs() < 0.01);
    // ...and the first back frame continues from the peak, no jump.
    assert!((height_at(out) - peak).abs() < 1e-9);
    assert_eq!(height_at(out + back), start);
}

#[test]
fn camera_housing_is_detected_from_auxiliary_top_areas() {
    // Notch present: the menu-bar halves leave a gap (the housing).
    assert!(has_camera_housing(1512.0, 700.0, 700.0));
    // No notch: the halves span the full width.
    assert!(!has_camera_housing(1512.0, 756.0, 756.0));
    // Missing auxiliary areas are treated as "no notch".
    assert!(!has_camera_housing(1512.0, 0.0, 0.0));
}

#[test]
fn notch_width_never_drops_below_the_fallback_floor() {
    // 1512 - 700 - 700 + 4 = 116, clamped up to the fallback floor.
    assert_eq!(
        notch_logical_width(1512.0, 700.0, 700.0, FALLBACK_NOTCH_WIDTH),
        FALLBACK_NOTCH_WIDTH
    );
    // A wider gap is reported verbatim once it exceeds the floor.
    assert_eq!(notch_logical_width(1512.0, 600.0, 600.0, 200.0), 316.0);
    // Without auxiliary areas we fall back.
    assert_eq!(notch_logical_width(1512.0, 0.0, 0.0, 200.0), 200.0);
}

#[test]
fn notched_display_widens_to_notch_width() {
    let notch = NotchMetrics {
        has_notch: true,
        width: 200.0,
        height: 38.0,
        ..NotchMetrics::default()
    };
    let compact =
        island_window_logical_size(IslandWindowMode::Compact, 132.0, notch, false, false, false);
    // Compact sits in the menu-bar band (like dormant) — no extra_top. Its
    // height matches the notch so the pill bottom is flush with the housing.
    assert_eq!(compact.height, 38.0);
    // Width is clamped up to the notch width so the capsule visually
    // fuses with the camera housing (Dynamic-Island style).
    assert_eq!(compact.width, 200.0);

    // Content wider than the notch keeps its own width.
    let wide =
        island_window_logical_size(IslandWindowMode::Compact, 300.0, notch, false, false, false);
    assert_eq!(wide.width, 300.0);

    // Dormant is slightly wider than the notch (padding on each side).
    let dormant =
        island_window_logical_size(IslandWindowMode::Dormant, 132.0, notch, false, false, false);
    assert_eq!(dormant.width, 200.0 + 2.0 * DORMANT_NOTCH_PADDING);
    assert_eq!(dormant.height, 38.0);
}

#[test]
fn dormant_window_is_centered_on_notched_displays() {
    let notch = NotchMetrics {
        has_notch: true,
        width: 200.0,
        height: 38.0,
        ..NotchMetrics::default()
    };
    let center_x = 756.0;
    let dormant_width = 200.0 + 2.0 * DORMANT_NOTCH_PADDING;
    let origin = compact_window_origin_x(
        center_x,
        dormant_width,
        notch,
        0.0,
        IslandWindowMode::Dormant,
    );
    assert_eq!(origin, center_x - dormant_width / 2.0);
}

#[test]
fn compact_window_anchors_left_column_before_the_notch() {
    let notch = NotchMetrics {
        has_notch: true,
        width: 200.0,
        height: 38.0,
        ..NotchMetrics::default()
    };
    let center_x = 756.0;
    let left_pane = 58.0;
    let origin =
        compact_window_origin_x(center_x, 460.0, notch, left_pane, IslandWindowMode::Compact);
    assert_eq!(origin, center_x - notch.width / 2.0 - left_pane);
}

#[test]
fn non_notched_display_uses_minimum_comfortable_width() {
    let no_notch = NotchMetrics::default();

    // Compact: content width is kept as-is on non-notched displays.
    let compact = island_window_logical_size(
        IslandWindowMode::Compact,
        132.0,
        no_notch,
        false,
        false,
        false,
    );
    assert_eq!(compact.width, 132.0);
    // Without a detectable notch the standard notch band height applies.
    assert_eq!(compact.height, NOTCH_BAND_HEIGHT);

    // A compact_width that already exceeds the floor is kept as-is.
    let wide = island_window_logical_size(
        IslandWindowMode::Compact,
        250.0,
        no_notch,
        false,
        false,
        false,
    );
    assert_eq!(wide.width, 250.0);

    // Dormant: uses the same FALLBACK_NOTCH_WIDTH reference + padding.
    let dormant = island_window_logical_size(
        IslandWindowMode::Dormant,
        132.0,
        no_notch,
        false,
        false,
        false,
    );
    assert_eq!(
        dormant.width,
        FALLBACK_NOTCH_WIDTH + 2.0 * DORMANT_NOTCH_PADDING
    );
    assert_eq!(dormant.height, NOTCH_BAND_HEIGHT);
}

#[test]
fn collapsed_band_height_and_corner_radius_prefer_live_metrics() {
    // Live notch height wins on notched displays (bottom flush with the
    // housing).
    let notched = NotchMetrics {
        has_notch: true,
        height: 38.0,
        ..NotchMetrics::default()
    };
    assert_eq!(collapsed_band_height(&notched), 38.0);

    // Everywhere else: the standard notch band height, uniformly.
    assert_eq!(
        collapsed_band_height(&NotchMetrics::default()),
        NOTCH_BAND_HEIGHT
    );

    // A zero-height notch report also falls back to the band constant.
    let empty_notch = NotchMetrics {
        has_notch: true,
        ..NotchMetrics::default()
    };
    assert_eq!(collapsed_band_height(&empty_notch), NOTCH_BAND_HEIGHT);

    // Corner radius: calibrated value when present, notch fallback otherwise.
    let calibrated = NotchMetrics {
        corner_radius: 9.5,
        ..NotchMetrics::default()
    };
    assert_eq!(collapsed_corner_radius(&calibrated), 9.5);
    assert_eq!(
        collapsed_corner_radius(&NotchMetrics::default()),
        FALLBACK_NOTCH_CORNER_RADIUS
    );
}

#[test]
fn micro_window_is_a_thin_top_strip() {
    let wide = island_window_logical_size(
        IslandWindowMode::Micro,
        104.0,
        NotchMetrics::default(),
        false,
        false,
        false,
    );
    assert_eq!(wide.width, 104.0);
    assert_eq!(wide.height, MICRO_WINDOW_HEIGHT);
    let narrow = island_window_logical_size(
        IslandWindowMode::Micro,
        48.0,
        NotchMetrics::default(),
        false,
        false,
        false,
    );
    assert_eq!(narrow.width, MICRO_WINDOW_WIDTH);
}

#[test]
fn micro_presentation_width_does_not_clamp_to_saved_compact_width() {
    assert_eq!(
        resolve_presentation_width(IslandWindowMode::Micro, Some(104.0), 220.0),
        104.0
    );
    assert_eq!(
        resolve_presentation_width(IslandWindowMode::Micro, None, 220.0),
        MICRO_WINDOW_WIDTH
    );
    assert_eq!(
        resolve_presentation_width(IslandWindowMode::Compact, None, 220.0),
        220.0
    );
    assert_eq!(
        resolve_presentation_width(IslandWindowMode::Compact, Some(180.0), 220.0),
        180.0
    );
}

#[test]
fn micro_mode_skips_compact_width_persistence() {
    assert!(!should_persist_compact_width(IslandWindowMode::Micro));
    assert!(should_persist_compact_width(IslandWindowMode::Compact));
}

#[test]
fn collapsed_pass_through_includes_micro() {
    assert!(is_collapsed_pass_through_mode(IslandWindowMode::Micro));
    assert!(is_collapsed_pass_through_mode(IslandWindowMode::Compact));
    assert!(!is_collapsed_pass_through_mode(IslandWindowMode::Expanded));
}

#[test]
fn appkit_frame_places_the_window_at_the_screen_top() {
    fn appkit_window_origin_y(
        screen_origin_y: f64,
        screen_height: f64,
        window_height: f64,
        desired_top_y: f64,
        monitor_top_y: f64,
    ) -> f64 {
        screen_origin_y + screen_height - (desired_top_y - monitor_top_y) - window_height
    }

    assert_eq!(appkit_window_origin_y(0.0, 1260.0, 28.0, 0.0, 0.0), 1232.0);
    assert_eq!(appkit_window_origin_y(0.0, 1260.0, 320.0, 0.0, 0.0), 940.0);
}
