import { useState, useEffect, useCallback, useRef } from "react";
import { usePlatform } from "./hooks/usePlatform";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "./i18n";
import { I18nProvider } from "./i18n/provider";
import { type DisplaySettings, loadDisplay, applyDisplay } from "./display";
import Overview from "./pages/Overview";
import Settings from "./pages/Settings";
import Logs from "./pages/Logs";

export type { ThemeMode, LangMode, AccentColor, DisplaySettings } from "./display";

// ── Icons (clean line-art, no background box) ──────────────────────────────────

function IconOverview() {
    return (
        <svg
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            width="18"
            height="18"
        >
            <rect x="2.5" y="2.5" width="6" height="6" rx="1.5" />
            <rect x="11.5" y="2.5" width="6" height="3.5" rx="1.5" />
            <rect x="11.5" y="9" width="6" height="8.5" rx="1.5" />
            <rect x="2.5" y="11.5" width="6" height="6" rx="1.5" />
        </svg>
    );
}

function IconPlayer() {
    return (
        <svg
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            width="18"
            height="18"
        >
            <circle cx="10" cy="10" r="7.5" />
            <polygon points="8.5,7 14,10 8.5,13" fill="currentColor" stroke="none" />
        </svg>
    );
}

function IconVersionPrefer() {
    return (
        <svg
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            width="18"
            height="18"
        >
            <line x1="3" y1="5.5" x2="17" y2="5.5" />
            <line x1="3" y1="10" x2="17" y2="10" />
            <line x1="3" y1="14.5" x2="11" y2="14.5" />
            <polyline points="13,12 16,15 13,18" strokeWidth="1.8" />
        </svg>
    );
}

function IconNetwork() {
    return (
        <svg
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            width="18"
            height="18"
        >
            <circle cx="10" cy="10" r="7.5" />
            <path d="M2.5 10h15M10 2.5c-2.8 2.2-4.5 4.7-4.5 7.5s1.7 5.3 4.5 7.5M10 2.5c2.8 2.2 4.5 4.7 4.5 7.5s-1.7 5.3-4.5 7.5" />
        </svg>
    );
}

function IconSystem() {
    return (
        <svg
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            width="18"
            height="18"
        >
            <circle cx="10" cy="10" r="2.8" />
            <path d="M10 2v2M10 16v2M2 10h2M16 10h2M4.3 4.3l1.4 1.4M14.3 14.3l1.4 1.4M4.3 15.7l1.4-1.4M14.3 5.7l1.4-1.4" />
        </svg>
    );
}

function IconLogs() {
    return (
        <svg
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            width="18"
            height="18"
        >
            <rect x="3" y="3" width="14" height="14" rx="2.5" />
            <line x1="6" y1="7" x2="14" y2="7" />
            <line x1="6" y1="10.5" x2="14" y2="10.5" />
            <line x1="6" y1="14" x2="10" y2="14" />
        </svg>
    );
}

// ── Toast ───────────────────────────────────────────────────────────────────────

export interface Toast {
    id: number;
    message: string;
    error: boolean;
}

// ── About modal ─────────────────────────────────────────────────────────────────

