//! Cross-platform process termination using the `sysinfo` crate.

use regex::Regex;
use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};
use tracing::info;

/// Kill all processes whose command line matches `name_pattern` but **not**
/// `exclude_pattern`.
///
/// Silently skips processes that cannot be killed (insufficient privileges,
/// already exited, etc.).
///
/// # Errors
/// Returns an error if either regex pattern is invalid.
pub fn kill_matching_processes(
    name_pattern: &str,
    exclude_pattern: &str,
) -> Result<(), regex::Error> {
    let name_re = Regex::new(name_pattern)?;
    let exclude_re: Option<Regex> = if exclude_pattern.is_empty() {
        None
    } else {
        Some(Regex::new(exclude_pattern)?)
    };

    let refresh = RefreshKind::nothing().with_processes(
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    let sys = System::new_with_specifics(refresh);

    for (pid, process) in sys.processes() {
        let cmd: String = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect::<Vec<String>>()
            .join(" ");

        if !name_re.is_match(&cmd) {
            continue;
        }
        if exclude_re.as_ref().is_some_and(|excl| excl.is_match(&cmd)) {
            continue;
        }
        info!("killing pid={} cmd={cmd:?}", pid.as_u32());
        process.kill();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_name_pattern_errors() {
        assert!(kill_matching_processes("[unclosed", "").is_err());
    }

    #[test]
    fn valid_patterns_do_not_panic() {
        let result = kill_matching_processes(
            r"nonexistent_etlp_process_xyz_12345",
            r"(tmux|grep)",
        );
        assert!(result.is_ok());
    }
}
