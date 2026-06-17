//! Supported local players.

/// The set of media players etlp can drive.
///
/// Mirrors the keys of the Python `start_player_func_dict`. `iina` is a macOS
/// front-end for mpv and shares mpv's control path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerKind {
    Mpv,
    Iina,
    Vlc,
    Mpc,
    PotPlayer,
    DandanPlay,
}

impl PlayerKind {
    /// Canonical lowercase name, matching the Python dispatch keys.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PlayerKind::Mpv => "mpv",
            PlayerKind::Iina => "iina",
            PlayerKind::Vlc => "vlc",
            PlayerKind::Mpc => "mpc",
            PlayerKind::PotPlayer => "potplayer",
            PlayerKind::DandanPlay => "dandanplay",
        }
    }

    /// Whether this player is controlled through the mpv JSON IPC protocol.
    #[must_use]
    pub fn is_mpv_family(self) -> bool {
        matches!(self, PlayerKind::Mpv | PlayerKind::Iina)
    }

    /// Detect the player from an executable path, mirroring the Python logic
    /// of substring-matching the lowercased player path. `ddplay` is treated
    /// as an alias for `dandanplay`.
    ///
    /// Returns the first match in a fixed priority order so that, e.g.,
    /// a path containing both is resolved deterministically.
    #[must_use]
    pub fn detect_from_path(path: &str) -> Option<PlayerKind> {
        let lower = path.to_lowercase();
        // Order matters: longer / more specific aliases first.
        const CANDIDATES: &[(&str, PlayerKind)] = &[
            ("dandanplay", PlayerKind::DandanPlay),
            ("ddplay", PlayerKind::DandanPlay),
            ("potplayer", PlayerKind::PotPlayer),
            ("iina", PlayerKind::Iina),
            ("mpv", PlayerKind::Mpv),
            ("vlc", PlayerKind::Vlc),
            ("mpc", PlayerKind::Mpc),
        ];
        CANDIDATES
            .iter()
            .find(|(needle, _)| lower.contains(needle))
            .map(|(_, kind)| *kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_common_players() {
        assert_eq!(
            PlayerKind::detect_from_path("/opt/homebrew/bin/mpv"),
            Some(PlayerKind::Mpv)
        );
        assert_eq!(
            PlayerKind::detect_from_path(
                r"C:\Program Files\PotPlayer\PotPlayerMini64.exe"
            ),
            Some(PlayerKind::PotPlayer)
        );
        assert_eq!(
            PlayerKind::detect_from_path("/Applications/IINA.app/iina-cli"),
            Some(PlayerKind::Iina)
        );
    }

    #[test]
    fn ddplay_aliases_to_dandanplay() {
        assert_eq!(
            PlayerKind::detect_from_path(r"D:\ddplay\ddplay.exe"),
            Some(PlayerKind::DandanPlay)
        );
    }

    #[test]
    fn unknown_player_returns_none() {
        assert_eq!(PlayerKind::detect_from_path("/usr/bin/totem"), None);
    }

    #[test]
    fn iina_is_mpv_family() {
        assert!(PlayerKind::Iina.is_mpv_family());
        assert!(PlayerKind::Mpv.is_mpv_family());
        assert!(!PlayerKind::Vlc.is_mpv_family());
    }
}
