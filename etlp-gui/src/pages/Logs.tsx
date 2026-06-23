import {
    useState,
    useEffect,
    useRef,
    useCallback,
    useDeferredValue,
    useMemo,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "../i18n";

interface LogResponse {
    lines: string[];
    next_bytes: number;
}
interface TailResponse {
    lines: string[];
    start_bytes: number;
    next_bytes: number;
}
interface BeforeResponse {
    lines: string[];
    start_bytes: number;
}
interface LogPaths {
    app_log: string;
    mpv_log: string | null;
}
type LogSource = "app" | "mpv";

// localStorage key holding the last user-picked mpv log path, so a manual
// choice is remembered and preferred over the config default on next launch.
const MPV_CUSTOM_KEY = "logs_mpv_custom_path";

const POLL_MS = 800;
// One page = newest 200 lines; older pages are fetched on scroll-up.
const PAGE_SIZE = 200;
// Hard cap on rendered lines so the DOM stays bounded even after live tailing
// and several older pages; trimming only happens at the bottom (live append).
const MAX_LINES = 3000;

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

// Display-only anonymizer for the Logs view. Heuristically redacts values that
// could identify a user or server when sharing a screenshot. It never touches
// the on-disk log file — file redaction is governed by the `mix_log` setting.
// Key=value rules cover device id, tokens and user/account names; bare IPv4
// addresses and URL hosts are matched structurally.
const ANON_KV_RULES: { re: RegExp; replace: string }[] = [
    // Secrets: api_key, token, access/refresh token, X-Emby-Token, ...
    {
        re: /\b((?:api[_-]?key|access[_-]?token|refresh[_-]?token|token|x-emby-token|x-mediabrowser-token)["']?\s*[:=]\s*["']?)[^\s"'&,}]+/gi,
        replace: "$1***",
    },
    // Bearer tokens in Authorization headers.
    { re: /\bBearer\s+[A-Za-z0-9._-]+/gi, replace: "Bearer ***" },
    // Device identifiers.
    {
        re: /\b((?:device[_-]?id|x-emby-device-id)["']?\s*[:=]\s*["']?)[^\s"'&,}]+/gi,
        replace: "$1***",
    },
    // Numeric / opaque user ids (kept distinct from usernames below).
    {
        re: /\b((?:user[_-]?id|userid)["']?\s*[:=]\s*["']?)[^\s"'&,}]+/gi,
        replace: "$1***",
    },
    // Account names, incl. Bangumi / Trakt usernames passed as key=value.
    {
        re: /\b((?:user[_-]?name|username|nickname)["']?\s*[:=]\s*["']?)[^\s"'&,}]+/gi,
        replace: "$1***",
    },
    // Bangumi / Trakt user slug embedded in a URL path, e.g. /users/alice.
    { re: /\/users\/[^/\s"'?]+/gi, replace: "/users/***" },
];

// Keep the scheme and the leading half of the host so the masked URL stays
// recognizable, mirroring the file-level `mix_host_gen` placeholder.
function maskUrlHost(line: string): string {
    return line.replace(
        /(https?:\/\/)([^/\s:"']+)(:\d+)?/gi,
        (_m, scheme: string, host: string, port = "") => {
            const keep = host.slice(0, Math.floor(host.length / 2));
            return `${scheme}${keep}***${port}`;
        },
    );
}

function maskSensitive(line: string): string {
    let out = maskUrlHost(line);
    for (const { re, replace } of ANON_KV_RULES) {
        out = out.replace(re, replace);
    }
    // Bare IPv4 addresses anywhere in the remaining text.
    return out.replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, "***.***.***.***");
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
    // Display-only anonymous mode; persisted so it survives a tab switch.
    const [anon, setAnon] = useState(() => localStorage.getItem("logs_anon") === "1");
    const [paths, setPaths] = useState<LogPaths | null>(null);
    const [loadingOlder, setLoadingOlder] = useState(false);
    const [hasOlder, setHasOlder] = useState(false);
    // A user-picked mpv log file; falls back to the config-derived default.
    // Restored from localStorage so a manual pick survives a restart and keeps
    // priority over the default until the user resets it.
    const [mpvCustomPath, setMpvCustomPath] = useState<string | null>(
        () => localStorage.getItem(MPV_CUSTOM_KEY) || null,
    );

    const bodyRef = useRef<HTMLDivElement>(null);
    // Live-tail cursor: byte offset up to which we have appended new lines.
    const posRef = useRef(0);
    // Oldest loaded byte offset (where the next older page ends); 0 = at head.
    const oldestRef = useRef(0);
    // True until the very first tail page has been loaded for this source.
    const initializedRef = useRef(false);

    useEffect(() => {
        invoke<LogPaths>("get_log_paths")
            .then(setPaths)
            .catch(() => {});
    }, []);

    // Drop a remembered pick whose file has since been deleted or rotated away,
    // so the view falls back to the default mpv log instead of a dead path.
    useEffect(() => {
        const saved = localStorage.getItem(MPV_CUSTOM_KEY);
        if (!saved) return;
        invoke<boolean>("path_exists", { path: saved })
            .then((exists) => {
                if (!exists) {
                    localStorage.removeItem(MPV_CUSTOM_KEY);
                    setMpvCustomPath(null);
                }
            })
            .catch(() => {});
    }, []);

    // The mpv log to read: a user-picked file wins over the config default.
    const effectiveMpvPath = mpvCustomPath ?? paths?.mpv_log ?? null;

    // Append newly-written lines (live tail). Trims from the top only while the
    // user is at the bottom, so scrolling up to read older pages is not undone.
    const fetchTail = useCallback(
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

    // Load an older page (scroll-up), preserving the visual scroll position by
    // restoring the distance from the bottom after the prepend.
    const loadOlder = useCallback(async (path: string | null) => {
        if (oldestRef.current <= 0) {
            setHasOlder(false);
            return;
        }
        setLoadingOlder(true);
        const el = bodyRef.current;
        const prevHeight = el?.scrollHeight ?? 0;
        const prevTop = el?.scrollTop ?? 0;
        try {
            const resp = await invoke<BeforeResponse>("read_log_before", {
                beforeBytes: oldestRef.current,
                maxLines: PAGE_SIZE,
                path,
            });
            if (resp.lines.length > 0) {
                oldestRef.current = resp.start_bytes;
                setHasOlder(resp.start_bytes > 0);
                setLines((prev) => [...resp.lines, ...prev]);
                // Restore scroll so the viewport stays on the same lines.
                requestAnimationFrame(() => {
                    const node = bodyRef.current;
                    if (node) {
                        node.scrollTop = node.scrollHeight - prevHeight + prevTop;
                    }
                });
            } else {
                setHasOlder(false);
            }
        } catch {
            /* ignore: paging is best-effort */
        } finally {
            setLoadingOlder(false);
        }
    }, []);

    // Empty the active log file on disk, then reset the view to match. The live
    // poll keeps running and will pick up anything written afterwards.
    const clearLog = useCallback(async () => {
        const logPath = source === "mpv" ? effectiveMpvPath : null;
        if (source === "mpv" && !logPath) {
            setLines([]);
            return;
        }
        try {
            await invoke("clear_log_file", { path: logPath });
        } catch {
            /* ignore: clearing is best-effort */
        }
        setLines([]);
        posRef.current = 0;
        oldestRef.current = 0;
        setHasOlder(false);
        setAutoScroll(true);
    }, [source, effectiveMpvPath]);

    // Reset the buffer when the log source changes (cleanup runs before the
    // polling effect restarts), so a source swap drops the old file's lines.
    useEffect(() => {
        return () => {
            posRef.current = 0;
            oldestRef.current = 0;
            initializedRef.current = false;
            setLines([]);
            setHasOlder(false);
        };
    }, [source, effectiveMpvPath]);

    // Initialize with the newest page, then live-poll for appended lines. Gated
    // on `active` so the loop pauses while the tab is hidden.
    useEffect(() => {
        const logPath = source === "mpv" ? effectiveMpvPath : null;
        if (!active || (source === "mpv" && !logPath)) {
            return;
        }

        const live = { ok: true };

        const init = setTimeout(() => {
            void (async () => {
                // First activation for this source: load the tail page.
                if (!initializedRef.current) {
                    try {
                        const resp = await invoke<TailResponse>("tail_log", {
                            maxLines: PAGE_SIZE,
                            path: logPath,
                        });
                        if (!live.ok) return;
                        setLines(resp.lines);
                        posRef.current = resp.next_bytes;
                        oldestRef.current = resp.start_bytes;
                        setHasOlder(resp.start_bytes > 0);
                        initializedRef.current = true;
                    } catch {
                        return;
                    }
                }
                // Catch up on anything written since the cursor.
                const next = await fetchTail(posRef.current, logPath);
                if (live.ok) posRef.current = next;
            })();
        }, 0);

        const iv = setInterval(async () => {
            if (!live.ok || !initializedRef.current) return;
            const next = await fetchTail(posRef.current, logPath);
            if (live.ok) posRef.current = next;
        }, POLL_MS);

        return () => {
            live.ok = false;
            clearTimeout(init);
            clearInterval(iv);
        };
    }, [active, source, effectiveMpvPath, fetchTail]);

    useEffect(() => {
        if (autoScroll && bodyRef.current) {
            bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
        }
    }, [lines, autoScroll]);

    const handleScroll = () => {
        const el = bodyRef.current;
        if (!el) return;
        setAutoScroll(el.scrollHeight - el.scrollTop - el.clientHeight < 40);
        // Near the top → page in older lines.
        if (el.scrollTop < 60 && hasOlder && !loadingOlder) {
            const logPath = source === "mpv" ? effectiveMpvPath : null;
            void loadOlder(logPath);
        }
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
            // Remember the pick so it is preferred over the default next launch.
            localStorage.setItem(MPV_CUSTOM_KEY, selected);
            setMpvCustomPath(selected);
            setSource("mpv");
            setLines([]);
            posRef.current = 0;
            oldestRef.current = 0;
            initializedRef.current = false;
            setHasOlder(false);
        }
    };

    // Forget the remembered pick and fall back to the config-derived default
    // mpv log under the log folder. The source-reset is handled by the effect
    // keyed on `effectiveMpvPath`, so we only clear the buffer cursors here.
    const handleResetMpvLog = () => {
        localStorage.removeItem(MPV_CUSTOM_KEY);
        setMpvCustomPath(null);
        setSource("mpv");
        setLines([]);
        posRef.current = 0;
        oldestRef.current = 0;
        initializedRef.current = false;
        setHasOlder(false);
    };

    // Defer the filter so typing stays responsive on large buffers; the
    // expensive filtering runs at lower priority and is memoized.
    const deferredFilter = useDeferredValue(filter);
    const displayed = useMemo(() => {
        const base = anon ? lines.map(maskSensitive) : lines;
        if (!deferredFilter) return base;
        const needle = deferredFilter.toLowerCase();
        return base.filter((l) => l.toLowerCase().includes(needle));
    }, [lines, deferredFilter, anon]);

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
                        {source === "mpv" && mpvCustomPath && (
                            <button
                                className="btn"
                                style={{ padding: "4px 10px", fontSize: 12 }}
                                onClick={handleResetMpvLog}
                                title={t("logs_reset_mpv_title")}
                            >
                                {t("logs_reset_mpv")}
                            </button>
                        )}
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
                            className={`btn${anon ? " btn-primary" : ""}`}
                            style={{ padding: "4px 10px", fontSize: 12 }}
                            title={t("logs_anon_title")}
                            onClick={() => {
                                const next = !anon;
                                setAnon(next);
                                localStorage.setItem("logs_anon", next ? "1" : "0");
                            }}
                        >
                            {t("logs_anon")}
                        </button>
                        <button
                            className="btn"
                            style={{ padding: "4px 10px", fontSize: 12 }}
                            onClick={() => void clearLog()}
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
                        <>
                            {(loadingOlder || hasOlder) && (
                                <div className="log-older-hint">
                                    {loadingOlder
                                        ? t("logs_loading_older")
                                        : t("logs_scroll_older")}
                                </div>
                            )}
                            {displayed.map((line, i) => (
                                <LogLineView key={i} raw={line} />
                            ))}
                        </>
                    )}
                </div>
            </div>
        </>
    );
}
