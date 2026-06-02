//! §4.3 Layer-1 file-system encryption detection.
//!
//! The probe only **reports** the status; writing `user_setting.fs_encryption_status`, showing the
//! warning dialog, and the `audit.sqlite` `crypto_op` entry are all done by the api/audit/frontend
//! layers (§4.3.2). R1 ships Windows BitLocker deep (§0.1); macOS/Linux are skeletons that return
//! [`FsEncryptionStatus::Unknown`] until R2 (the brief §8).
//!
//! The result reuses `data_model::user_setting::FsEncryptionStatus` (one source of truth). Note the
//! probe never returns `DisabledWithAck` — that variant records a *user acknowledgement* and is only
//! ever written by the api layer (§4.3.2).

use data_model::user_setting::FsEncryptionStatus;

/// Detect the system-drive FS encryption status (`backend/04` §4.3.1).
///
/// - Windows: `Get-BitLockerVolume` `ProtectionStatus` (`On` → `Enabled`).
/// - macOS / Linux: skeleton returning `Unknown` (R2 — the brief §8). The function exists on all
///   targets so callers compile cross-platform; only Windows performs a real probe in R1.
#[cfg(target_os = "windows")]
#[must_use]
pub fn detect_fs_encryption() -> FsEncryptionStatus {
    use std::process::Command;
    // ProtectionStatus is "On" (1) when BitLocker protection is active on the system drive.
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-BitLockerVolume -MountPoint $env:SystemDrive).ProtectionStatus",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            let trimmed = s.trim();
            // PowerShell prints the enum value name ("On"/"Off") or its numeric form ("1"/"0").
            if trimmed.eq_ignore_ascii_case("on") || trimmed == "1" {
                FsEncryptionStatus::Enabled
            } else {
                FsEncryptionStatus::Disabled
            }
        }
        // Command ran but returned non-zero (e.g. BitLocker cmdlets unavailable on Home), or the
        // command could not be spawned at all → we could not confirm.
        Ok(_) | Err(_) => FsEncryptionStatus::Unknown,
    }
}

/// macOS skeleton — returns `Unknown` (R2 will parse `fdesetup status`, the brief §8).
#[cfg(target_os = "macos")]
#[must_use]
pub fn detect_fs_encryption() -> FsEncryptionStatus {
    FsEncryptionStatus::Unknown
}

/// Linux skeleton — returns `Unknown` (R2 will parse `/proc/mounts` + LUKS, the brief §8).
#[cfg(target_os = "linux")]
#[must_use]
pub fn detect_fs_encryption() -> FsEncryptionStatus {
    FsEncryptionStatus::Unknown
}

/// Fallback for any other target — `Unknown`.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
#[must_use]
pub fn detect_fs_encryption() -> FsEncryptionStatus {
    FsEncryptionStatus::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CR-09: the probe always returns one of the three real statuses (never `DisabledWithAck`,
    /// which is an api-layer-only acknowledgement). On a Windows CI runner this exercises the real
    /// BitLocker command; on other platforms it is the skeleton.
    #[test]
    fn detect_returns_a_valid_status() {
        let s = detect_fs_encryption();
        assert!(matches!(
            s,
            FsEncryptionStatus::Enabled
                | FsEncryptionStatus::Disabled
                | FsEncryptionStatus::Unknown
        ));
        assert_ne!(
            s,
            FsEncryptionStatus::DisabledWithAck,
            "the probe must never synthesize the user-ack variant"
        );
    }

    /// Non-Windows skeletons report `Unknown` (R1 only Windows is deep).
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn non_windows_is_unknown_skeleton() {
        assert_eq!(detect_fs_encryption(), FsEncryptionStatus::Unknown);
    }
}
