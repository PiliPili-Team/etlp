import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { UpdateInfo } from "../App";

interface ServerStatus {
    running: boolean;
    port: number;
    /** Seconds since the service started, or null while stopped. */
    uptime_secs: number | null;
}

interface Props {
    addToast: (msg: string, err?: boolean) => void;
    /** Pending update info from the app-level check, or null when none. */
    update: UpdateInfo | null;
    onDismissUpdate: () => void;
}

// Re-anchor the local uptime clock to the backend only when it drifts beyond
// this, so the per-second ticker stays smooth between 5s status polls while
// still self-correcting against wall-clock changes or a service restart.
const UPTIME_RESYNC_MS = 2000;

export default function Overview({ addToast, update, onDismissUpdate }: Props) {
    const t = useI18n();
    const [status, setStatus] = useState<ServerStatus>({
        running: false,
        port: 58000,
        uptime_secs: null,
    });
    const [busy, setBusy] = useState(false);
    // Local anchor for the per-second display, derived from the backend's
    // authoritative uptime. Never persisted: the in-process service dies with
    // the app, so a fresh launch must start from the real (possibly zero)
    // uptime, not a stale client timestamp.
    const [startTime, setStartTime] = useState<Date | null>(null);
    const [elapsed, setElapsed] = useState("");

    const [downloading, setDownloading] = useState(false);
    const [downloadErr, setDownloadErr] = useState<string | null>(null);

    const doInstallUpdate = useCallback(async () => {
        if (!update || downloading) return;
        setDownloading(true);
        setDownloadErr(null);
        try {
            await invoke("download_and_apply_update", {
                version: update.latest,
            });
        } catch (e) {
            setDownloadErr(String(e));
        } finally {
            setDownloading(false);
        }
    }, [update, downloading]);

    // Anchor `startTime` to `now - uptime`, keeping the existing anchor when it
    // is already within tolerance so the local ticker does not jump each poll.
    const syncUptime = useCallback((s: ServerStatus) => {
        if (s.running && s.uptime_secs != null) {
            const anchor = Date.now() - s.uptime_secs * 1000;
            setStartTime((prev) =>
                prev && Math.abs(prev.getTime() - anchor) < UPTIME_RESYNC_MS
                    ? prev
                    : new Date(anchor),
            );
        } else {
            setStartTime(null);
        }
    }, []);

    const refreshStatus = useCallback(async () => {
        try {
            const s = await invoke<ServerStatus>("get_server_status");
            setStatus(s);
            syncUptime(s);
        } catch {
            /* ignore */
        }
    }, [syncUptime]);

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

    // Show a localized failure label plus the raw backend detail (kept for
    // diagnosis), e.g. "Failed to start service: bind ...".
    const failToast = useCallback(
        (key: Parameters<typeof t>[0], e: unknown) =>
            addToast(`${t(key)}: ${String(e)}`, true),
        [addToast, t],
    );

    const handleStart = useCallback(async () => {
        setBusy(true);
        try {
            const port = await invoke<number>("start_server");
            setStatus({ running: true, port, uptime_secs: 0 });
            setStartTime(new Date());
            addToast(t("toast_started", { port }));
        } catch (e) {
            failToast("toast_start_failed", e);
        } finally {
            setBusy(false);
        }
    }, [addToast, t, failToast]);

    const handleStop = useCallback(async () => {
        setBusy(true);
        try {
            await invoke("stop_server");
            setStatus((s) => ({ ...s, running: false, uptime_secs: null }));
            setStartTime(null);
            addToast(t("toast_stopped"));
        } catch (e) {
            failToast("toast_stop_failed", e);
        } finally {
            setBusy(false);
        }
    }, [addToast, t, failToast]);

    return (
        <>
            <div className="page-title">{t("page_overview")}</div>

            {update && (
                <div className="update-banner">
                    <span className="update-banner-dot" />
                    <span className="update-banner-text">
                        {downloading
                            ? t("ov_update_downloading")
                            : downloadErr
                              ? `${t("ov_update_failed")}: ${downloadErr}`
                              : t("ov_update_available", {
                                    version: update.latest,
                                })}
                    </span>
                    <button
                        className="update-banner-btn"
                        onClick={() => void doInstallUpdate()}
                        disabled={downloading}
                    >
                        {t("ov_update_action")}
                    </button>
                    <button
                        className="update-banner-close"
                        title={t("ov_update_dismiss")}
                        onClick={onDismissUpdate}
                        disabled={downloading}
                    >
                        ✕
                    </button>
                </div>
            )}

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
        </>
    );
}
