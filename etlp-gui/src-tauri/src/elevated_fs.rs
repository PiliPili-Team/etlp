//! Privileged filesystem fallbacks for Windows-only write operations.
//!
//! Normal writes always run in-process first. These helpers are used only after
//! a write fails with permission denied, so portable installs in protected
//! directories can still clear logs/cache or save config after a UAC prompt.

use std::path::Path;

#[cfg(target_os = "windows")]
fn quote_ps_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn run_powershell_elevated(script: &str) -> Result<(), String> {
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "$p = Start-Process -Verb RunAs -Wait -PassThru -FilePath powershell.exe \
                 -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass',\
                 '-Command', {}); exit $p.ExitCode",
                quote_ps_string(script)
            ),
        ])
        .status()
        .map_err(|e| format!("request elevation: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "elevated operation failed with code {}",
            status.code().unwrap_or(-1)
        ))
    }
}

#[cfg(target_os = "windows")]
pub fn truncate_file(path: &Path) -> Result<(), String> {
    let target = quote_ps_string(&path.to_string_lossy());
    run_powershell_elevated(&format!(
        "if (Test-Path -LiteralPath {target}) {{ \
         $fs = [System.IO.File]::Open({target}, 'Open', 'Write', 'ReadWrite'); \
         try {{ $fs.SetLength(0) }} finally {{ $fs.Dispose() }} \
         }}"
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn truncate_file(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("elevation is only supported on Windows".to_owned())
}

#[cfg(target_os = "windows")]
pub fn remove_file(path: &Path) -> Result<(), String> {
    let target = quote_ps_string(&path.to_string_lossy());
    run_powershell_elevated(&format!(
        "if (Test-Path -LiteralPath {target}) {{ \
         Remove-Item -LiteralPath {target} -Force \
         }}"
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn remove_file(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("elevation is only supported on Windows".to_owned())
}

#[cfg(target_os = "windows")]
pub fn remove_dir_all(path: &Path) -> Result<(), String> {
    let target = quote_ps_string(&path.to_string_lossy());
    run_powershell_elevated(&format!(
        "if (Test-Path -LiteralPath {target}) {{ \
         Remove-Item -LiteralPath {target} -Recurse -Force \
         }}"
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn remove_dir_all(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("elevation is only supported on Windows".to_owned())
}

#[cfg(target_os = "windows")]
pub fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    let source = quote_ps_string(&source.to_string_lossy());
    let target = quote_ps_string(&target.to_string_lossy());
    run_powershell_elevated(&format!(
        "$parent = Split-Path -LiteralPath {target}; \
         if ($parent) {{ New-Item -ItemType Directory -Force -Path $parent | Out-Null }}; \
         Copy-Item -LiteralPath {source} -Destination {target} -Force"
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    let _ = (source, target);
    Err("elevation is only supported on Windows".to_owned())
}
