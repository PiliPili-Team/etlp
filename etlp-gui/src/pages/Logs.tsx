import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface LogResponse {
    lines: string[];
    next_bytes: number;
}

const POLL_INTERVAL = 800;
const MAX_LINES = 2000;

function classifyLine(line: string): string {
    const l = line.toLowerCase();
    if (l.includes(" error") || l.includes("[error]")) return "error";
    if (l.includes(" warn")  || l.includes("[warn]"))  return "warn";
    if (l.includes(" debug") || l.includes("[debug]")) return "debug";
    if (l.includes(" trace") || l.includes("[trace]")) return "trace";
    return "info";
}

export default function Logs() {
    const [lines, setLines] = useState<string[]>([]);
    const [nextBytes, setNextBytes] = useState(0);
    const [autoScroll, setAutoScroll] = useState(true);
    const [filter, setFilter] = useState("");
    const bodyRef = useRef<HTMLDivElement>(null);
    const polling = useRef(false);

    const fetchNew = useCallback(async (since: number) => {
        try {
            const resp = await invoke<LogResponse>("get_log_lines", {
                sinceBytes: since,
            });
            if (resp.lines.length > 0) {
                setLines((prev) => {
                    const merged = [...prev, ...resp.lines];
                    return merged.length > MAX_LINES
                        ? merged.slice(merged.length - MAX_LINES)
                        : merged;
                });
            }
            setNextBytes(resp.next_bytes);
            return resp.next_bytes;
        } catch {
            return since;
        }
    }, []);

    // Initial load
    useEffect(() => {
        void fetchNew(0);
    }, [fetchNew]);

    // Polling
    useEffect(() => {
        if (polling.current) return;
        polling.current = true;

        let current = nextBytes;
        const tick = async () => {
            current = await fetchNew(current);
        };

        const iv = setInterval(() => void tick(), POLL_INTERVAL);
        return () => { clearInterval(iv); polling.current = false; };
    }, [fetchNew]);

    // Auto-scroll
    useEffect(() => {
        if (autoScroll && bodyRef.current) {
            bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
        }
    }, [lines, autoScroll]);

    const handleScroll = () => {
        const el = bodyRef.current;
        if (!el) return;
        const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
        setAutoScroll(atBottom);
    };

    const handleClear = () => setLines([]);

    const handleScrollToBottom = () => {
        if (bodyRef.current) {
            bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
        }
        setAutoScroll(true);
    };

    const displayed = filter
        ? lines.filter((l) => l.toLowerCase().includes(filter.toLowerCase()))
        : lines;

    return (
        <>
            <div className="page-title">日志</div>
            <div className="log-container">
                <div className="log-toolbar">
                    <span className="log-toolbar-title">
                        实时日志
                        {lines.length > 0 && (
                            <span style={{ marginLeft: 8, color: "var(--text-3)" }}>
                                {lines.length} 行
                            </span>
                        )}
                    </span>
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                        <input
                            className="input"
                            style={{ minWidth: 140, height: 28, padding: "3px 10px" }}
                            placeholder="过滤…"
                            value={filter}
                            onChange={(e) => setFilter(e.target.value)}
                        />
                        <button className="btn" style={{ padding: "4px 10px", fontSize: 12 }}
                            onClick={handleClear}>
                            清空
                        </button>
                        {!autoScroll && (
                            <button className="btn btn-primary" style={{ padding: "4px 10px", fontSize: 12 }}
                                onClick={handleScrollToBottom}>
                                ↓ 跳到底部
                            </button>
                        )}
                    </div>
                </div>

                <div
                    className="log-body"
                    ref={bodyRef}
                    onScroll={handleScroll}
                >
                    {displayed.length === 0 ? (
                        <span style={{ color: "var(--text-3)" }}>
                            等待日志输出…
                        </span>
                    ) : (
                        displayed.map((line, i) => (
                            <span
                                key={i}
                                className={`log-line ${classifyLine(line)}`}
                            >
                                {line}
                                {"\n"}
                            </span>
                        ))
                    )}
                </div>
            </div>
        </>
    );
}
