import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

// ── Types ──────────────────────────────────────────────────────────────────────

interface ConfigDto {
    player: string;
    fullscreen: boolean;
    disable_audio: boolean;
    player_path: string;
    version_prefer: string[];
    subtitle_priority: string[];
    sub_extract_priority: string[];
    pretty_title: boolean;
    kill_process_at_start: boolean;
    last_ep_disable_playlist: boolean;
    version_prefer_for_playlist: boolean;
    http_proxy: string;
    redirect_check_host: string[];
    skip_certificate_verify: boolean;
    log_level: string;
    user_agent: string;
    mix_log: boolean;
    item_limit: number;
    version_filter: string;
    speed_limit_mb: number;
    trakt_client_id: string;
    trakt_client_secret: string;
    trakt_enable_host: string;
    bangumi_access_token: string;
    config_path: string;
}

type SectionTab = "player" | "playlist" | "network" | "system";

interface Props {
    section: SectionTab;
    addToast: (msg: string, err?: boolean) => void;
}

// ── Delta patch ────────────────────────────────────────────────────────────────

async function patch(
    section: string,
    key: string,
    value: unknown,
): Promise<void> {
    await invoke("update_config_field", { section, key, value });
}

// ── Reusable row components ────────────────────────────────────────────────────

interface ToggleRowProps {
    label: string;
    desc?: string;
    checked: boolean;
    onChange: (v: boolean) => void;
}

function ToggleRow({ label, desc, checked, onChange }: ToggleRowProps) {
    return (
        <div className="row">
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div className="row-control">
                <label className="toggle">
                    <input
                        type="checkbox"
                        checked={checked}
                        onChange={(e) => onChange(e.target.checked)}
                    />
                    <span className="toggle-track">
                        <span className="toggle-thumb" />
                    </span>
                </label>
            </div>
        </div>
    );
}

interface InputRowProps {
    label: string;
    desc?: string;
    value: string;
    placeholder?: string;
    mono?: boolean;
    onCommit: (v: string) => void;
}

function InputRow({
    label,
    desc,
    value,
    placeholder,
    mono,
    onCommit,
}: InputRowProps) {
    const [local, setLocal] = useState(value);
    // Keep in sync if parent reloads config
    useEffect(() => setLocal(value), [value]);

    return (
        <div className="row">
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div className="row-control">
                <input
                    className={`input${mono ? " code" : ""}`}
                    value={local}
                    placeholder={placeholder ?? ""}
                    onChange={(e) => setLocal(e.target.value)}
                    onBlur={() => {
                        if (local !== value) onCommit(local);
                    }}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") {
                            (e.target as HTMLInputElement).blur();
                        }
                    }}
                />
            </div>
        </div>
    );
}

interface NumberRowProps {
    label: string;
    desc?: string;
    value: number;
    min?: number;
    max?: number;
    onCommit: (v: number) => void;
}

function NumberRow({ label, desc, value, min, max, onCommit }: NumberRowProps) {
    const [local, setLocal] = useState(String(value));
    useEffect(() => setLocal(String(value)), [value]);
    return (
        <div className="row">
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div className="row-control">
                <input
                    className="input narrow"
                    type="number"
                    value={local}
                    min={min}
                    max={max}
                    onChange={(e) => setLocal(e.target.value)}
                    onBlur={() => {
                        const n = parseInt(local, 10);
                        if (!isNaN(n) && n !== value) onCommit(n);
                    }}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                    }}
                />
            </div>
        </div>
    );
}

interface SelectRowProps {
    label: string;
    desc?: string;
    value: string;
    options: { value: string; label: string }[];
    onChange: (v: string) => void;
}

function SelectRow({ label, desc, value, options, onChange }: SelectRowProps) {
    return (
        <div className="row">
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div className="row-control">
                <select
                    className="select"
                    value={value}
                    onChange={(e) => onChange(e.target.value)}
                >
                    {options.map((o) => (
                        <option key={o.value} value={o.value}>
                            {o.label}
                        </option>
                    ))}
                </select>
            </div>
        </div>
    );
}

