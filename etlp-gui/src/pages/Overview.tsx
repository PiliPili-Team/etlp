import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ServerStatus {
    running: boolean;
    port: number;
}

interface Props {
    addToast: (msg: string, err?: boolean) => void;
}

export default function Overview({ addToast }: Props) {
    const [status, setStatus] = useState<ServerStatus>({ running: false, port: 58000 });
    const [busy, setBusy] = useState(false);
    const [startTime, setStartTime] = useState<Date | null>(null);
    const [elapsed, setElapsed] = useState("");

    const refreshStatus = useCallback(async () => {
        try {
            const s = await invoke<ServerStatus>("get_server_status");
            setStatus(s);
        } catch { /* ignore */ }
    }, []);

    useEffect(() => {
        void refreshStatus();
        const iv = setInterval(refreshStatus, 5000);
        return () => clearInterval(iv);
    }, [refreshStatus]);

    // Uptime counter
    useEffect(() => {
        if (!status.running) { setStartTime(null); return; }
        if (!startTime) setStartTime(new Date());
    }, [status.running]);

    useEffect(() => {
        if (!startTime) { setElapsed(""); return; }
        const iv = setInterval(() => {
            const secs = Math.floor((Date.now() - startTime.getTime()) / 1000);
            const h = Math.floor(secs / 3600);
            const m = Math.floor((secs % 3600) / 60);
            const s = secs % 60;
            setElapsed(
                h > 0
                    ? `${h}h ${m}m ${s}s`
                    : m > 0
                    ? `${m}m ${s}s`
                    : `${s}s`,
            );
        }, 1000);
        return () => clearInterval(iv);
    }, [startTime]);

    const handleStart = useCallback(async () => {
        setBusy(true);
        try {
            const port = await invoke<number>("start_server");
            setStatus({ running: true, port });
            setStartTime(new Date());
            addToast(`服务已启动，端口 ${port}`);
        } catch (e) {
            addToast(String(e), true);
        } finally {
            setBusy(false);
        }
    }, [addToast]);

    const handleStop = useCallback(async () => {
        setBusy(true);
        try {
            await invoke("stop_server");
            setStatus((s) => ({ ...s, running: false }));
            setStartTime(null);
            addToast("服务已停止");
        } catch (e) {
            addToast(String(e), true);
        } finally {
            setBusy(false);
        }
    }, [addToast]);

    const handleReload = useCallback(async () => {
        try {
            await invoke("reload_config");
            addToast("配置已重新加载");
        } catch (e) {
            addToast(String(e), true);
        }
    }, [addToast]);

    const handleOpenFolder = useCallback(async () => {
        try { await invoke("open_config_folder"); }
        catch (e) { addToast(String(e), true); }
    }, [addToast]);

    const handleEditConfig = useCallback(async () => {
        try { await invoke("edit_config"); }
        catch (e) { addToast(String(e), true); }
    }, [addToast]);

    return (
        <>
            <div className="page-title">概览</div>

            {/* Hero card */}
            <div className="overview-hero">
                <div className="overview-hero-header">
                    <div>
                        <div className="overview-hero-title">本地服务</div>
                        <div style={{ marginTop: 6 }}>
                            <span
                                className={`status-badge ${status.running ? "running" : "stopped"}`}
                            >
                                <span className="status-dot" />
                                {status.running ? "运行中" : "已停止"}
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
                                启动
                            </button>
                        ) : (
                            <button
                                className="btn btn-danger"
                                onClick={handleStop}
                                disabled={busy}
                            >
                                停止
                            </button>
                        )}
                    </div>
                </div>

                <div className="overview-meta">
                    <div className="meta-item">
                        <div className="meta-label">端口</div>
                        <div>
                            <span className="meta-value">{status.port}</span>
                        </div>
                    </div>
                    <div className="meta-item">
                        <div className="meta-label">运行时间</div>
                        <div>
                            <span className="meta-value" style={{ fontSize: 15 }}>
                                {elapsed || (status.running ? "—" : "—")}
                            </span>
                        </div>
                    </div>
                    <div className="meta-item">
                        <div className="meta-label">监听地址</div>
                        <div>
                            <span className="meta-value" style={{ fontSize: 13 }}>
                                127.0.0.1
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            {/* Config actions */}
            <div className="settings-group">
                <div className="row">
                    <div className="row-label">
                        <div>配置文件</div>
                        <div className="row-desc">查看或编辑当前配置</div>
                    </div>
                    <div className="row-control">
                        <button className="btn" onClick={handleOpenFolder}>
                            打开目录
                        </button>
                        <button className="btn" onClick={handleEditConfig}>
                            编辑配置
                        </button>
                    </div>
                </div>
                <div className="row">
                    <div className="row-label">
                        <div>重新加载</div>
                        <div className="row-desc">从磁盘重新读取配置，立即生效</div>
                    </div>
                    <div className="row-control">
                        <button
                            className="btn"
                            onClick={handleReload}
                            disabled={!status.running}
                        >
                            重新加载
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
}
