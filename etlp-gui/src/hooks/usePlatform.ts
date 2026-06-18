import { platform as getPlatform } from "@tauri-apps/plugin-os";

export type Platform = "macos" | "windows" | "linux" | "unknown";

// platform() is synchronous in @tauri-apps/plugin-os v2.
let _cached: Platform | null = null;

export function usePlatform(): Platform {
    if (_cached !== null) return _cached;
    try {
        const p = getPlatform();
        if (p === "macos" || p === "windows" || p === "linux") {
            _cached = p;
        } else {
            _cached = "unknown";
        }
    } catch {
        _cached = "unknown";
    }
    return _cached;
}
