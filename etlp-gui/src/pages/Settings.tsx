import { useState, useEffect, useLayoutEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LangMode, DisplaySettings, AccentColor } from "../display";
import { ACCENT_PALETTES } from "../display";
import { useI18n } from "../i18n";

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
    mix_log: boolean;
    item_limit: number;
    version_filter: string;
    speed_limit_mb: number;
    disable_progress_report: boolean;
    trakt_client_id: string;
    trakt_client_secret: string;
    trakt_enable_host: string;
    bangumi_access_token: string;
    config_path: string;
}

type SectionTab = "player" | "version-prefer" | "network" | "system";

interface Props {
    section: SectionTab;
    addToast: (msg: string, err?: boolean) => void;
    display: DisplaySettings;
    onDisplayChange: (patch: Partial<DisplaySettings>) => void;
}

// ── Delta patch ────────────────────────────────────────────────────────────────

async function patch(section: string, key: string, value: unknown): Promise<void> {
    await invoke("update_config_field", { section, key, value });
}

// ── Row components ─────────────────────────────────────────────────────────────

function ToggleRow({
    label,
    desc,
    checked,
    onChange,
}: {
    label: string;
    desc?: string;
    checked: boolean;
    onChange: (v: boolean) => void;
}) {
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

function InputRow({
    label,
    desc,
    value,
    placeholder,
    mono,
    onCommit,
}: {
    label: string;
    desc?: string;
    value: string;
    placeholder?: string;
    mono?: boolean;
    onCommit: (v: string) => void;
}) {
    const [local, setLocal] = useState(value);
    const [prevStrValue, setPrevStrValue] = useState(value);
    if (prevStrValue !== value) {
        setPrevStrValue(value);
        setLocal(value);
    }
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
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                    }}
                />
            </div>
        </div>
    );
}

function NumberRow({
    label,
    desc,
    value,
    min,
    max,
    onCommit,
}: {
    label: string;
    desc?: string;
    value: number;
    min?: number;
    max?: number;
    onCommit: (v: number) => void;
}) {
    const [local, setLocal] = useState(String(value));
    const [prevNumValue, setPrevNumValue] = useState(value);
    if (prevNumValue !== value) {
        setPrevNumValue(value);
        setLocal(String(value));
    }
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
                        let n = parseInt(local, 10);
                        if (isNaN(n)) {
                            setLocal(String(value));
                            return;
                        }
                        // Clamp into [min, max] so out-of-range input (e.g. a
                        // negative episode cap) is corrected instead of saved.
                        if (min !== undefined && n < min) n = min;
                        if (max !== undefined && n > max) n = max;
                        setLocal(String(n));
                        if (n !== value) onCommit(n);
                    }}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                    }}
                />
            </div>
        </div>
    );
}

function SelectRow({
    label,
    desc,
    value,
    options,
    onChange,
}: {
    label: string;
    desc?: string;
    value: string;
    options: { value: string; label: string }[];
    onChange: (v: string) => void;
}) {
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

/** Return a copy of `list` with the item at `from` moved to index `to`. */
function reorder<T>(list: T[], from: number, to: number): T[] {
    const next = [...list];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    return next;
}

/** Index of the tag under a screen point, or null. The dragged tag carries
 *  `pointer-events: none`, so this hit-tests through to the tag beneath it. */
function tagIndexAtPoint(x: number, y: number): number | null {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-tag-index]");
    const idx = el ? Number(el.dataset.tagIndex) : NaN;
    return Number.isInteger(idx) ? idx : null;
}

