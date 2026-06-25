//! Platform-specific launch-at-login registration.

const BUNDLE_ID: &str = "com.atoll.agentisland";
const LEGACY_LAUNCH_AGENT: &str = "Atoll.plist";
const OPEN_LAUNCH_AGENT: &str = "com.atoll.agentisland.login.plist";

pub fn is_enabled() -> Result<bool, String> {
    platform_impl::is_enabled()
}

pub fn enable() -> Result<(), String> {
    platform_impl::enable()
}

pub fn disable() -> Result<(), String> {
    platform_impl::disable()
}

/// Remove broken LaunchAgent plists left by the old autostart plugin.
pub fn migrate_legacy_if_needed() {
    platform_impl::migrate_legacy_if_needed();
}

#[cfg(target_os = "macos")]
mod platform_impl {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use objc2::rc::Retained;
    use objc2_foundation::NSError;
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    use super::{BUNDLE_ID, LEGACY_LAUNCH_AGENT, OPEN_LAUNCH_AGENT};

    fn launch_agents_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join("Library/LaunchAgents"))
    }

    fn legacy_plist_path() -> Option<PathBuf> {
        launch_agents_dir().map(|dir| dir.join(LEGACY_LAUNCH_AGENT))
    }

    fn open_plist_path() -> Option<PathBuf> {
        launch_agents_dir().map(|dir| dir.join(OPEN_LAUNCH_AGENT))
    }

    fn gui_launchctl_domain() -> String {
        let uid = Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "501".into());
        format!("gui/{uid}")
    }

    fn bootout_launch_agent(path: &Path) {
        let Some(path) = path.to_str() else {
            return;
        };
        let _ = Command::new("launchctl")
            .args(["bootout", &gui_launchctl_domain(), path])
            .output();
    }

    fn remove_launch_agent(path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        bootout_launch_agent(path);
        fs::remove_file(path).map_err(|error| error.to_string())
    }

    fn remove_legacy_launch_agent() -> Result<(), String> {
        if let Some(path) = legacy_plist_path() {
            remove_launch_agent(&path)?;
        }
        Ok(())
    }

    fn remove_open_launch_agent() -> Result<(), String> {
        if let Some(path) = open_plist_path() {
            remove_launch_agent(&path)?;
        }
        Ok(())
    }

    fn running_from_app_bundle() -> bool {
        current_app_bundle_path().is_some()
    }

    fn current_app_bundle_path() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
        let exe = exe.display().to_string();
        let marker = ".app/";
        let index = exe.find(marker)?;
        Some(PathBuf::from(format!("{}.app", &exe[..index + 4])))
    }

    fn legacy_points_to_broken_target(path: &Path) -> bool {
        let Ok(contents) = fs::read_to_string(path) else {
            return false;
        };
        contents.contains("/target/debug/")
            || contents.contains("/target/release/")
            || (!contents.contains(".app") && contents.contains("<string>/"))
    }

    fn sm_app_service_available() -> bool {
        let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() else {
            return false;
        };
        let version = String::from_utf8_lossy(&output.stdout);
        let major = version
            .split('.')
            .next()
            .and_then(|part| part.parse::<u32>().ok())
            .unwrap_or(0);
        major >= 13
    }

    fn main_service() -> Retained<SMAppService> {
        unsafe { SMAppService::mainAppService() }
    }

    fn sm_status_is_enabled(status: SMAppServiceStatus) -> bool {
        matches!(
            status,
            SMAppServiceStatus::Enabled | SMAppServiceStatus::RequiresApproval
        )
    }

    fn format_service_error(error: Retained<NSError>) -> String {
        error.localizedDescription().to_string()
    }

    fn sm_is_enabled() -> Result<bool, String> {
        unsafe {
            let service = main_service();
            Ok(sm_status_is_enabled(service.status()))
        }
    }

    fn sm_enable() -> Result<(), String> {
        unsafe {
            let service = main_service();
            service
                .registerAndReturnError()
                .map_err(format_service_error)
        }
    }

    fn sm_disable() -> Result<(), String> {
        unsafe {
            let service = main_service();
            service
                .unregisterAndReturnError()
                .map_err(format_service_error)
        }
    }

    fn write_open_launch_agent() -> Result<(), String> {
        let path = open_plist_path().ok_or_else(|| "home directory unavailable".to_string())?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{BUNDLE_ID}.login</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/open</string>
    <string>-b</string>
    <string>{BUNDLE_ID}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>LimitLoadToSessionType</key>
  <array>
    <string>Aqua</string>
  </array>
</dict>
</plist>
"#
        );
        fs::write(&path, plist).map_err(|error| error.to_string())
    }

    pub fn migrate_legacy_if_needed() {
        if let Some(path) = legacy_plist_path() {
            if path.exists() && legacy_points_to_broken_target(&path) {
                let _ = remove_legacy_launch_agent();
            }
        }
    }

    pub fn is_enabled() -> Result<bool, String> {
        migrate_legacy_if_needed();
        if sm_app_service_available() {
            return sm_is_enabled();
        }
        Ok(open_plist_path().is_some_and(|path| path.exists()))
    }

    pub fn enable() -> Result<(), String> {
        if !running_from_app_bundle() {
            return Err(
                "Launch at login requires the installed Atoll.app. Download it from GitHub Releases and enable the setting there."
                    .into(),
            );
        }

        remove_legacy_launch_agent()?;
        remove_open_launch_agent()?;

        if sm_app_service_available() {
            return sm_enable();
        }

        write_open_launch_agent()
    }

    pub fn disable() -> Result<(), String> {
        remove_legacy_launch_agent()?;
        if sm_app_service_available() {
            let _ = sm_disable();
        }
        remove_open_launch_agent()
    }
}

#[cfg(target_os = "windows")]
mod platform_impl {
    use auto_launch::{AutoLaunch, AutoLaunchBuilder};
    use std::env::current_exe;

    fn manager() -> Result<AutoLaunch, String> {
        let app_path = current_exe().map_err(|error| error.to_string())?;
        AutoLaunchBuilder::new()
            .set_app_name("Atoll")
            .set_app_path(&app_path.display().to_string())
            .build()
            .map_err(|error| error.to_string())
    }

    pub fn migrate_legacy_if_needed() {}

    pub fn is_enabled() -> Result<bool, String> {
        manager()?.is_enabled().map_err(|error| error.to_string())
    }

    pub fn enable() -> Result<(), String> {
        manager()?.enable().map_err(|error| error.to_string())
    }

    pub fn disable() -> Result<(), String> {
        manager()?.disable().map_err(|error| error.to_string())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform_impl {
    pub fn migrate_legacy_if_needed() {}

    pub fn is_enabled() -> Result<bool, String> {
        Ok(false)
    }

    pub fn enable() -> Result<(), String> {
        Err("Launch at login is not supported on this platform.".into())
    }

    pub fn disable() -> Result<(), String> {
        Ok(())
    }
}
