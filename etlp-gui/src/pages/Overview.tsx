import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

interface ServerStatus {
    running: boolean;
    port: number;
}

interface Props {
    addToast: (msg: string, err?: boolean) => void;
    onAbout: () => void;
}

export default function Overview({ addToast, onAbout }: Props) {
    const t = useI18n();
    const [status, setStatus] = useState<ServerStatus>({ running: false, port: 58000 });
    const [busy, setBusy] = useState(false);
    const [startTime, setStartTime] = useState<Date | null>(null);
    const [elapsed, setElapsed] = useState("");
    const [prevRunning, setPrevRunning] = useState(status.running);

    const refreshStatus = useCallback(async () => {
        try {
            const s = await invoke<ServerStatus>("get_server_status");
            setStatus(s);
        } catch {
            /* ignore */
        }
    }, []);

    // Render-phase derived state: sync startTime with status.running transitions
    // so we avoid calling setState synchronously inside a useEffect body.
    if (prevRunning !== status.running) {
        setPrevRunning(status.running);
        if (!status.running) {
            setStartTime(null);
        } else if (!startTime) {
            setStartTime(new Date());
        }
    }

    useEffect(() => {
        // Defer the initial fetch past the synchronous effect body.
        const init = setTimeout(refreshStatus, 0);
        const iv = setInterval(refreshStatus, 5000);
        return () => {
            clearTimeout(init);
            clearInterval(iv);
        };
    }, [refreshStatus]);

    useEffect(() => {
        if (!startTime) return;
        const iv = setInterval(() => {
            const secs = Math.floor((Date.now() - startTime.getTime()) / 1000);
            const h = Math.floor(secs / 3600);
            const m = Math.floor((secs % 3600) / 60);
            const s = secs % 60;
            setElapsed(h > 0 ? `${h}h ${m}m ${s}s` : m > 0 ? `${m}m ${s}s` : `${s}s`);
        }, 1000);
        return () => {
            clearInterval(iv);
            setElapsed("");
        };
    }, [startTime]);

    const handleStart = useCallback(async () => {
        setBusy(true);
        try {
            const port = await invoke<number>("start_server");
            setStatus({ running: true, port });
            setStartTime(new Date());
            addToast(t("toast_started", { port }));
        } catch (e) {
            addToast(String(e), true);
        } finally {
            setBusy(false);
        }
    }, [addToast, t]);

    const handleStop = useCallback(async () => {
        setBusy(true);
        try {
            await invoke("stop_server");
            setStatus((s) => ({ ...s, running: false }));
            setStartTime(null);
            addToast(t("toast_stopped"));
        } catch (e) {
            addToast(String(e), true);
        } finally {
            setBusy(false);
        }
    }, [addToast, t]);

    const handleRestart = useCallback(async () => {
        setBusy(true);
        try {
            const port = await invoke<number>("restart_server");
            setStatus({ running: true, port });
            setStartTime(new Date());
            addToast(t("toast_restarted", { port }));
        } catch (e) {
            addToast(String(e), true);
        } finally {
            setBusy(false);
        }
    }, [addToast, t]);

    const handleOpenFolder = useCallback(async () => {
        try {
            await invoke("open_config_folder");
        } catch (e) {
            addToast(String(e), true);
        }
    }, [addToast]);

    const handleEditConfig = useCallback(async () => {
        try {
            await invoke("edit_config");
        } catch (e) {
            addToast(String(e), true);
        }
    }, [addToast]);

    return (
        <>
            <div className="page-title">{t("page_overview")}</div>

            {/* Hero card */}
            <div className="overview-hero">
                <div className="overview-hero-header">
                    <div>
                        <div className="overview-hero-title">{t("ov_service")}</div>
                        <div style={{ marginTop: 8 }}>
                            <span
                                className={`status-badge ${status.running ? "running" : "stopped"}`}
                            >
                                <span className="status-dot" />
                                {status.running ? t("ov_running") : t("ov_stopped")}
                            </span>
                        </div>
                    </div>
                    <div className="overview-actions">
                        {!status.running ? (
                            <button
                                className="btn btn-primary"
                                onClick={handleStart}
                                disabled={busy}
                            >
                                {t("ov_start")}
                            </button>
                        ) : (
                            <button
                                className="btn btn-danger"
                                onClick={handleStop}
                                disabled={busy}
                            >
                                {t("ov_stop")}
                            </button>
                        )}
                    </div>
                </div>

                <div className="overview-meta">
                    <div className="meta-item">
                        <div className="meta-label">{t("ov_port")}</div>
                        <div className="meta-value">{status.port}</div>
                        <div className="meta-sub">{t("ov_port_desc")}</div>
                    </div>
                    <div className="meta-item">
                        <div className="meta-label">{t("ov_uptime")}</div>
                        <div className="meta-value">{elapsed || "—"}</div>
                        <div className="meta-sub">{t("ov_uptime_desc")}</div>
                    </div>
                    <div className="meta-item">
                        <div className="meta-label">{t("ov_address")}</div>
                        <div className="meta-value">127.0.0.1</div>
                        <div className="meta-sub">{t("ov_address_desc")}</div>
                    </div>
                </div>
            </div>

            {/* Config actions */}
            <div className="settings-group-title" style={{ marginTop: 0 }}>
                {t("ov_config")}
            </div>
            <div
                style={{
                    background: "var(--surface)",
                    borderRadius: 12,
                    boxShadow: "var(--shadow-sm)",
                    overflow: "hidden",
                    marginBottom: 14,
                }}
            >
                <div className="row">
                    <div className="row-label">
                        <div>{t("ov_config_file")}</div>
                        <div className="row-desc">{t("ov_config_file_desc")}</div>
                    </div>
                    <div className="row-control">
                        <button className="btn" onClick={handleOpenFolder}>
                            {t("open_dir")}
                        </button>
                        <button className="btn" onClick={handleEditConfig}>
                            {t("ov_edit_config")}
                        </button>
                    </div>
                </div>
                <div className="row">
                    <div className="row-label">
                        <div>{t("ov_restart")}</div>
                        <div className="row-desc">{t("ov_restart_desc")}</div>
                    </div>
                    <div className="row-control">
                        <button className="btn" onClick={handleRestart} disabled={busy}>
                            {t("ov_restart")}
                        </button>
                    </div>
                </div>
                <div className="row" style={{ borderBottom: "none" }}>
                    <div className="row-label">
                        <div>{t("ov_about")}</div>
                        <div className="row-desc">{t("ov_about_desc")}</div>
                    </div>
                    <div className="row-control">
                        <button className="btn" onClick={onAbout}>
                            {t("ov_view")}
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
}