function AboutModal({ onClose }: { onClose: () => void }) {
    const t = useI18n();
    const [version, setVersion] = useState("0.1.0");

    useEffect(() => {
        invoke<string>("get_app_version")
            .then(setVersion)
            .catch(() => {});
    }, []);

    const openLink = async (url: string) => {
        try {
            await openUrl(url);
        } catch {
            window.open(url, "_blank");
        }
    };

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div
                className="modal-card"
                onClick={(e) => e.stopPropagation()}
                style={{ position: "relative" }}
            >
                <button className="modal-close" onClick={onClose}>
                    ✕
                </button>

                <img
                    className="about-icon"
                    src="/app-icon.png"
                    alt="etlp icon"
                    onError={(e) => {
                        (e.target as HTMLImageElement).style.display = "none";
                    }}
                />

                <div className="about-name">{t("app_name")}</div>
                <div className="about-version">
                    {t("about_version_label")} {version}
                </div>

                <div className="about-links">
                    <button
                        className="about-link-btn"
                        title="GitHub"
                        onClick={() =>
                            void openLink("https://github.com/PiliPili-Team/etlp")
                        }
                    >
                        <svg
                            viewBox="0 0 24 24"
                            width="22"
                            height="22"
                            fill="currentColor"
                        >
                            <path d="M12 2C6.477 2 2 6.477 2 12c0 4.418 2.865 8.166 6.839 9.489.5.092.682-.217.682-.482 0-.237-.009-.868-.013-1.703-2.782.604-3.369-1.342-3.369-1.342-.454-1.155-1.11-1.463-1.11-1.463-.908-.62.069-.608.069-.608 1.003.07 1.531 1.03 1.531 1.03.892 1.529 2.341 1.087 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0 0 22 12c0-5.523-4.477-10-10-10z" />
                        </svg>
                    </button>
                    <button
                        className="about-link-btn"
                        title="Greasy Fork (Tampermonkey)"
                        onClick={() =>
                            void openLink(
                                "https://greasyfork.org/zh-CN/scripts/448648-embytolocalplayer",
                            )
                        }
                    >
                        {/* Tampermonkey userscript icon */}
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            width="22"
                            height="22"
                            fill="currentColor"
                        >
                            <path d="M5.955.002C3-.071.275 2.386.043 5.335c-.069 3.32-.011 6.646-.03 9.969c.06 1.87-.276 3.873.715 5.573c1.083 2.076 3.456 3.288 5.77 3.105c4.003-.011 8.008.022 12.011-.017c2.953-.156 5.478-2.815 5.482-5.772c-.007-4.235.023-8.473-.015-12.708C23.82 2.533 21.16.007 18.205.003c-4.083-.005-8.167 0-12.25-.002zm.447 12.683c2.333-.046 4.506 1.805 4.83 4.116c.412 2.287-1.056 4.716-3.274 5.411c-2.187.783-4.825-.268-5.874-2.341c-1.137-2.039-.52-4.827 1.37-6.197a4.9 4.9 0 0 1 2.948-.99zm11.245 0c2.333-.046 4.505 1.805 4.829 4.116c.413 2.287-1.056 4.716-3.273 5.411c-2.188.783-4.825-.268-5.875-2.341c-1.136-2.039-.52-4.827 1.37-6.197a4.9 4.9 0 0 1 2.949-.99z" />
                        </svg>
                    </button>
                </div>

                <div className="about-credits">
                    <div>
                        {t("about_thanks")}&nbsp;
                        <a
                            href="#"
                            onClick={(e) => {
                                e.preventDefault();
                                void openLink(
                                    "https://github.com/kjtsune/embyToLocalPlayer",
                                );
                            }}
                        >
                            embyToLocalPlayer
                        </a>
                        &nbsp;{t("about_thanks_desc")}
                    </div>
                    <div style={{ marginTop: 8, fontSize: 11 }}>
                        © 2024–2026 PiliPili Team. All rights reserved
                    </div>
                </div>
            </div>
        </div>
    );
}

// ── AppInner (consumes i18n context) ────────────────────────────────────────────

type TabId = "overview" | "player" | "version-prefer" | "network" | "system" | "logs";

const LAST_TAB_KEY = "etlp-last-tab";
const VALID_TABS: TabId[] = [
    "overview",
    "player",
    "version-prefer",
    "network",
    "system",
    "logs",
];

interface AppInnerProps {
    display: DisplaySettings;
    onDisplayChange: (patch: Partial<DisplaySettings>) => void;
}