interface TagListRowProps {
    label: string;
    desc?: string;
    tags: string[];
    placeholder?: string;
    onAdd: (tag: string) => void;
    onRemove: (index: number) => void;
}

function TagListRow({
    label,
    desc,
    tags,
    placeholder,
    onAdd,
    onRemove,
}: TagListRowProps) {
    const [input, setInput] = useState("");

    const handleAdd = () => {
        const trimmed = input.trim();
        if (trimmed && !tags.includes(trimmed)) {
            onAdd(trimmed);
            setInput("");
        }
    };

    return (
        <div className="row" style={{ flexDirection: "column", alignItems: "flex-start", gap: 8 }}>
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div style={{ width: "100%" }}>
                {tags.length > 0 && (
                    <div className="tag-list">
                        {tags.map((tag, i) => (
                            <span key={i} className="tag">
                                {tag}
                                <button
                                    className="tag-remove"
                                    onClick={() => onRemove(i)}
                                    title="删除"
                                >
                                    ×
                                </button>
                            </span>
                        ))}
                    </div>
                )}
                <div className="tag-add-row">
                    <input
                        className="tag-input"
                        value={input}
                        placeholder={placeholder ?? "输入后按 Enter 添加"}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={(e) => {
                            if (e.key === "Enter") handleAdd();
                        }}
                    />
                    <button className="btn" onClick={handleAdd}>
                        添加
                    </button>
                </div>
            </div>
        </div>
    );
}

interface TextareaRowProps {
    label: string;
    desc?: string;
    value: string;
    placeholder?: string;
    onCommit: (v: string) => void;
}

function TextareaRow({ label, desc, value, placeholder, onCommit }: TextareaRowProps) {
    const [local, setLocal] = useState(value);
    useEffect(() => setLocal(value), [value]);
    return (
        <div className="row" style={{ flexDirection: "column", alignItems: "flex-start", gap: 8 }}>
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <textarea
                style={{
                    width: "100%",
                    minHeight: 90,
                    background: "var(--surface-alt)",
                    border: "1px solid var(--border)",
                    borderRadius: 7,
                    padding: "6px 10px",
                    fontFamily: '"SF Mono", "Cascadia Code", monospace',
                    fontSize: 12,
                    color: "var(--text)",
                    resize: "vertical",
                    outline: "none",
                }}
                value={local}
                placeholder={placeholder}
                onChange={(e) => setLocal(e.target.value)}
                onBlur={() => { if (local !== value) onCommit(local); }}
            />
        </div>
    );
}

// ── Sections ───────────────────────────────────────────────────────────────────

const PLAYERS = [
    { value: "mpv",        label: "mpv" },
    { value: "iina",       label: "IINA (macOS)" },
    { value: "vlc",        label: "VLC" },
    { value: "mpc-hc",     label: "MPC-HC (Windows)" },
    { value: "potplayer",  label: "PotPlayer (Windows)" },
    { value: "dandanplay", label: "弹弹Play" },
];

const LOG_LEVELS = [
    { value: "error", label: "Error" },
    { value: "warn",  label: "Warn" },
    { value: "info",  label: "Info (默认)" },
    { value: "debug", label: "Debug" },
    { value: "trace", label: "Trace" },
];

// ── Main component ─────────────────────────────────────────────────────────────

