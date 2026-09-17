use super::*;

#[test]
fn approval_notice_mode_normalization_falls_back_to_interrupt() {
    assert_eq!(normalize_approval_notice_mode("notify"), "notify");
    assert_eq!(normalize_approval_notice_mode("interrupt"), "interrupt");
    assert_eq!(normalize_approval_notice_mode(""), "interrupt");
    assert_eq!(normalize_approval_notice_mode("yolo"), "interrupt");
}

#[test]
fn notification_language_normalization_falls_back_to_english() {
    assert_eq!(normalize_notification_language("zh-CN"), "zh-CN");
    assert_eq!(normalize_notification_language("en"), "en");
    assert_eq!(normalize_notification_language("fr"), "en");
}
