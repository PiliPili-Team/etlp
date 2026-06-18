import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { usePlatform } from "./hooks/usePlatform";

// ── Types ──────────────────────────────────────────────────────────────────────

interface ServerStatus {
    running: boolean;
    port: number;
}

interface Toast {
    id: number;
    message: string;
    error: boolean;
}

// ── App ────────────────────────────────────────────────────────────────────────

export default function App() {
    const platform = usePlatform();
    const [status, setStatus] = useState<ServerStatus>({
        running: false,
        port: 58000,
    });
    const [autostart, setAutostart] = useState(false);
    const [busy, setBusy] = useState(false);
    const [toasts, setToasts] = useState<Toast[]>([]);
    const toastId = useRef(0);

    // Apply platform class to body once on mount.
    useEffect(() => {
        document.body.className = platform !== "unknown" ? `platform-${platform}` : "";
    }, [platform]);

    const addToast = useCallback((message: string, error = false) => {
        const id = ++toastId.current;
        setToasts((prev) => [...prev, { id, message, error }]);
        setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 3000);
    }, []);

    const refreshStatus = useCallback(async () => {
        try {
            const s = await invoke<ServerStatus>("get_server_status");
            setStatus(s);
        } catch {
            // ignore
        }
    }, []);

    // Fetch initial status on mount
    useEffect(() => {
        void refreshStatus();
        invoke<boolean>("get_autostart")
            .then(setAutostart)
            .catch(() => {});
    }, [refreshStatus]);

    const handleStart = useCallback(async () => {
        setBusy(true);
        try {
            const port = await invoke<number>("start_server");
            setStatus({ running: true, port });
            addToast(`Server started on port ${port}`);
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
            addToast("Server stopped");
        } catch (e) {
            addToast(String(e), true);
        } finally {
            setBusy(false);
        }
    }, [addToast]);

    const handleReloadConfig = useCallback(async () => {
        try {
            await invoke("reload_config");
            addToast("Config reloaded");
        } catch (e) {
            addToast(String(e), true);
        }
    }, [addToast]);

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

    const handleAutostartToggle = useCallback(async () => {
        const next = !autostart;
        try {
            await invoke("set_autostart", { enabled: next });
            setAutostart(next);
            addToast(next ? "Launch at login enabled" : "Launch at login disabled");
        } catch (e) {
            addToast(String(e), true);
        }
    }, [autostart, addToast]);

    const isMac = platform === "macos";

    return (
        <div className="app">
            {isMac && (
                <div className="titlebar">
                    <span className="titlebar-title">etlp</span>
                </div>
            )}

            <div className="main-content">
                {/* Server control card */}
                <div className="card">
                    <div className="card-title">Server</div>

                    <div className="status-row">
                        <span
                            className={`status-dot ${status.running ? "running" : "stopped"}`}
                        />
                        <span className="status-label">
                            {status.running ? "Running" : "Stopped"}
                        </span>
                        {status.running && (
                            <span className="status-port">:{status.port}</span>
                        )}
                    </div>

                    <div className="btn-row">
                        <button
                            className="btn btn-primary"
                            onClick={handleStart}
                            disabled={busy || status.running}
                        >
                            Start
                        </button>
                        <button
                            className="btn btn-danger"
                            onClick={handleStop}
                            disabled={busy || !status.running}
                        >
                            Stop
                        </button>
                    </div>
                </div>

                {/* Config card */}
                <div className="card">
                    <div className="card-title">Configuration</div>
                    <div className="btn-row">
                        <button
                            className="btn"
                            onClick={handleReloadConfig}
                            disabled={!status.running}
                        >
                            Reload Config
                        </button>
                        <button className="btn" onClick={handleOpenFolder}>
                            Open Folder
                        </button>
                        <button className="btn" onClick={handleEditConfig}>
                            Edit Config
                        </button>
                    </div>
                </div>

                {/* System card */}
                <div className="card">
                    <div className="card-title">System</div>
                    <div className="toggle-row">
                        <div>
                            <div className="toggle-label">Launch at Login</div>
                            <div className="toggle-desc">
                                Start etlp automatically when you log in
                            </div>
                        </div>
                        <label className="toggle">
                            <input
                                type="checkbox"
                                checked={autostart}
                                onChange={handleAutostartToggle}
                            />
                            <span className="toggle-track">
                                <span className="toggle-thumb" />
                            </span>
                        </label>
                    </div>
                </div>
            </div>

            {/* Toast notifications */}
            <div className="toast-area">
                {toasts.map((t) => (
                    <div key={t.id} className={`toast ${t.error ? "error" : ""}`}>
                        {t.message}
                    </div>
                ))}
            </div>
        </div>
    );
}
