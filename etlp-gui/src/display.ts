export type AccentColor =
    | "blue"
    | "indigo"
    | "purple"
    | "pink"
    | "red"
    | "orange"
    | "teal"
    | "green";

export type ThemeMode = "system" | "light" | "dark";
export type LangMode = "system" | "zh-CN" | "zh-TW" | "en";

export interface DisplaySettings {
    theme: ThemeMode;
    lang: LangMode;
    fontSize: number;
    zoom: number;
    fontFamily: string;
    accentColor: AccentColor;
}

/** [light-hex, dark-hex, soft-rgba] */
export const ACCENT_PALETTES: Record<AccentColor, [string, string, string]> = {
    blue: ["#007aff", "#0a84ff", "rgba(0,122,255,0.14)"],
    indigo: ["#5856d6", "#6065d3", "rgba(88,86,214,0.14)"],
    purple: ["#af52de", "#bf5af2", "rgba(175,82,222,0.14)"],
    pink: ["#ff2d78", "#ff375f", "rgba(255,45,120,0.14)"],
    red: ["#ff3b30", "#ff453a", "rgba(255,59,48,0.14)"],
    orange: ["#ff9500", "#ff9f0a", "rgba(255,149,0,0.14)"],
    teal: ["#5ac8fa", "#64d2ff", "rgba(90,200,250,0.14)"],
    green: ["#34c759", "#30d158", "rgba(52,199,89,0.14)"],
};

export function defaultDisplay(): DisplaySettings {
    return {
        theme: "system",
        lang: "system",
        fontSize: 13,
        zoom: 1,
        fontFamily: "",
        accentColor: "blue",
    };
}

export function loadDisplay(): DisplaySettings {
    try {
        const raw = localStorage.getItem("etlp-display");
        if (raw) return { ...defaultDisplay(), ...JSON.parse(raw) };
    } catch {
        /* ignore */
    }
    return defaultDisplay();
}

export function applyDisplay(s: DisplaySettings) {
    const root = document.documentElement;
    if (!s.theme || s.theme === "system") {
        root.removeAttribute("data-theme");
    } else {
        root.setAttribute("data-theme", s.theme);
    }
    root.style.setProperty("--base-font-size", `${s.fontSize}px`);
    root.style.setProperty("--app-zoom", String(s.zoom));
    root.style.setProperty(
        "--app-font",
        s.fontFamily ? `"${s.fontFamily}"` : "-apple-system",
    );
    const isDark =
        s.theme === "dark" ||
        (s.theme === "system" &&
            window.matchMedia("(prefers-color-scheme: dark)").matches);
    const [light, dark, soft] = ACCENT_PALETTES[s.accentColor ?? "blue"];
    root.style.setProperty("--accent", isDark ? dark : light);
    root.style.setProperty("--accent-soft", soft);
}
