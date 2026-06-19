import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "../i18n";

interface LogResponse {
    lines: string[];
    next_bytes: number;
}
interface LogPaths {
    app_log: string;
    mpv_log: string | null;
}
type LogSource = "app" | "mpv";

const POLL_MS = 800;
const MAX_LINES = 2000;

// Parse a tracing/log4rs formatted line.
// Expected formats:
//   2026-06-19T12:34:56.789Z  INFO etlp: message
//   2026-06-19T12:34:56  INFO message
//   [INFO]  message
//   plain raw text
interface ParsedLine {
    time: string;
    level: "info" | "warn" | "error" | "debug" | "trace" | "raw";
    content: string;
}

function parseLine(raw: string): ParsedLine {
    // ISO timestamp + level
    const m1 = raw.match(
        /^(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?\s+(INFO|WARN|ERROR|DEBUG|TRACE)\s+(.*)$/i,
    );
    if (m1) {
        return {
            time: m1[1].replace("T", " "),
            level: m1[2].toLowerCase() as ParsedLine["level"],
            content: m1[3],
        };
    }

    // [LEVEL] format
    const m2 = raw.match(/^\s*\[(INFO|WARN|ERROR|DEBUG|TRACE)\]\s*(.*)$/i);
    if (m2) {
        return {
            time: "",
            level: m2[1].toLowerCase() as ParsedLine["level"],
            content: m2[2],
        };
    }

    // Level keyword anywhere near the start
    const m3 = raw.match(/\b(INFO|WARN|ERROR|DEBUG|TRACE)\b(.*)$/i);
    if (m3) {
        return {
            time: "",
            level: m3[1].toLowerCase() as ParsedLine["level"],
            content: m3[2].trim() || raw,
        };
    }

    return { time: "", level: "raw", content: raw };
}

function LogLineView({ raw }: { raw: string }) {
    const p = parseLine(raw);
    if (p.level === "raw") {
        return (
            <div className="log-line">
                <span className="log-content">{raw}</span>
            </div>
        );
    }
    return (
        <div className="log-line">
            {p.time && <span className="log-time">{p.time.slice(11)}</span>}
            <span className={`log-badge log-badge-${p.level}`}>
                {p.level.toUpperCase()}
            </span>
            <span className={`log-content ${p.level}`}>{p.content}</span>
        </div>
    );
}

export default function Logs({ active }: { active: boolean }) {
    const t = useI18n();
    const [lines, setLines] = useState<string[]>([]);
    const [autoScroll, setAutoScroll] = useState(true);
    const [filter, setFilter] = useState("");
    const [source, setSource] = useState<LogSource>("app");
    const [paths, setPaths] = useState<LogPaths | null>(null);
    // A user-picked mpv log file; falls back to the config-derived default.
    const [mpvCustomPath, setMpvCustomPath] = useState<string | null>(null);

    const bodyRef = useRef<HTMLDivElement>(null);
    const posRef = useRef(0);

    useEffect(() => {
        invoke<LogPaths>("get_log_paths")
            .then(setPaths)
            .catch(() => {});
    }, []);

    // The mpv log to read: a user-picked file wins over the config default.
    const effectiveMpvPath = mpvCustomPath ?? paths?.mpv_log ?? null;

    const fetchChunk = useCallback(
        async (since: number, path: string | null): Promise<number> => {
            try {
                const resp = await invoke<LogResponse>("get_log_lines", {
                    sinceBytes: since,
                    path,
                });
                if (resp.lines.length > 0) {
                    setLines((prev) => {
                        const merged = [...prev, ...resp.lines];
                        return merged.length > MAX_LINES
                            ? merged.slice(-MAX_LINES)
                            : merged;
                    });
                }
                return resp.next_bytes;
            } catch {
                return since;
            }
        },
        [],
    );

    // Reset the buffer only when the log source itself changes — not when the
    // tab is hidden/shown. This is what lets a hidden Logs tab keep its content
    // and resume from where it left off instead of reloading from byte 0.
    // Clearing happens in cleanup (allowed in effects) so the source swap drops
    // the old file's lines before the polling effect restarts from byte 0.
    useEffect(() => {
        return () => {
            posRef.current = 0;
            setLines([]);
        };
    }, [source, effectiveMpvPath]);

    // Poll the active source. Gated on `active` so the loop pauses while the
    // tab is hidden, then resumes from posRef.current (appending new lines
    // only) when the tab is shown again.
    useEffect(() => {
        // app log reads the default file (path = null); mpv reads the chosen
        // file. Skip polling when hidden, or when the mpv view has no file yet.
        const logPath = source === "mpv" ? effectiveMpvPath : null;
        if (!active || (source === "mpv" && !logPath)) {
            return;
        }

        const live = { ok: true };
        let pos = posRef.current;

        // Defer the initial fetch past the synchronous effect body so the rule
        // doesn't flag the async setState call chain as synchronous.
        const init = setTimeout(() => {
            void fetchChunk(pos, logPath).then((next) => {
                if (!live.ok) return;
                pos = next;
                posRef.current = next;
            });
        }, 0);

        const iv = setInterval(async () => {
            if (!live.ok) {
                clearInterval(iv);
                return;
            }
            const next = await fetchChunk(pos, logPath);
            if (live.ok) {
                pos = next;
                posRef.current = next;
            }
        }, POLL_MS);

        return () => {
            live.ok = false;
            clearTimeout(init);
            clearInterval(iv);
        };
    }, [active, source, effectiveMpvPath, fetchChunk]);

    useEffect(() => {
        if (autoScroll && bodyRef.current) {
            bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
        }
    }, [lines, autoScroll]);

    const handleScroll = () => {
        const el = bodyRef.current;
        if (!el) return;
        setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 40);
    };

    const handleSourceSwitch = (s: LogSource) => {
        if (s !== source) setSource(s);
    };

    const handleOpenLogFolder = async () => {
        try {
            await invoke("open_log_folder");
        } catch {
            /* ignore: opening the folder is best-effort */
        }
    };

    const handlePickMpvLog = async () => {
        const selected = await open({
            multiple: false,
            directory: false,
            filters: [{ name: "Log", extensions: ["log", "txt"] }],
        });
        if (typeof selected === "string") {
            setMpvCustomPath(selected);
            setSource("mpv");
            setLines([]);
            posRef.current = 0;
        }
    };

    const displayed = filter
        ? lines.filter((l) => l.toLowerCase().includes(filter.toLowerCase()))
        : lines;

    // mpv view is usable when a default log exists or the user picked a file.
    const hasMpv = Boolean(effectiveMpvPath);

    return (
        <>
            <div className="page-title">{t("page_logs")}</div>
            <div className="log-container">
                <div className="log-toolbar">
                    <div
                        style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 10,
                            flex: 1,
                            minWidth: 0,
                        }}
                    >
                        <div className="log-source-tabs">
                            <button
                                className={`log-source-tab${source === "app" ? " active" : ""}`}
                                onClick={() => handleSourceSwitch("app")}
                            >
                                {t("logs_app")}
                            </button>
                            <button
                                className={`log-source-tab${source === "mpv" ? " active" : ""}`}
                                onClick={() => handleSourceSwitch("mpv")}
                                title={t("logs_mpv")}
                            >
                                {t("logs_mpv")}
                            </button>
                        </div>
                        {lines.length > 0 && (
                            <span
                                style={{
                                    fontSize: 11,
                                    color: "var(--text-3)",
                                    flexShrink: 0,
                                }}
                            >
                                {lines.length} {t("logs_lines")}
                            </span>
                        )}
                    </div>

                    <div
                        style={{
                            display: "flex",
                            gap: 7,
                            alignItems: "center",
                            flexShrink: 0,
                        }}
                    >
                        {source === "mpv" && (
                            <button
                                className="btn"
                                style={{ padding: "4px 10px", fontSize: 12 }}
                                onClick={() => void handlePickMpvLog()}
                                title={effectiveMpvPath ?? undefined}
                            >
                                {t("logs_pick_mpv")}
                            </button>
                        )}
                        <button
                            className="btn"
                            style={{ padding: "4px 10px", fontSize: 12 }}
                            onClick={() => void handleOpenLogFolder()}
                        >
                            {t("logs_open_folder")}
                        </button>
                        <input
                            className="input"
                            style={{
                                minWidth: 110,
                                height: 28,
                                padding: "3px 10px",
                                fontSize: 12,
                            }}
                            placeholder={t("logs_filter")}
                            value={filter}
                            onChange={(e) => setFilter(e.target.value)}
                        />
                        <button
                            className="btn"
                            style={{ padding: "4px 10px", fontSize: 12 }}
                            onClick={() => {
                                setLines([]);
                                posRef.current = 0;
                            }}
                        >
                            {t("logs_clear")}
                        </button>
                        {!autoScroll && (
                            <button
                                className="btn btn-primary"
                                style={{ padding: "4px 10px", fontSize: 12 }}
                                onClick={() => {
                                    if (bodyRef.current)
                                        bodyRef.current.scrollTop =
                                            bodyRef.current.scrollHeight;
                                    setAutoScroll(true);
                                }}
                            >
                                {t("logs_bottom")}
                            </button>
                        )}
                    </div>
                </div>

                <div className="log-body" ref={bodyRef} onScroll={handleScroll}>
                    {displayed.length === 0 ? (
                        <span style={{ color: "var(--text-3)" }}>
                            {source === "mpv" && !hasMpv
                                ? t("logs_no_mpv")
                                : t("logs_empty")}
                        </span>
                    ) : (
                        displayed.map((line, i) => <LogLineView key={i} raw={line} />)
                    )}
                </div>
            </div>
        </>
    );
}
