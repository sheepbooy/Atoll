use super::*;

#[test]
fn tray_contains_only_show_and_quit() {
    assert_eq!(
        tray_menu_entries(),
        [("show", "Show Atoll"), ("quit", "Quit")]
    );
}
