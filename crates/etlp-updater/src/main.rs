/// Windows Portable updater: waits for the main process to exit, copies
/// update files into the install dir, then re-launches the application.
///
/// Invoked by the main application before it calls `app.exit(0)`:
///   updater.exe --pid <PID> --update-dir <dir> --install-dir <dir> --exe <path>
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use clap::Parser;
use sysinfo::System;

#[derive(Parser)]
#[command(name = "updater", about = "ETLP Windows Portable updater")]
struct Args {
    /// PID of the main application to wait for.
    #[arg(long)]
    pid: u32,

    /// Directory containing the extracted update files.
    #[arg(long)]
    update_dir: PathBuf,

    /// Installation directory to copy files into.
    #[arg(long)]
    install_dir: PathBuf,

    /// Path to the main executable to relaunch after update.
    #[arg(long)]
    exe: PathBuf,
}

fn main() {
    let args = Args::parse();
    wait_for_pid(args.pid);
    match apply_update(&args.update_dir, &args.install_dir) {
        Ok(()) => {
            relaunch(&args.exe);
        }
        Err(e) => {
            eprintln!("[updater] update failed: {e}");
            // Still relaunch so the user is not left with no application.
            relaunch(&args.exe);
        }
    }
}

/// Polls sysinfo until the process with `pid` is no longer alive.
fn wait_for_pid(pid: u32) {
    let target = sysinfo::Pid::from_u32(pid);
    loop {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, false);
        if sys.process(target).is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(300));
    }
}

/// Copies all files from `src` into `dest`, creating subdirectories as needed.
/// Returns `Err` on the first I/O failure; previously-written files are left
/// in place (partial update is still runnable on the old binaries that were
/// not yet overwritten).
fn apply_update(src: &Path, dest: &Path) -> Result<(), String> {
    copy_dir(src, dest, src)
}

fn copy_dir(src: &Path, dest: &Path, root: &Path) -> Result<(), String> {
    let entries = fs::read_dir(src)
        .map_err(|e| format!("read_dir {}: {e}", src.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let src_path = entry.path();

        // Build relative path from the update root to determine the
        // corresponding destination path.
        let relative = src_path
            .strip_prefix(root)
            .map_err(|e| format!("strip prefix: {e}"))?;

        // Zip-slip guard: reject any component that resolves up the tree.
        if relative.components().any(|c| {
            matches!(c, std::path::Component::ParentDir)
        }) {
            return Err(format!("unsafe path: {}", relative.display()));
        }

        let dest_path = dest.join(relative);

        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)
                .map_err(|e| format!("mkdir {}: {e}", dest_path.display()))?;
            copy_dir(&src_path, dest, root)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
            }
            fs::copy(&src_path, &dest_path).map_err(|e| {
                format!(
                    "copy {} -> {}: {e}",
                    src_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn relaunch(exe: &Path) {
    if let Err(e) = Command::new(exe).spawn() {
        eprintln!("[updater] failed to relaunch {}: {e}", exe.display());
    }
}