export default function Settings({ section, addToast }: Props) {
    const [cfg, setCfg] = useState<ConfigDto | null>(null);
    const [autostart, setAutostart] = useState(false);
    const loaded = useRef(false);

    const loadConfig = useCallback(async () => {
        try {
            const [c, a] = await Promise.all([
                invoke<ConfigDto>("get_config"),
                invoke<boolean>("get_autostart"),
            ]);
            setCfg(c);
            setAutostart(a);
            loaded.current = true;
        } catch (e) {
            addToast(String(e), true);
        }
    }, [addToast]);

    useEffect(() => { void loadConfig(); }, [loadConfig]);

    // Generic delta-update helper. Only writes to file; does NOT reload the
    // server — call `reload_config` explicitly when running.
    const update = useCallback(
        async (sec: string, key: string, value: unknown) => {
            try {
                await patch(sec, key, value);
                // optimistic local state update
                setCfg((prev) =>
                    prev
                        ? ({ ...prev, [key.replace(/\./g, "_")]: value } as ConfigDto)
                        : prev,
                );
            } catch (e) {
                addToast(String(e), true);
            }
        },
        [addToast],
    );

    const handleAutostart = useCallback(
        async (enabled: boolean) => {
            try {
                await invoke("set_autostart", { enabled });
                setAutostart(enabled);
                addToast(enabled ? "开机自启动已开启" : "开机自启动已关闭");
            } catch (e) {
                addToast(String(e), true);
            }
        },
        [addToast],
    );

    if (!cfg) {
        return (
            <div style={{ color: "var(--text-3)", padding: 40, textAlign: "center" }}>
                加载配置中…
            </div>
        );
    }

    if (section === "player") return <PlayerSection cfg={cfg} update={update} />;
    if (section === "playlist") return <PlaylistSection cfg={cfg} update={update} />;
    if (section === "network") return <NetworkSection cfg={cfg} update={update} />;
    if (section === "system")
        return (
            <SystemSection
                cfg={cfg}
                update={update}
                autostart={autostart}
                onAutostart={handleAutostart}
            />
        );
    return null;
}

// ── Player section ─────────────────────────────────────────────────────────────

function PlayerSection({
    cfg,
    update,
}: {
    cfg: ConfigDto;
    update: (sec: string, key: string, value: unknown) => void;
}) {
    return (
        <>
            <div className="page-title">播放器</div>

            <div className="settings-group">
                <SelectRow
                    label="播放器类型"
                    desc="选择本地视频播放器"
                    value={cfg.player}
                    options={PLAYERS}
                    onChange={(v) => update("emby", "player", v)}
                />
                <InputRow
                    label="播放器路径"
                    desc="可选；留空使用系统 PATH 中的播放器"
                    value={cfg.player_path}
                    placeholder="例：/opt/homebrew/bin/mpv"
                    mono
                    onCommit={(v) => update("dev", "player_path", v || null)}
                />
            </div>

            <div className="settings-group-title">启动选项</div>
            <div className="settings-group">
                <ToggleRow
                    label="全屏模式"
                    desc="启动播放器时自动全屏"
                    checked={cfg.fullscreen}
                    onChange={(v) => update("emby", "fullscreen", v)}
                />
                <ToggleRow
                    label="静音启动"
                    desc="启动时关闭音频（mpv --no-audio）"
                    checked={cfg.disable_audio}
                    onChange={(v) => update("emby", "disable_audio", v)}
                />
                <ToggleRow
                    label="美化标题"
                    desc="将服务器名称拼接到播放器窗口标题"
                    checked={cfg.pretty_title}
                    onChange={(v) => update("dev", "pretty_title", v)}
                />
                <ToggleRow
                    label="启动时清理进程"
                    desc="etlp 启动时关闭已有的同名播放器进程"
                    checked={cfg.kill_process_at_start}
                    onChange={(v) => update("dev", "kill_process_at_start", v)}
                />
            </div>
        </>
    );
}

// ── Playlist section ───────────────────────────────────────────────────────────