function AppInner({ display, onDisplayChange }: AppInnerProps) {
    const t = useI18n();
    const platform = usePlatform();
    const [tab, setTab] = useState<TabId>(() => {
        const saved = localStorage.getItem(LAST_TAB_KEY) as TabId | null;
        return saved && VALID_TABS.includes(saved) ? saved : "overview";
    });
    const [toasts, setToasts] = useState<Toast[]>([]);
    const [showAbout, setShowAbout] = useState(false);
    const toastIdRef = useRef(0);

    useEffect(() => {
        document.body.className = platform !== "unknown" ? `platform-${platform}` : "";
    }, [platform]);

    // Listen for tray "About" event
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        import("@tauri-apps/api/event").then(({ listen }) => {
            listen("show-about", () => setShowAbout(true)).then((fn) => {
                unlisten = fn;
            });
        });
        return () => {
            unlisten?.();
        };
    }, []);

    const handleTabChange = useCallback((id: TabId) => {
        setTab(id);
        localStorage.setItem(LAST_TAB_KEY, id);
    }, []);

    const addToast = useCallback((message: string, error = false) => {
        const id = ++toastIdRef.current;
        setToasts((prev) => [...prev, { id, message, error }]);
        setTimeout(() => setToasts((prev) => prev.filter((tst) => tst.id !== id)), 3000);
    }, []);

    const isMac = platform === "macos";
    const isWindows = platform === "windows";

    const NAV_SECTIONS = [
        {
            items: [
                {
                    id: "overview" as TabId,
                    icon: <IconOverview />,
                    label: t("nav_overview"),
                },
            ],
        },
        {
            label: t("nav_sec_play"),
            items: [
                { id: "player" as TabId, icon: <IconPlayer />, label: t("nav_player") },
                {
                    id: "version-prefer" as TabId,
                    icon: <IconVersionPrefer />,
                    label: t("nav_version_prefer"),
                },
            ],
        },
        {
            label: t("nav_sec_config"),
            items: [
                {
                    id: "network" as TabId,
                    icon: <IconNetwork />,
                    label: t("nav_network"),
                },
                { id: "system" as TabId, icon: <IconSystem />, label: t("nav_system") },
            ],
        },
        {
            label: t("nav_sec_debug"),
            items: [{ id: "logs" as TabId, icon: <IconLogs />, label: t("nav_logs") }],
        },
    ];

    return (
        <div className="app">
            {(isMac || isWindows) && (
                // data-tauri-drag-region activates Tauri's JS drag API
                // (core:window:allow-start-dragging) on mousedown, bypassing
                // any NSVisualEffectView hit-test interference with the CSS
                // -webkit-app-region approach.
                // On Windows this also provides the drag region that would
                // otherwise be absent (no native title bar is rendered there).
                <div className="titlebar" data-tauri-drag-region>
                    <img className="titlebar-logo" src="/app-icon.png" alt="" />
                    <span className="titlebar-name">{t("app_name")}</span>
                </div>
            )}

            <div className="body">
                <nav className="sidebar">
                    {NAV_SECTIONS.map((section, si) => (
                        <div key={si}>
                            {section.label && (
                                <div className="sidebar-section-label">
                                    {section.label}
                                </div>
                            )}
                            {section.items.map((item) => (
                                <div
                                    key={item.id}
                                    className={`nav-item${tab === item.id ? " active" : ""}`}
                                    onClick={() => handleTabChange(item.id)}
                                >
                                    <span className="nav-icon">{item.icon}</span>
                                    {item.label}
                                </div>
                            ))}
                        </div>
                    ))}
                </nav>

                <main className="content">
                    {tab === "overview" && (
                        <Overview
                            addToast={addToast}
                            onAbout={() => setShowAbout(true)}
                        />
                    )}
                    {(tab === "player" ||
                        tab === "version-prefer" ||
                        tab === "network" ||
                        tab === "system") && (
                        <Settings
                            section={tab}
                            addToast={addToast}
                            display={display}
                            onDisplayChange={onDisplayChange}
                        />
                    )}
                    {/* Logs stays mounted so its buffer survives tab switches;
                        polling pauses while hidden via the `active` prop. */}
                    <div style={{ display: tab === "logs" ? "contents" : "none" }}>
                        <Logs active={tab === "logs"} />
                    </div>
                </main>
            </div>

            <div className="toast-area">
                {toasts.map((tst) => (
                    <div key={tst.id} className={`toast${tst.error ? " error" : ""}`}>
                        {tst.message}
                    </div>
                ))}
            </div>

            {showAbout && <AboutModal onClose={() => setShowAbout(false)} />}
        </div>
    );
}

// ── Root ─────────────────────────────────────────────────────────────────────────

export default function App() {
    const [display, setDisplay] = useState<DisplaySettings>(loadDisplay);

    useEffect(() => {
        applyDisplay(display);
        localStorage.setItem("etlp-display", JSON.stringify(display));
    }, [display]);

    const updateDisplay = useCallback((patch: Partial<DisplaySettings>) => {
        setDisplay((prev) => ({ ...prev, ...patch }));
    }, []);

    return (
        <I18nProvider lang={display.lang}>
            <AppInner display={display} onDisplayChange={updateDisplay} />
        </I18nProvider>
    );
}
