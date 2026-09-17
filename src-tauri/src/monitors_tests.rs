use super::*;

#[test]
fn paused_position_freezes_creep_but_allows_seeks() {
    // Mirrors QQ Music behavior measured live: elapsedTime keeps
    // advancing while paused and snaps back on resume.
    let mut held: Option<f64> = None;
    let mut prev_raw: Option<f64> = None;
    let step = |raw, playing, prev, held: &mut Option<f64>| {
        sanitize_paused_position(Some(raw), playing, prev, held)
    };

    // Playing positions pass through.
    assert_eq!(step(71.2, true, prev_raw, &mut held), Some(71.2));
    prev_raw = Some(71.2);

    // First paused sample lands within the last playing position's
    // creep window — held at the last playing value.
    assert_eq!(step(71.8, false, prev_raw, &mut held), Some(71.2));
    prev_raw = Some(71.8);

    // Subsequent paused creep stays frozen.
    assert_eq!(step(72.9, false, prev_raw, &mut held), Some(71.2));
    prev_raw = Some(72.9);
    assert_eq!(step(73.99, false, prev_raw, &mut held), Some(71.2));
    prev_raw = Some(73.99);

    // A forward seek while paused (≥2s in one poll) is adopted.
    assert_eq!(step(100.0, false, prev_raw, &mut held), Some(100.0));
    prev_raw = Some(100.0);

    // A backward seek while paused is adopted.
    assert_eq!(step(50.0, false, prev_raw, &mut held), Some(50.0));
    prev_raw = Some(50.0);

    // Resume passes the snapped-back true position through.
    assert_eq!(step(71.0, true, prev_raw, &mut held), Some(71.0));
}

#[test]
fn paused_position_none_and_cold_start_pass_through() {
    let mut held: Option<f64> = None;
    // No history: adopt whatever arrives.
    assert_eq!(
        sanitize_paused_position(Some(30.0), false, None, &mut held),
        Some(30.0)
    );
    // None (player omitted elapsedTime) passes through as None.
    assert_eq!(
        sanitize_paused_position(None, false, Some(30.0), &mut held),
        None
    );
    assert_eq!(held, None);
}
