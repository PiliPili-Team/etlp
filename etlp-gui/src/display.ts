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
export type LangMode =
    | "system"
    | "zh-CN"
    | "zh-TW"
    | "en"
    | "ja"
    | "ko"
    | "de"
    | "it"
    | "fr"
    | "ar"
    | "es"
    | "ru"
    | "pt"
    | "sk"
    | "uk"
    | "sr"
    | "tr"
    | "he"
    | "th"
    | "pl"
    | "id";

/** Locales rendered right-to-left; drive `dir="rtl"` and mirrored layout. */
export const RTL_LOCALES: ReadonlySet<string> = new Set(["ar", "he"]);

/**
 * Resolve a concrete locale code from the chosen language mode, mapping
 * `system` onto the closest supported locale via the browser language tag.
 */
export function resolveLocale(lang: LangMode): string {
    if (lang !== "system") return lang;
    const sys = navigator.language.toLowerCase();
    if (/^zh-(tw|hk|mo)/.test(sys)) return "zh-TW";
    if (sys.startsWith("zh")) return "zh-CN";
    if (sys.startsWith("he") || sys.startsWith("iw")) return "he";
    if (sys.startsWith("id") || sys.startsWith("in")) return "id";
    const base = sys.split("-")[0];
    const supported = [
        "ja",
        "ko",
        "de",
        "it",
        "fr",
        "ar",
        "es",
        "ru",
        "pt",
        "sk",
        "uk",
        "sr",
        "tr",
        "th",
        "pl",
    ];
    return supported.includes(base) ? base : "en";
}

/** Whether the chosen language resolves to a right-to-left locale. */
export function isRTL(lang: LangMode): boolean {
    return RTL_LOCALES.has(resolveLocale(lang));
}

export interface DisplaySettings {
    theme: ThemeMode;
    lang: LangMode;
    fontSize: number;
    zoom: number;
    fontFamily: string;
    accentColor: AccentColor;
    /** Vertically center the sidebar nav items as a group (default: top-aligned). */
    centerNav: boolean;
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
        centerNav: false,
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
    // Light mode is not yet ready; force dark unconditionally.
    root.setAttribute("data-theme", "dark");
    root.setAttribute("dir", isRTL(s.lang) ? "rtl" : "ltr");

    // Express font-size as a zoom multiplier so ALL elements scale, including
    // those with hardcoded px values. Default font size (13) gives a multiplier
    // of 1 — backward-compatible with stored zoom preferences.
    const effectiveZoom = s.zoom * (s.fontSize / 13);
    root.style.setProperty("--base-font-size", `${s.fontSize}px`);
    root.style.setProperty("--app-zoom", String(effectiveZoom));

    // On Windows / Linux the platform body class overrides font-family; we
    // must set --app-font on :root so the var() in that rule resolves correctly.
    const fontCss = s.fontFamily ? `"${s.fontFamily}"` : "system-ui";
    root.style.setProperty("--app-font", fontCss);

    const [, dark, soft] = ACCENT_PALETTES[s.accentColor ?? "blue"];
    root.style.setProperty("--accent", dark);
    root.style.setProperty("--accent-soft", soft);
    root.setAttribute("data-center-nav", s.centerNav ? "true" : "false");

    const computed = getComputedStyle(root);
    console.debug(
        "[display] applied — " +
            `font: ${s.fontFamily || "(system)"} → css: ${fontCss} ` +
            `| computed font-family: ${computed.getPropertyValue("--app-font").trim()} ` +
            `| size: ${s.fontSize}px, zoom: ${s.zoom} → effective: ${effectiveZoom.toFixed(3)} ` +
            `| lang: ${s.lang} (rtl=${isRTL(s.lang)}) ` +
            `| accent: ${s.accentColor}`,
    );
}
