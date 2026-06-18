import { useState, useEffect, useCallback, useRef } from "react";
import { usePlatform } from "./hooks/usePlatform";
import Overview from "./pages/Overview";
import Settings from "./pages/Settings";
import Logs from "./pages/Logs";

// ── Icons ──────────────────────────────────────────────────────────────────────

function IconOverview() {
    return (
        <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round">
            <rect x="2" y="2" width="6" height="7" rx="1.5" />
            <rect x="10" y="2" width="6" height="4" rx="1.5" />
            <rect x="10" y="9" width="6" height="7" rx="1.5" />
            <rect x="2" y="12" width="6" height="4" rx="1.5" />
        </svg>
    );
}

function IconPlayer() {
    return (
        <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round">
            <circle cx="9" cy="9" r="7" />
            <polygon points="7.5,6.5 12.5,9 7.5,11.5" fill="currentColor" stroke="none" />
        </svg>
    );
}

function IconPlaylist() {
    return (
        <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round">
            <line x1="3" y1="5" x2="15" y2="5" />
            <line x1="3" y1="9" x2="15" y2="9" />
            <line x1="3" y1="13" x2="11" y2="13" />
        </svg>
    );
}

function IconNetwork() {
    return (
        <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round">
            <circle cx="9" cy="9" r="7" />
            <path d="M2 9h14M9 2c-2.5 2-4 4.3-4 7s1.5 5 4 7M9 2c2.5 2 4 4.3 4 7s-1.5 5-4 7" />
        </svg>
    );
}

function IconSystem() {
    return (
        <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round">
            <circle cx="9" cy="9" r="2.5" />
            <path d="M9 1v2M9 15v2M1 9h2M15 9h2M3.2 3.2l1.4 1.4M13.4 13.4l1.4 1.4M3.2 14.8l1.4-1.4M13.4 4.6l1.4-1.4" />
        </svg>
    );
}

function IconLogs() {
    return (
        <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.6"
            strokeLinecap="round" strokeLinejoin="round">
            <rect x="2" y="2" width="14" height="14" rx="2" />
            <line x1="5" y1="6" x2="13" y2="6" />
            <line x1="5" y1="9" x2="13" y2="9" />
            <line x1="5" y1="12" x2="9"  y2="12" />
        </svg>
    );
}

// ── Nav items config ───────────────────────────────────────────────────────────

type TabId = "overview" | "player" | "playlist" | "network" | "system" | "logs";

interface NavSection {
    label?: string;
    items: { id: TabId; icon: React.ReactNode; label: string }[];
}

const NAV_SECTIONS: NavSection[] = [
    {
        items: [{ id: "overview", icon: <IconOverview />, label: "概览" }],
    },
    {
        label: "播放",
        items: [
            { id: "player",   icon: <IconPlayer />,   label: "播放器" },
            { id: "playlist", icon: <IconPlaylist />, label: "播放列表" },
        ],
    },
    {
        label: "配置",
        items: [
            { id: "network", icon: <IconNetwork />, label: "网络" },
            { id: "system",  icon: <IconSystem />,  label: "系统" },
        ],
    },
    {
        label: "调试",
        items: [{ id: "logs", icon: <IconLogs />, label: "日志" }],
    },
];

// ── Toast ──────────────────────────────────────────────────────────────────────

export interface Toast { id: number; message: string; error: boolean; }

// ── App ────────────────────────────────────────────────────────────────────────

export default function App() {
    const platform = usePlatform();
    const [tab, setTab] = useState<TabId>("overview");
    const [toasts, setToasts] = useState<Toast[]>([]);
    const toastIdRef = useRef(0);

    useEffect(() => {
        document.body.className = platform !== "unknown" ? `platform-${platform}` : "";
    }, [platform]);

    const addToast = useCallback((message: string, error = false) => {
        const id = ++toastIdRef.current;
        setToasts((prev) => [...prev, { id, message, error }]);
        setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 3000);
    }, []);

    const isMac = platform === "macos";

    return (
        <div className="app">
            {isMac && (
                <div className="titlebar">
                    <span className="titlebar-title">etlp</span>
                </div>
            )}

            <div className="body">
                {/* ── Sidebar ── */}
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
                                    onClick={() => setTab(item.id)}
                                >
                                    <span className="nav-icon">{item.icon}</span>
                                    {item.label}
                                </div>
                            ))}
                        </div>
                    ))}
                </nav>

                {/* ── Main content ── */}
                <main className="content">
                    {tab === "overview" && (
                        <Overview addToast={addToast} />
                    )}
                    {(tab === "player" || tab === "playlist" || tab === "network" || tab === "system") && (
                        <Settings section={tab} addToast={addToast} />
                    )}
                    {tab === "logs" && <Logs />}
                </main>
            </div>

            {/* ── Toasts ── */}
            <div className="toast-area">
                {toasts.map((t) => (
                    <div key={t.id} className={`toast${t.error ? " error" : ""}`}>
                        {t.message}
                    </div>
                ))}
            </div>
        </div>
    );
}