function PlaylistSection({
    cfg,
    update,
}: {
    cfg: ConfigDto;
    update: (sec: string, key: string, value: unknown) => void;
}) {
    const addVersionTag = (tag: string) =>
        update("dev", "version_prefer", [...cfg.version_prefer, tag]);
    const removeVersionTag = (i: number) =>
        update("dev", "version_prefer", cfg.version_prefer.filter((_, j) => j !== i));

    const addSubTag = (tag: string) =>
        update("dev", "subtitle_priority", [...cfg.subtitle_priority, tag]);
    const removeSubTag = (i: number) =>
        update("dev", "subtitle_priority", cfg.subtitle_priority.filter((_, j) => j !== i));

    const addSubExtTag = (tag: string) =>
        update("dev", "sub_extract_priority", [...cfg.sub_extract_priority, tag]);
    const removeSubExtTag = (i: number) =>
        update("dev", "sub_extract_priority", cfg.sub_extract_priority.filter((_, j) => j !== i));

    return (
        <>
            <div className="page-title">播放列表</div>

            <div className="settings-group-title">版本偏好</div>
            <div className="settings-group">
                <TagListRow
                    label="版本优先级"
                    desc="按顺序匹配视频版本关键词，排在前面的优先选择"
                    tags={cfg.version_prefer}
                    placeholder="例：VCB-Studio、ANi、DBD-Raws"
                    onAdd={addVersionTag}
                    onRemove={removeVersionTag}
                />
                <ToggleRow
                    label="播放列表使用版本偏好"
                    desc="组装播放列表时同样按版本优先级筛选"
                    checked={cfg.version_prefer_for_playlist}
                    onChange={(v) => update("dev", "version_prefer_for_playlist", v)}
                />
            </div>

            <div className="settings-group-title">字幕偏好</div>
            <div className="settings-group">
                <TagListRow
                    label="字幕优先级"
                    desc="按顺序匹配字幕轨关键词"
                    tags={cfg.subtitle_priority}
                    placeholder="例：简中、CHS、简繁"
                    onAdd={addSubTag}
                    onRemove={removeSubTag}
                />
                <TagListRow
                    label="跨版本字幕提取"
                    desc="当前版本无字幕时从其他版本中提取"
                    tags={cfg.sub_extract_priority}
                    placeholder="例：CHS、简体"
                    onAdd={addSubExtTag}
                    onRemove={removeSubExtTag}
                />
            </div>

            <div className="settings-group-title">列表限制</div>
            <div className="settings-group">
                <NumberRow
                    label="最大集数"
                    desc="单次播放最多加入播放列表的集数（0 = 不限）"
                    value={cfg.item_limit}
                    min={0}
                    max={999}
                    onCommit={(v) => update("playlist", "item_limit", v)}
                />
                <ToggleRow
                    label="最后一集禁用列表"
                    desc="当前集是最后一集时不添加后续集"
                    checked={cfg.last_ep_disable_playlist}
                    onChange={(v) => update("dev", "last_ep_disable_playlist", v)}
                />
                <TextareaRow
                    label="版本过滤正则"
                    desc="只有匹配该正则的版本才会加入播放列表（留空不过滤）"
                    value={cfg.version_filter}
                    placeholder="例：(?i)(vcb|ani|bdrip)"
                    onCommit={(v) => update("playlist", "version_filter", v)}
                />
            </div>
        </>
    );
}

// ── Network section ────────────────────────────────────────────────────────────

function NetworkSection({
    cfg,
    update,
}: {
    cfg: ConfigDto;
    update: (sec: string, key: string, value: unknown) => void;
}) {
    const addRedirectHost = (h: string) =>
        update("dev", "redirect_check_host", [...cfg.redirect_check_host, h]);
    const removeRedirectHost = (i: number) =>
        update("dev", "redirect_check_host", cfg.redirect_check_host.filter((_, j) => j !== i));

    return (
        <>
            <div className="page-title">网络</div>

            <div className="settings-group">
                <InputRow
                    label="HTTP 代理"
                    desc="格式：host:port（留空不使用）"
                    value={cfg.http_proxy}
                    placeholder="例：127.0.0.1:7890"
                    mono
                    onCommit={(v) => update("dev", "http_proxy", v || null)}
                />
                <ToggleRow
                    label="跳过 TLS 证书验证"
                    desc="用于自签名证书的 Emby 服务器（不安全）"
                    checked={cfg.skip_certificate_verify}
                    onChange={(v) => update("dev", "skip_certificate_verify", v)}
                />
                <InputRow
                    label="User-Agent"
                    desc="自定义 HTTP 请求 UA；下载/预取 UA 固定不可修改"
                    value={cfg.user_agent}
                    placeholder="留空使用内置 etlp"
                    onCommit={(v) => update("dev", "user_agent", v || null)}
                />
            </div>

            <div className="settings-group-title">重定向检测</div>
            <div className="settings-group">
                <TagListRow
                    label="需检测重定向的主机"
                    desc="这些主机的流媒体 URL 会先探测 30x 跳转再交给播放器"
                    tags={cfg.redirect_check_host}
                    placeholder="例：cdn.example.com"
                    onAdd={addRedirectHost}
                    onRemove={removeRedirectHost}
                />
            </div>
        </>
    );
}

