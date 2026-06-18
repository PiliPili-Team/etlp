export type Platform = "macos" | "windows" | "linux" | "unknown";

let _cached: Platform | null = null;

function detectFromUA(): Platform {
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("mac os x") || ua.includes("macintosh")) return "macos";
    if (ua.includes("windows nt")) return "windows";
    if (ua.includes("linux")) return "linux";
    return "unknown";
}

export function usePlatform(): Platform {
    if (_cached === null) {
        _cached = detectFromUA();
    }
    return _cached;
}