function TagListRow({
    label,
    desc,
    tags,
    placeholder,
    onAdd,
    onRemove,
    onReorder,
}: {
    label: string;
    desc?: string;
    tags: string[];
    placeholder?: string;
    onAdd: (tag: string) => void;
    onRemove: (index: number) => void;
    onReorder?: (from: number, to: number) => void;
}) {
    const t = useI18n();
    const [input, setInput] = useState("");
    // Pointer-driven reordering instead of HTML5 drag-and-drop: Tauri's WebView
    // intercepts native drag-and-drop at the OS level, so `dragstart`/`drop`
    // never reach the document. The dragged tag follows the cursor live and
    // the order commits on release.
    const [dragIndex, setDragIndex] = useState<number | null>(null);
    const [overIndex, setOverIndex] = useState<number | null>(null);
    const [dragDelta, setDragDelta] = useState({ x: 0, y: 0 });
    // Drag-to-reorder is opt-in: only ordered priority lists pass onReorder.
    const reorderable = Boolean(onReorder);
    const handleAdd = () => {
        const tag = input.trim();
        if (tag && !tags.includes(tag)) {
            onAdd(tag);
            setInput("");
        }
    };

    const listRef = useRef<HTMLDivElement>(null);
    const prevRects = useRef<Map<string, DOMRect>>(new Map());
    // Live state of an in-flight drag, read by the window listeners.
    const dragRef = useRef<{
        key: string;
        startX: number;
        startY: number;
        el: HTMLElement;
    } | null>(null);

    const startDrag = (e: React.PointerEvent, i: number) => {
        // Let the remove button keep its click.
        if ((e.target as HTMLElement).closest(".tag-remove")) return;
        e.preventDefault();
        dragRef.current = {
            key: tags[i],
            startX: e.clientX,
            startY: e.clientY,
            el: e.currentTarget as HTMLElement,
        };
        setDragIndex(i);
        setOverIndex(i);
        setDragDelta({ x: 0, y: 0 });
    };

    // While a drag is active, track the pointer on the window so it keeps
    // working past the tag's bounds and in both directions.
    useEffect(() => {
        if (dragIndex === null) return;
        const onMove = (e: PointerEvent) => {
            const d = dragRef.current;
            if (!d) return;
            setDragDelta({ x: e.clientX - d.startX, y: e.clientY - d.startY });
            setOverIndex(tagIndexAtPoint(e.clientX, e.clientY));
        };
        const onUp = (e: PointerEvent) => {
            const d = dragRef.current;
            const target = tagIndexAtPoint(e.clientX, e.clientY);
            // Seed the FLIP with the release position so the tag settles from
            // where it was dropped instead of snapping back to its old slot.
            if (d) prevRects.current.set(d.key, d.el.getBoundingClientRect());
            if (target !== null && target !== dragIndex) {
                onReorder?.(dragIndex, target);
            }
            dragRef.current = null;
            setDragIndex(null);
            setOverIndex(null);
            setDragDelta({ x: 0, y: 0 });
        };
        window.addEventListener("pointermove", onMove);
        window.addEventListener("pointerup", onUp);
        window.addEventListener("pointercancel", onUp);
        return () => {
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", onUp);
            window.removeEventListener("pointercancel", onUp);
        };
    }, [dragIndex, onReorder]);

    // FLIP animation: when the order changes, slide each tag from its previous
    // position to its new one so a dropped tag visibly pushes the others aside
    // instead of snapping. Keyed by tag text since React reuses the DOM node
    // across reorders.
    useLayoutEffect(() => {
        const list = listRef.current;
        if (!list) return;
        const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        const nodes = Array.from(list.querySelectorAll<HTMLElement>("[data-tag-key]"));
        for (const node of nodes) {
            const key = node.dataset.tagKey;
            if (!key) continue;
            const next = node.getBoundingClientRect();
            const prev = prevRects.current.get(key);
            prevRects.current.set(key, next);
            if (reduce || !prev) continue;
            const dx = prev.left - next.left;
            const dy = prev.top - next.top;
            if (dx === 0 && dy === 0) continue;
            // Invert to the old spot, then play back to the new one next frame.
            node.style.transition = "none";
            node.style.transform = `translate(${dx}px, ${dy}px)`;
            requestAnimationFrame(() => {
                node.style.transition = "transform 0.22s cubic-bezier(0.2, 0, 0, 1)";
                node.style.transform = "";
            });
        }
    }, [tags]);

    return (
        <div
            className="row"
            style={{ flexDirection: "column", alignItems: "flex-start", gap: 10 }}
        >
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div style={{ width: "100%" }}>
                {tags.length > 0 && (
                    <div
                        className={`tag-list${dragIndex !== null ? " dragging" : ""}`}
                        ref={listRef}
                    >
                        {tags.map((tag, i) => (
                            <span
                                key={tag}
                                data-tag-index={i}
                                data-tag-key={tag}
                                className={`tag${reorderable ? " tag-draggable" : ""}${
                                    dragIndex === i ? " tag-dragging" : ""
                                }${
                                    overIndex === i &&
                                    dragIndex !== null &&
                                    dragIndex !== i
                                        ? " tag-over"
                                        : ""
                                }`}
                                style={
                                    dragIndex === i
                                        ? {
                                              transform: `translate(${dragDelta.x}px, ${dragDelta.y}px)`,
                                              pointerEvents: "none",
                                              position: "relative",
                                              zIndex: 20,
                                          }
                                        : undefined
                                }
                                onPointerDown={
                                    reorderable ? (e) => startDrag(e, i) : undefined
                                }
                            >
                                {tag}
                                <button
                                    className="tag-remove"
                                    onClick={() => onRemove(i)}
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
                        placeholder={placeholder ?? t("add_placeholder")}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={(e) => {
                            if (e.key === "Enter") handleAdd();
                        }}
                    />
                    <button className="btn" onClick={handleAdd}>
                        {t("add")}
                    </button>
                </div>
            </div>
        </div>
    );
}

function TextareaRow({
    label,
    desc,
    value,
    placeholder,
    onCommit,
}: {
    label: string;
    desc?: string;
    value: string;
    placeholder?: string;
    onCommit: (v: string) => void;
}) {
    const [local, setLocal] = useState(value);
    const [prevTextValue, setPrevTextValue] = useState(value);
    if (prevTextValue !== value) {
        setPrevTextValue(value);
        setLocal(value);
    }
    return (
        <div
            className="row"
            style={{ flexDirection: "column", alignItems: "flex-start", gap: 10 }}
        >
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <textarea
                style={{
                    width: "100%",
                    minHeight: 80,
                    background: "var(--surface-alt)",
                    border: "1.5px solid var(--border)",
                    borderRadius: 8,
                    padding: "7px 10px",
                    fontFamily: '"SF Mono", "Cascadia Code", monospace',
                    fontSize: 12,
                    color: "var(--text)",
                    resize: "vertical",
                    outline: "none",
                    lineHeight: 1.6,
                }}
                value={local}
                placeholder={placeholder}
                onChange={(e) => setLocal(e.target.value)}
                onBlur={() => {
                    if (local !== value) onCommit(local);
                }}
            />
        </div>
    );
}

// ── Player path row ────────────────────────────────────────────────────────────

function PlayerPathRow({
    value,
    onCommit,
}: {
    value: string;
    onCommit: (v: string) => void;
}) {
    const t = useI18n();
    const [local, setLocal] = useState(value);
    const [pathValid, setPathValid] = useState<boolean | null>(null);
    const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const [prevPathValue, setPrevPathValue] = useState(value);
    if (prevPathValue !== value) {
        setPrevPathValue(value);
        setLocal(value);
    }

    const validatePath = useCallback(async (p: string) => {
        if (!p.trim()) {
            setPathValid(null);
            return;
        }
        const exists = await invoke<boolean>("path_exists", { path: p });
        setPathValid(exists);
    }, []);

    const handleChange = (p: string) => {
        setLocal(p);
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => void validatePath(p), 400);
    };

    const handlePick = async () => {
        const picked = await invoke<string | null>("pick_player_path");
        if (picked) {
            setLocal(picked);
            setPathValid(true);
            onCommit(picked);
        }
    };

    return (
        <div
            className="row"
            style={{ flexDirection: "column", alignItems: "stretch", gap: 0 }}
        >
            <div
                style={{ display: "flex", alignItems: "center", gap: 12, minHeight: 54 }}
            >
                <div className="row-label">
                    <div>{t("pl_path")}</div>
                    <div className="row-desc">{t("pl_path_desc")}</div>
                </div>
                <div
                    className="row-control"
                    style={{ flexShrink: 1, minWidth: 0, gap: 6 }}
                >
                    <input
                        className={`input code${pathValid === false ? " error" : ""}`}
                        style={{ flex: 1, minWidth: 140 }}
                        value={local}
                        placeholder="/opt/homebrew/bin/mpv"
                        onChange={(e) => handleChange(e.target.value)}
                        onBlur={() => {
                            if (local !== value) onCommit(local);
                        }}
                        onKeyDown={(e) => {
                            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                        }}
                    />
                    <button
                        className="btn"
                        style={{ whiteSpace: "nowrap", flexShrink: 0 }}
                        onClick={() => void handlePick()}
                    >
                        {t("pl_browse")}
                    </button>
                </div>
            </div>
            {pathValid === false && (
                <div
                    className="path-error-hint"
                    style={{ paddingLeft: 16, paddingBottom: 8 }}
                >
                    {t("pl_path_error")}
                </div>
            )}
        </div>
    );
}

// ── Accent color picker row ────────────────────────────────────────────────────

const ACCENT_NAMES: Record<AccentColor, string> = {
    blue: "蓝色",
    indigo: "靛蓝",
    purple: "紫色",
    pink: "粉色",
    red: "红色",
    orange: "橙色",
    teal: "青色",
    green: "绿色",
};

function AccentColorRow({
    value,
    onChange,
}: {
    value: AccentColor;
    onChange: (v: AccentColor) => void;
}) {
    const t = useI18n();
    return (
        <div className="row">
            <div className="row-label">
                <div>{t("sys_accent")}</div>
                <div className="row-desc">{t("sys_accent_desc")}</div>
            </div>
            <div className="row-control">
                <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
                    {(Object.keys(ACCENT_PALETTES) as AccentColor[]).map((key) => {
                        const [light] = ACCENT_PALETTES[key];
                        const active = value === key;
                        return (
                            <button
                                key={key}
                                title={ACCENT_NAMES[key]}
                                onClick={() => onChange(key)}
                                style={{
                                    width: 24,
                                    height: 24,
                                    borderRadius: "50%",
                                    background: light,
                                    border: active
                                        ? `3px solid ${light}`
                                        : "3px solid transparent",
                                    boxShadow: active
                                        ? `0 0 0 2px var(--surface), 0 0 0 4px ${light}`
                                        : "none",
                                    cursor: "pointer",
                                    padding: 0,
                                    flexShrink: 0,
                                    transition: "box-shadow 0.15s",
                                    outline: "none",
                                }}
                            />
                        );
                    })}
                </div>
            </div>
        </div>
    );
}

// ── Font picker row (pure <select> dropdown) ────────────────────────────────────

function FontPickerRow({
    value,
    onChange,
}: {
    value: string;
    onChange: (v: string) => void;
}) {
    const t = useI18n();
    const [fonts, setFonts] = useState<string[]>([]);

    useEffect(() => {
        invoke<string[]>("list_system_fonts")
            .then(setFonts)
            .catch(() => setFonts(["SF Pro Text", "Helvetica Neue", "Arial", "Roboto"]));
    }, []);

    // Build deduplicated option list: default sentinel + system fonts
    const options = [
        { value: "", label: t("sys_font_default") },
        ...fonts.filter((f) => f.trim()).map((f) => ({ value: f, label: f })),
    ];

    return (
        <div className="row">
            <div className="row-label">
                <div>{t("sys_font")}</div>
                <div className="row-desc">{t("sys_font_desc")}</div>
            </div>
            <div className="row-control">
                <select
                    className="select"
                    value={value}
                    onChange={(e) => onChange(e.target.value)}
                    style={{ minWidth: 180 }}
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

// ── Main component ─────────────────────────────────────────────────────────────

export default function Settings({ section, addToast, display, onDisplayChange }: Props) {
    const t = useI18n();
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

    useEffect(() => {
        const init = setTimeout(loadConfig, 0);
        return () => clearTimeout(init);
    }, [loadConfig]);

    const update = useCallback(
        async (sec: string, key: string, value: unknown) => {
            try {
                await patch(sec, key, value);
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
                addToast(enabled ? t("autostart_on") : t("autostart_off"));
            } catch (e) {
                addToast(String(e), true);
            }
        },
        [addToast, t],
    );

    if (!cfg) {
        return (
            <div style={{ color: "var(--text-3)", padding: 40, textAlign: "center" }}>
                {t("loading")}
            </div>
        );
    }

    if (section === "player") return <PlayerSection cfg={cfg} update={update} />;
    if (section === "version-prefer")
        return <VersionPreferSection cfg={cfg} update={update} />;
    if (section === "network") return <NetworkSection cfg={cfg} update={update} />;
    if (section === "system")
        return (
            <SystemSection
                cfg={cfg}
                update={update}
                autostart={autostart}
                onAutostart={handleAutostart}
                display={display}
                onDisplayChange={onDisplayChange}
            />
        );
    return null;
}

// ── Player ─────────────────────────────────────────────────────────────────────

function PlayerSection({
    cfg,
    update,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
}) {
    const t = useI18n();

    const PLAYERS = [
        { value: "mpv", label: "mpv" },
        { value: "iina", label: "IINA (macOS)" },
        { value: "vlc", label: "VLC" },
        { value: "mpc-hc", label: "MPC-HC (Windows)" },
        { value: "potplayer", label: "PotPlayer (Windows)" },
        { value: "dandanplay", label: "弹弹Play" },
    ];

    return (
        <>
            <div className="page-title">{t("page_player")}</div>
            <div className="settings-group">
                <SelectRow
                    label={t("pl_type")}
                    desc={t("pl_type_desc")}
                    value={cfg.player}
                    options={PLAYERS}
                    onChange={(v) => update("emby", "player", v)}
                />
                <PlayerPathRow
                    value={cfg.player_path}
                    onCommit={(v) => update("dev", "player_path", v || null)}
                />
            </div>
            <div className="settings-group-title">{t("pl_startup")}</div>
            <div className="settings-group">
                <ToggleRow
                    label={t("pl_fullscreen")}
                    desc={t("pl_fullscreen_desc")}
                    checked={cfg.fullscreen}
                    onChange={(v) => update("emby", "fullscreen", v)}
                />
                <ToggleRow
                    label={t("pl_mute")}
                    desc={t("pl_mute_desc")}
                    checked={cfg.disable_audio}
                    onChange={(v) => update("emby", "disable_audio", v)}
                />
                <ToggleRow
                    label={t("pl_pretty_title")}
                    desc={t("pl_pretty_title_desc")}
                    checked={cfg.pretty_title}
                    onChange={(v) => update("dev", "pretty_title", v)}
                />
                <ToggleRow
                    label={t("pl_kill_start")}
                    desc={t("pl_kill_start_desc")}
                    checked={cfg.kill_process_at_start}
                    onChange={(v) => update("dev", "kill_process_at_start", v)}
                />
            </div>
        </>
    );
}

// ── Version prefer ─────────────────────────────────────────────────────────────

function VersionPreferSection({
    cfg,
    update,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
}) {
    const t = useI18n();
    return (
        <>
            <div className="page-title">{t("page_vp")}</div>
            <div className="settings-group-title">{t("vp_priority")}</div>
            <div className="settings-group">
                <TagListRow
                    label={t("vp_keywords")}
                    desc={t("vp_keywords_desc")}
                    tags={cfg.version_prefer}
                    placeholder={t("vp_keywords_placeholder")}
                    onAdd={(tag) =>
                        update("dev", "version_prefer", [...cfg.version_prefer, tag])
                    }
                    onRemove={(i) =>
                        update(
                            "dev",
                            "version_prefer",
                            cfg.version_prefer.filter((_, j) => j !== i),
                        )
                    }
                    onReorder={(from, to) =>
                        update(
                            "dev",
                            "version_prefer",
                            reorder(cfg.version_prefer, from, to),
                        )
                    }
                />
                <ToggleRow
                    label={t("vp_playlist")}
                    desc={t("vp_playlist_desc")}
                    checked={cfg.version_prefer_for_playlist}
                    onChange={(v) => update("dev", "version_prefer_for_playlist", v)}
                />
            </div>
            <div className="settings-group-title">{t("vp_subtitle")}</div>
            <div className="settings-group">
                <TagListRow
                    label={t("vp_sub_priority")}
                    desc={t("vp_sub_priority_desc")}
                    tags={cfg.subtitle_priority}
                    placeholder={t("vp_sub_priority_placeholder")}
                    onAdd={(tag) =>
                        update("dev", "subtitle_priority", [
                            ...cfg.subtitle_priority,
                            tag,
                        ])
                    }
                    onRemove={(i) =>
                        update(
                            "dev",
                            "subtitle_priority",
                            cfg.subtitle_priority.filter((_, j) => j !== i),
                        )
                    }
                    onReorder={(from, to) =>
                        update(
                            "dev",
                            "subtitle_priority",
                            reorder(cfg.subtitle_priority, from, to),
                        )
                    }
                />
                <TagListRow
                    label={t("vp_sub_extract")}
                    desc={t("vp_sub_extract_desc")}
                    tags={cfg.sub_extract_priority}
                    placeholder={t("vp_sub_extract_placeholder")}
                    onAdd={(tag) =>
                        update("dev", "sub_extract_priority", [
                            ...cfg.sub_extract_priority,
                            tag,
                        ])
                    }
                    onRemove={(i) =>
                        update(
                            "dev",
                            "sub_extract_priority",
                            cfg.sub_extract_priority.filter((_, j) => j !== i),
                        )
                    }
                    onReorder={(from, to) =>
                        update(
                            "dev",
                            "sub_extract_priority",
                            reorder(cfg.sub_extract_priority, from, to),
                        )
                    }
                />
            </div>
            <div className="settings-group-title">{t("vp_limits")}</div>
            <div className="settings-group">
                <NumberRow
                    label={t("vp_max_eps")}
                    desc={t("vp_max_eps_desc")}
                    value={cfg.item_limit}
                    min={0}
                    max={100}
                    onCommit={(v) => update("playlist", "item_limit", v)}
                />
                <ToggleRow
                    label={t("vp_last_ep")}
                    desc={t("vp_last_ep_desc")}
                    checked={cfg.last_ep_disable_playlist}
                    onChange={(v) => update("dev", "last_ep_disable_playlist", v)}
                />
                <TextareaRow
                    label={t("vp_filter")}
                    desc={t("vp_filter_desc")}
                    value={cfg.version_filter}
                    placeholder={t("vp_filter_placeholder")}
                    onCommit={(v) => update("playlist", "version_filter", v)}
                />
            </div>
        </>
    );
}

// ── Network ────────────────────────────────────────────────────────────────────

function NetworkSection({
    cfg,
    update,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
}) {
    const t = useI18n();
    return (
        <>
            <div className="page-title">{t("page_network")}</div>
            <div className="settings-group">
                <InputRow
                    label={t("net_proxy")}
                    desc={t("net_proxy_desc")}
                    value={cfg.http_proxy}
                    placeholder="127.0.0.1:7890"
                    mono
                    onCommit={(v) => update("dev", "http_proxy", v || null)}
                />
                <ToggleRow
                    label={t("net_skip_tls")}
                    desc={t("net_skip_tls_desc")}
                    checked={cfg.skip_certificate_verify}
                    onChange={(v) => update("dev", "skip_certificate_verify", v)}
                />
            </div>
            <div className="settings-group-title">{t("net_redirect")}</div>
            <div className="settings-group">
                <TagListRow
                    label={t("net_redirect_hosts")}
                    desc={t("net_redirect_hosts_desc")}
                    tags={cfg.redirect_check_host}
                    placeholder="cdn.example.com"
                    onAdd={(h) =>
                        update("dev", "redirect_check_host", [
                            ...cfg.redirect_check_host,
                            h,
                        ])
                    }
                    onRemove={(i) =>
                        update(
                            "dev",
                            "redirect_check_host",
                            cfg.redirect_check_host.filter((_, j) => j !== i),
                        )
                    }
                />
            </div>
        </>
    );
}

// ── System ─────────────────────────────────────────────────────────────────────

function SystemSection({
    cfg,
    update,
    autostart,
    onAutostart,
    display,
    onDisplayChange,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
    autostart: boolean;
    onAutostart: (v: boolean) => void;
    display: DisplaySettings;
    onDisplayChange: (patch: Partial<DisplaySettings>) => void;
}) {
    const t = useI18n();

    const dpr =
        typeof window !== "undefined" ? window.devicePixelRatio.toFixed(1) : "1.0";

    const LANG_OPTIONS = [
        { value: "system", label: t("sys_lang_system") },
        { value: "zh-CN", label: "简体中文" },
        { value: "zh-TW", label: "繁體中文" },
        { value: "en", label: "English" },
    ];

    const FONT_SIZE_OPTIONS = [
        { value: "12", label: t("font_12") },
        { value: "13", label: t("font_13") },
        { value: "14", label: t("font_14") },
        { value: "15", label: t("font_15") },
        { value: "16", label: t("font_16") },
    ];

    const ZOOM_OPTIONS = [
        { value: "0.8", label: "80%" },
        { value: "0.9", label: "90%" },
        { value: "1", label: "100%" },
        { value: "1.1", label: "110%" },
        { value: "1.25", label: "125%" },
        { value: "1.5", label: "150%" },
        { value: "1.75", label: "175%" },
        { value: "2", label: "200%" },
    ];

    const LOG_LEVELS = [
        { value: "error", label: t("log_error") },
        { value: "warn", label: t("log_warn") },
        { value: "info", label: t("log_info") },
        { value: "debug", label: t("log_debug") },
        { value: "trace", label: t("log_trace") },
    ];

    return (
        <>
            <div className="page-title">{t("page_system")}</div>

            {/* Appearance */}
            <div className="settings-group-title">{t("sys_appearance")}</div>
            <div className="settings-group">
                <SelectRow
                    label={t("sys_lang")}
                    desc={t("sys_lang_desc")}
                    value={display.lang}
                    options={LANG_OPTIONS}
                    onChange={(v) => onDisplayChange({ lang: v as LangMode })}
                />
                <AccentColorRow
                    value={display.accentColor ?? "blue"}
                    onChange={(v) => onDisplayChange({ accentColor: v })}
                />
            </div>

            {/* Display */}
            <div className="settings-group-title">{t("sys_display")}</div>
            <div className="settings-group">
                <SelectRow
                    label={t("sys_font_size")}
                    desc={t("sys_font_size_desc")}
                    value={String(display.fontSize)}
                    options={FONT_SIZE_OPTIONS}
                    onChange={(v) => onDisplayChange({ fontSize: parseInt(v, 10) })}
                />
                <SelectRow
                    label={t("sys_zoom")}
                    desc={t("sys_zoom_desc", { dpr })}
                    value={String(display.zoom)}
                    options={ZOOM_OPTIONS}
                    onChange={(v) => onDisplayChange({ zoom: parseFloat(v) })}
                />
                <FontPickerRow
                    value={display.fontFamily}
                    onChange={(v) => onDisplayChange({ fontFamily: v })}
                />
            </div>

            {/* Startup */}
            <div className="settings-group-title">{t("sys_startup")}</div>
            <div className="settings-group">
                <ToggleRow
                    label={t("sys_autostart")}
                    desc={t("sys_autostart_desc")}
                    checked={autostart}
                    onChange={onAutostart}
                />
            </div>

            {/* Logs */}
            <div className="settings-group-title">{t("sys_logs_title")}</div>
            <div className="settings-group">
                <SelectRow
                    label={t("sys_log_level")}
                    desc={t("sys_log_level_desc")}
                    value={cfg.log_level}
                    options={LOG_LEVELS}
                    onChange={(v) => update("dev", "log_level", v)}
                />
                <ToggleRow
                    label={t("sys_log_mask")}
                    desc={t("sys_log_mask_desc")}
                    checked={cfg.mix_log}
                    onChange={(v) => update("dev", "mix_log", v)}
                />
            </div>

            {/* Privacy */}
            <div className="settings-group-title">{t("sys_privacy")}</div>
            <div className="settings-group">
                <ToggleRow
                    label={t("sys_no_progress")}
                    desc={t("sys_no_progress_desc")}
                    checked={cfg.disable_progress_report}
                    onChange={(v) => update("dev", "disable_progress_report", v)}
                />
            </div>

            {/* Download */}
            <div className="settings-group-title">{t("sys_download")}</div>
            <div className="settings-group">
                <NumberRow
                    label={t("sys_speed_limit")}
                    desc={t("sys_speed_limit_desc")}
                    value={cfg.speed_limit_mb}
                    min={0}
                    onCommit={(v) => update("gui", "speed_limit_mb", v)}
                />
            </div>

            {/* Trakt */}
            <div className="settings-group-title">{t("sys_trakt")}</div>
            <div className="settings-group">
                <InputRow
                    label={t("sys_trakt_id")}
                    desc={t("sys_trakt_id_desc")}
                    value={cfg.trakt_client_id}
                    placeholder={t("sys_trakt_id_placeholder")}
                    mono
                    onCommit={(v) => update("trakt", "client_id", v)}
                />
                <InputRow
                    label={t("sys_trakt_secret")}
                    desc={t("sys_trakt_secret_desc")}
                    value={cfg.trakt_client_secret}
                    placeholder={t("sys_trakt_secret_placeholder")}
                    mono
                    onCommit={(v) => update("trakt", "client_secret", v)}
                />
                <InputRow
                    label={t("sys_trakt_host")}
                    desc={t("sys_trakt_host_desc")}
                    value={cfg.trakt_enable_host}
                    placeholder={t("sys_trakt_host_placeholder")}
                    mono
                    onCommit={(v) => update("trakt", "enable_host", v)}
                />
            </div>

            {/* Bangumi */}
            <div className="settings-group-title">{t("sys_bangumi")}</div>
            <div className="settings-group">
                <InputRow
                    label={t("sys_bangumi_token")}
                    desc={t("sys_bangumi_token_desc")}
                    value={cfg.bangumi_access_token}
                    placeholder={t("sys_bangumi_token_placeholder")}
                    mono
                    onCommit={(v) => update("bangumi", "access_token", v || null)}
                />
            </div>
        </>
    );
}