// ── System section ─────────────────────────────────────────────────────────────

function SystemSection({
    cfg,
    update,
    autostart,
    onAutostart,
}: {
    cfg: ConfigDto;
    update: (sec: string, key: string, value: unknown) => void;
    autostart: boolean;
    onAutostart: (v: boolean) => void;
}) {
    return (
        <>
            <div className="page-title">系统</div>

            <div className="settings-group">
                <ToggleRow
                    label="开机自启动"
                    desc="登录后自动启动 etlp 应用"
                    checked={autostart}
                    onChange={onAutostart}
                />
            </div>

            <div className="settings-group-title">日志</div>
            <div className="settings-group">
                <SelectRow
                    label="日志级别"
                    desc="调试时可设置为 Debug 获取更多信息"
                    value={cfg.log_level}
                    options={LOG_LEVELS}
                    onChange={(v) => update("dev", "log_level", v)}
                />
                <ToggleRow
                    label="日志脱敏"
                    desc="将日志中的敏感 Token 替换为占位符"
                    checked={cfg.mix_log}
                    onChange={(v) => update("dev", "mix_log", v)}
                />
            </div>

            <div className="settings-group-title">下载</div>
            <div className="settings-group">
                <NumberRow
                    label="下载速度限制 (MiB/s)"
                    desc="0 表示不限速"
                    value={cfg.speed_limit_mb}
                    min={0}
                    onCommit={(v) => update("gui", "speed_limit_mb", v)}
                />
            </div>

            <div className="settings-group-title">Trakt.tv 集成</div>
            <div className="settings-group">
                <InputRow
                    label="Client ID"
                    value={cfg.trakt_client_id}
                    placeholder="留空禁用 Trakt 集成"
                    mono
                    onCommit={(v) => update("trakt", "client_id", v)}
                />
                <InputRow
                    label="Client Secret"
                    value={cfg.trakt_client_secret}
                    placeholder=""
                    mono
                    onCommit={(v) => update("trakt", "client_secret", v)}
                />
                <InputRow
                    label="触发主机"
                    desc="匹配该域名的请求才会触发 Trakt 打点"
                    value={cfg.trakt_enable_host}
                    placeholder="例：emby.example.com"
                    mono
                    onCommit={(v) => update("trakt", "enable_host", v)}
                />
            </div>

            <div className="settings-group-title">Bangumi.tv 集成</div>
            <div className="settings-group">
                <InputRow
                    label="Access Token"
                    value={cfg.bangumi_access_token}
                    placeholder="留空禁用 Bangumi 集成"
                    mono
                    onCommit={(v) => update("bangumi", "access_token", v || null)}
                />
            </div>

            <div className="settings-group-title" style={{ marginTop: 28 }}>关于</div>
            <div className="settings-group">
                <div className="row">
                    <div className="row-label">配置文件路径</div>
                    <div
                        className="row-control"
                        style={{ fontFamily: "monospace", fontSize: 11, color: "var(--text-3)", maxWidth: 320, overflow: "hidden", textOverflow: "ellipsis" }}
                        title={cfg.config_path}
                    >
                        {cfg.config_path || "—"}
                    </div>
                </div>
            </div>
        </>
    );
}
