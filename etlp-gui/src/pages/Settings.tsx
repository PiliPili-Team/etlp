import { useState, useEffect, useLayoutEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
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
    silent_start: boolean;
    check_update: boolean;
    disable_progress_report: boolean;
    trakt_client_id: string;
    trakt_client_secret: string;
    trakt_user_name: string;
    trakt_enable_host: string;
    trakt_allow_duplicate: boolean;
    bangumi_access_token: string;
    bangumi_enable_host: string;
    bangumi_username: string;
    bangumi_private: boolean;
    bangumi_genres: string;
    config_path: string;
}

type SectionTab =
    | "player"
    | "version-prefer"
    | "network"
    | "config"
    | "system"
    | "bangumi"
    | "trakt";

interface Props {
    section: SectionTab;
    addToast: (msg: string, err?: boolean) => void;
    display: DisplaySettings;
    onDisplayChange: (patch: Partial<DisplaySettings>) => void;
    onAbout: () => void;
}

// ── Delta patch ────────────────────────────────────────────────────────────────

async function patch(section: string, key: string, value: unknown): Promise<void> {
    await invoke("update_config_field", { section, key, value });
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/**
 * Map a backend error to a localized message.
 *
 * Backend commands return English strings or sentinels; known sentinels are
 * translated, anything else falls back to the raw message (kept verbose for
 * diagnosing unexpected failures).
 */
function mapBackendError(e: unknown, t: ReturnType<typeof useI18n>): string {
    const msg = String(e);
    if (msg.includes("NOT_CONFIGURED")) return t("sync_not_configured");
    if (msg.includes("SERVICE_RUNNING")) return t("cache_stop_first");
    return msg;
}

/** Split a comma-separated host keyword string into trimmed, non-empty tags. */
function parseHostList(raw: string): string[] {
    return raw
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
}

/** Format a byte count as a human-readable string (B / KB / MB / GB). */
function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let value = bytes / 1024;
    let i = 0;
    while (value >= 1024 && i < units.length - 1) {
        value /= 1024;
        i += 1;
    }
    return `${value.toFixed(value < 10 ? 2 : 1)} ${units[i]}`;
}

// ── Confirm modal (theme-aware) ──────────────────────────────────────────────────

function ConfirmModal({
    title,
    message,
    confirmLabel,
    cancelLabel,
    danger,
    onConfirm,
    onCancel,
}: {
    title: string;
    message: string;
    confirmLabel: string;
    cancelLabel: string;
    danger?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
}) {
    return (
        <div className="modal-overlay" onClick={onCancel}>
            <div className="modal-card confirm-card" onClick={(e) => e.stopPropagation()}>
                <div className="confirm-title">{title}</div>
                <div className="confirm-message">{message}</div>
                <div className="confirm-actions">
                    <button className="btn" onClick={onCancel}>
                        {cancelLabel}
                    </button>
                    <button
                        className={`btn ${danger ? "btn-danger" : "btn-primary"}`}
                        onClick={onConfirm}
                    >
                        {confirmLabel}
                    </button>
                </div>
            </div>
        </div>
    );
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

function ButtonRow({
    label,
    desc,
    button,
    danger,
    onClick,
}: {
    label: string;
    desc?: string;
    button: string;
    danger?: boolean;
    onClick: () => void;
}) {
    return (
        <div className="row">
            <div className="row-label">
                <div>{label}</div>
                {desc && <div className="row-desc">{desc}</div>}
            </div>
            <div className="row-control">
                <button
                    className={`btn ${danger ? "btn-danger" : "btn-primary"}`}
                    onClick={onClick}
                >
                    {button}
                </button>
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

export default function Settings({
    section,
    addToast,
    display,
    onDisplayChange,
    onAbout,
}: Props) {
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
            addToast(mapBackendError(e, t), true);
        }
    }, [addToast, t]);

    useEffect(() => {
        const init = setTimeout(loadConfig, 0);
        return () => clearTimeout(init);
    }, [loadConfig]);

    const update = useCallback(
        async (sec: string, key: string, value: unknown) => {
            try {
                await patch(sec, key, value);
                // The ConfigDto flattens some sections with a `${section}_`
                // prefix (trakt_, bangumi_, gui_) and others without one. Set
                // both shapes so optimistic UI (e.g. toggles bound directly to
                // cfg) reflects immediately regardless of the field's naming.
                const flat = key.replace(/\./g, "_");
                setCfg((prev) =>
                    prev
                        ? ({
                              ...prev,
                              [flat]: value,
                              [`${sec}_${flat}`]: value,
                          } as ConfigDto)
                        : prev,
                );
            } catch (e) {
                addToast(mapBackendError(e, t), true);
            }
        },
        [addToast, t],
    );

    const handleAutostart = useCallback(
        async (enabled: boolean) => {
            try {
                await invoke("set_autostart", { enabled });
                setAutostart(enabled);
                addToast(enabled ? t("autostart_on") : t("autostart_off"));
            } catch (e) {
                addToast(mapBackendError(e, t), true);
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
    if (section === "config") return <ConfigSection addToast={addToast} />;
    if (section === "bangumi")
        return <BangumiSection cfg={cfg} update={update} addToast={addToast} />;
    if (section === "trakt")
        return <TraktSection cfg={cfg} update={update} addToast={addToast} />;
    if (section === "system")
        return (
            <SystemSection
                cfg={cfg}
                update={update}
                autostart={autostart}
                onAutostart={handleAutostart}
                display={display}
                onDisplayChange={onDisplayChange}
                addToast={addToast}
                onAbout={onAbout}
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

// ── Config (config file actions + backup / restore / reset / update) ──────────────

interface BackupEntry {
    name: string;
    path: string;
    size: number;
    created_ms: number;
}

interface UpdateInfo {
    current: string;
    latest: string;
    has_update: boolean;
    url: string;
}

function ConfigSection({ addToast }: { addToast: (msg: string, err?: boolean) => void }) {
    const t = useI18n();
    const [busy, setBusy] = useState(false);
    const [backups, setBackups] = useState<BackupEntry[]>([]);
    const [expanded, setExpanded] = useState(false);
    // Pending destructive actions awaiting confirmation.
    const [restoreTarget, setRestoreTarget] = useState<BackupEntry | null>(null);
    const [importPath, setImportPath] = useState<string | null>(null);
    const [deleteTarget, setDeleteTarget] = useState<BackupEntry | null>(null);
    const [confirmReset, setConfirmReset] = useState(false);

    const refreshBackups = useCallback(async () => {
        try {
            const list = await invoke<BackupEntry[]>("list_config_backups");
            setBackups(list);
        } catch {
            setBackups([]);
        }
    }, []);

    useEffect(() => {
        const id = setTimeout(() => void refreshBackups(), 0);
        return () => clearTimeout(id);
    }, [refreshBackups]);

    const fmtTime = (ms: number) => (ms > 0 ? new Date(ms).toLocaleString() : "—");

    const handleOpenFolder = async () => {
        try {
            await invoke("open_config_folder");
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    const handleEditConfig = async () => {
        try {
            await invoke("edit_config");
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    const handleRestart = async () => {
        setBusy(true);
        try {
            const port = await invoke<number>("restart_server");
            addToast(t("toast_restarted", { port }));
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        } finally {
            setBusy(false);
        }
    };

    const handleBackup = async () => {
        setBusy(true);
        try {
            await invoke<BackupEntry>("backup_config");
            addToast(t("cfg_backup_done"));
            setExpanded(true);
            await refreshBackups();
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        } finally {
            setBusy(false);
        }
    };

    const handleImport = async () => {
        try {
            const selected = await openDialog({
                multiple: false,
                directory: false,
                filters: [{ name: "Backup", extensions: ["zip"] }],
            });
            if (typeof selected === "string") {
                setImportPath(selected);
            }
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    const doRestore = async (path: string) => {
        setBusy(true);
        try {
            await invoke("restore_config", { path });
            addToast(t("cfg_restore_done"));
            await refreshBackups();
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        } finally {
            setBusy(false);
        }
    };

    const doDelete = async (path: string) => {
        try {
            await invoke("delete_config_backup", { path });
            await refreshBackups();
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    const doReveal = async (path: string) => {
        try {
            await invoke("reveal_config_backup", { path });
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    const doReset = async () => {
        setConfirmReset(false);
        setBusy(true);
        try {
            await invoke("reset_config");
            addToast(t("cfg_reset_done"));
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        } finally {
            setBusy(false);
        }
    };

    return (
        <>
            <div className="page-title">{t("page_config")}</div>

            {/* Config file */}
            <div className="settings-group-title" style={{ marginTop: 0 }}>
                {t("cfg_file_title")}
            </div>
            <div className="settings-group">
                <div className="row">
                    <div className="row-label">
                        <div>{t("ov_config_file")}</div>
                        <div className="row-desc">{t("ov_config_file_desc")}</div>
                    </div>
                    <div className="row-control">
                        <button className="btn" onClick={() => void handleOpenFolder()}>
                            {t("open_dir")}
                        </button>
                        <button className="btn" onClick={() => void handleEditConfig()}>
                            {t("ov_edit_config")}
                        </button>
                    </div>
                </div>
                <ButtonRow
                    label={t("ov_restart")}
                    desc={t("ov_restart_desc")}
                    button={t("ov_restart")}
                    onClick={() => void handleRestart()}
                />
            </div>

            {/* Backup & restore */}
            <div className="settings-group-title">{t("cfg_backup_title")}</div>
            <div className="settings-group">
                <ButtonRow
                    label={t("cfg_backup_now")}
                    desc={t("cfg_backup_now_desc")}
                    button={t("cfg_backup_now")}
                    onClick={() => void handleBackup()}
                />
                <div
                    className="row"
                    style={{ flexDirection: "column", alignItems: "stretch" }}
                >
                    <div
                        className="row-label"
                        style={{
                            display: "flex",
                            flexDirection: "row",
                            justifyContent: "space-between",
                            alignItems: "center",
                            cursor: "pointer",
                            width: "100%",
                        }}
                        onClick={() => setExpanded((v) => !v)}
                    >
                        <div>
                            <div>{t("cfg_backup_list")}</div>
                            <div className="row-desc">
                                {t("cfg_backup_list_desc", { count: backups.length })}
                            </div>
                        </div>
                        <span className="cache-size">{expanded ? "▾" : "▸"}</span>
                    </div>
                    {expanded && (
                        <div
                            style={{
                                marginTop: 10,
                                display: "flex",
                                flexDirection: "column",
                                gap: 8,
                            }}
                        >
                            {backups.length === 0 && (
                                <div className="row-desc">{t("cfg_backup_empty")}</div>
                            )}
                            {backups.map((b) => (
                                <div
                                    key={b.path}
                                    style={{
                                        display: "flex",
                                        justifyContent: "space-between",
                                        alignItems: "center",
                                        gap: 12,
                                        padding: "8px 12px",
                                        background: "var(--bg)",
                                        borderRadius: 8,
                                    }}
                                >
                                    <div style={{ minWidth: 0 }}>
                                        <div
                                            className="code"
                                            style={{
                                                fontSize: 12,
                                                overflow: "hidden",
                                                textOverflow: "ellipsis",
                                                whiteSpace: "nowrap",
                                            }}
                                        >
                                            {b.name}
                                        </div>
                                        <div className="row-desc">
                                            {fmtTime(b.created_ms)} ·{" "}
                                            {formatBytes(b.size)}
                                        </div>
                                    </div>
                                    <div
                                        className="row-control"
                                        style={{ flexShrink: 0 }}
                                    >
                                        <button
                                            className="btn"
                                            onClick={() => void doReveal(b.path)}
                                        >
                                            {t("cfg_view")}
                                        </button>
                                        <button
                                            className="btn"
                                            disabled={busy}
                                            onClick={() => setRestoreTarget(b)}
                                        >
                                            {t("cfg_restore")}
                                        </button>
                                        <button
                                            className="btn btn-danger"
                                            onClick={() => setDeleteTarget(b)}
                                        >
                                            {t("cfg_delete")}
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
                <ButtonRow
                    label={t("cfg_import")}
                    desc={t("cfg_import_desc")}
                    button={t("cfg_import")}
                    onClick={() => void handleImport()}
                />
            </div>

            {/* Reset */}
            <div className="settings-group-title">{t("cfg_reset_title")}</div>
            <div className="settings-group">
                <ButtonRow
                    label={t("cfg_reset")}
                    desc={t("cfg_reset_desc")}
                    button={t("cfg_reset")}
                    danger
                    onClick={() => setConfirmReset(true)}
                />
            </div>

            {restoreTarget && (
                <ConfirmModal
                    title={t("cfg_restore_confirm_title")}
                    message={t("cfg_restore_confirm_message", {
                        name: restoreTarget.name,
                    })}
                    confirmLabel={t("cfg_restore")}
                    cancelLabel={t("cache_confirm_cancel")}
                    onConfirm={() => {
                        const path = restoreTarget.path;
                        setRestoreTarget(null);
                        void doRestore(path);
                    }}
                    onCancel={() => setRestoreTarget(null)}
                />
            )}

            {importPath && (
                <ConfirmModal
                    title={t("cfg_import_confirm_title")}
                    message={t("cfg_import_confirm_message")}
                    confirmLabel={t("cfg_restore")}
                    cancelLabel={t("cache_confirm_cancel")}
                    onConfirm={() => {
                        const path = importPath;
                        setImportPath(null);
                        void doRestore(path);
                    }}
                    onCancel={() => setImportPath(null)}
                />
            )}

            {deleteTarget && (
                <ConfirmModal
                    title={t("cfg_delete_confirm_title")}
                    message={t("cfg_delete_confirm_message", {
                        name: deleteTarget.name,
                    })}
                    confirmLabel={t("cfg_delete")}
                    cancelLabel={t("cache_confirm_cancel")}
                    danger
                    onConfirm={() => {
                        const path = deleteTarget.path;
                        setDeleteTarget(null);
                        void doDelete(path);
                    }}
                    onCancel={() => setDeleteTarget(null)}
                />
            )}

            {confirmReset && (
                <ConfirmModal
                    title={t("cfg_reset_confirm_title")}
                    message={t("cfg_reset_confirm_message")}
                    confirmLabel={t("cfg_reset")}
                    cancelLabel={t("cache_confirm_cancel")}
                    danger
                    onConfirm={() => void doReset()}
                    onCancel={() => setConfirmReset(false)}
                />
            )}
        </>
    );
}

function SystemSection({
    cfg,
    update,
    autostart,
    onAutostart,
    display,
    onDisplayChange,
    addToast,
    onAbout,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
    autostart: boolean;
    onAutostart: (v: boolean) => void;
    display: DisplaySettings;
    onDisplayChange: (patch: Partial<DisplaySettings>) => void;
    addToast: (msg: string, err?: boolean) => void;
    onAbout: () => void;
}) {
    const t = useI18n();

    const dpr =
        typeof window !== "undefined" ? window.devicePixelRatio.toFixed(1) : "1.0";

    // ── Cache ────────────────────────────────────────────────────────────────
    const [cacheSize, setCacheSize] = useState<number | null>(null);
    const [confirmClear, setConfirmClear] = useState(false);

    const refreshCacheSize = useCallback(async () => {
        try {
            const size = await invoke<number>("get_cache_size");
            setCacheSize(size);
        } catch {
            setCacheSize(null);
        }
    }, []);

    useEffect(() => {
        // Defer to a macrotask so the setState in refreshCacheSize does not run
        // synchronously inside the effect body (avoids cascading renders).
        const id = setTimeout(() => void refreshCacheSize(), 0);
        return () => clearTimeout(id);
    }, [refreshCacheSize]);

    const doClearCache = async () => {
        setConfirmClear(false);
        try {
            // Guard: clearing is only safe while the service is stopped.
            const status = await invoke<{ running: boolean }>("get_server_status");
            if (status.running) {
                addToast(t("cache_stop_first"), true);
                return;
            }
            const freed = await invoke<number>("clear_cache");
            addToast(t("cache_cleared").replace("{size}", formatBytes(freed)));
            await refreshCacheSize();
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    // ── Update ─────────────────────────────────────────────────────────────────
    const [checking, setChecking] = useState(false);

    const doCheckUpdate = async () => {
        setChecking(true);
        try {
            const info = await invoke<UpdateInfo>("check_update");
            if (info.has_update) {
                addToast(t("cfg_update_available", { version: info.latest }));
                await openUrl(info.url);
            } else {
                addToast(t("cfg_update_latest", { version: info.current }));
            }
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        } finally {
            setChecking(false);
        }
    };

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
                <ToggleRow
                    label={t("sys_center_nav")}
                    desc={t("sys_center_nav_desc")}
                    checked={display.centerNav ?? false}
                    onChange={(v) => onDisplayChange({ centerNav: v })}
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
                <ToggleRow
                    label={t("sys_silent_start")}
                    desc={t("sys_silent_start_desc")}
                    checked={cfg.silent_start}
                    onChange={(v) => update("gui", "silent_start", v)}
                />
            </div>

            {/* Update */}
            <div className="settings-group-title">{t("cfg_update_title")}</div>
            <div className="settings-group">
                <ToggleRow
                    label={t("cfg_update_auto")}
                    desc={t("cfg_update_auto_desc")}
                    checked={cfg.check_update}
                    onChange={(v) => update("gui", "check_update", v)}
                />
                <ButtonRow
                    label={t("cfg_update_check")}
                    desc={t("cfg_update_check_desc")}
                    button={checking ? t("cfg_update_checking") : t("cfg_update_check")}
                    onClick={() => void doCheckUpdate()}
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

            {/* Cache */}
            <div className="settings-group-title">{t("sys_cache")}</div>
            <div className="settings-group">
                <div className="row">
                    <div className="row-label">
                        <div>{t("sys_cache_size")}</div>
                        <div className="row-desc">{t("sys_cache_size_desc")}</div>
                    </div>
                    <div className="row-control">
                        <span className="cache-size">
                            {cacheSize === null ? "—" : formatBytes(cacheSize)}
                        </span>
                    </div>
                </div>
                <ButtonRow
                    label={t("sys_cache_clear")}
                    desc={t("sys_cache_clear_desc")}
                    button={t("sys_cache_clear")}
                    danger
                    onClick={() => setConfirmClear(true)}
                />
            </div>

            {/* General — kept last in the System tab. */}
            <div className="settings-group-title">{t("sys_general")}</div>
            <div className="settings-group">
                <ButtonRow
                    label={t("sys_about")}
                    desc={t("sys_about_desc")}
                    button={t("ov_view")}
                    onClick={onAbout}
                />
            </div>

            {confirmClear && (
                <ConfirmModal
                    title={t("cache_confirm_title")}
                    message={t("cache_confirm_message")}
                    confirmLabel={t("cache_confirm_ok")}
                    cancelLabel={t("cache_confirm_cancel")}
                    danger
                    onConfirm={() => void doClearCache()}
                    onCancel={() => setConfirmClear(false)}
                />
            )}
        </>
    );
}

// ── Bangumi ────────────────────────────────────────────────────────────────────

// Refresh (↻) glyph for the sync tab headers; spins via the `.spin` class.
function IconRefresh({ spinning }: { spinning?: boolean }) {
    // Single circular-arrow glyph (centered in its 1024 viewBox); the `.spin`
    // class rotates it about its own center.
    return (
        <svg
            className={spinning ? "spin" : ""}
            viewBox="0 0 1024 1024"
            fill="currentColor"
            width="16"
            height="16"
        >
            <path d="M512 176a336 336 0 1 0 237.568 573.568 48 48 0 0 1 67.904 67.904 432 432 0 1 1 0-610.944c8.512 8.512 19.2 19.712 30.528 31.872V170.688a48 48 0 0 1 96 0v192a48 48 0 0 1-48 48h-192a48 48 0 0 1 0-96h83.84a1558.016 1558.016 0 0 0-38.272-40.32A334.784 334.784 0 0 0 512 176z" />
        </svg>
    );
}

/// Shared header + auth actions for the Bangumi / Trakt sync tabs.
///
/// `refreshCmd` / `testCmd` are the backend command names; the title carries a
/// refresh button that (a) demands a running service, (b) confirms, then
/// (c) spins while the refresh resolves. A separate "test" action probes
/// whether authorization currently works.
function useSyncAuth(
    refreshCmd: string,
    testCmd: string,
    addToast: (msg: string, err?: boolean) => void,
    t: ReturnType<typeof useI18n>,
    isComplete: boolean,
) {
    const [busy, setBusy] = useState(false);
    const [testing, setTesting] = useState(false);
    const [confirmRefresh, setConfirmRefresh] = useState(false);

    const onRefreshClick = async () => {
        try {
            const status = await invoke<{ running: boolean }>("get_server_status");
            if (!status.running) {
                addToast(t("sync_start_service_first"), true);
                return;
            }
            setConfirmRefresh(true);
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        }
    };

    const doRefresh = async () => {
        setConfirmRefresh(false);
        setBusy(true);
        try {
            const result = await invoke<string>(refreshCmd);
            addToast(
                result === "AUTH_OPENED"
                    ? t("sync_authorize_opened")
                    : t("sync_auth_valid"),
            );
        } catch (e) {
            addToast(mapBackendError(e, t), true);
        } finally {
            setBusy(false);
        }
    };

    const doTest = async () => {
        // Incomplete config: point the user at the missing fields before any
        // network round-trip.
        if (!isComplete) {
            addToast(t("sync_incomplete"), true);
            return;
        }
        setTesting(true);
        try {
            const ok = await invoke<boolean>(testCmd);
            // A complete config that still fails is either wrong or unauthorized;
            // steer the user to the refresh-authorization button.
            addToast(ok ? t("sync_test_ok") : t("sync_test_fail"), !ok);
        } catch (e) {
            const msg = String(e);
            if (msg.includes("NOT_CONFIGURED")) {
                addToast(t("sync_incomplete"), true);
            } else {
                addToast(mapBackendError(e, t), true);
            }
        } finally {
            setTesting(false);
        }
    };

    return {
        busy,
        testing,
        confirmRefresh,
        setConfirmRefresh,
        onRefreshClick,
        doRefresh,
        doTest,
    };
}

function SyncTabHeader({
    title,
    busy,
    onRefresh,
}: {
    title: string;
    busy: boolean;
    onRefresh: () => void;
}) {
    const t = useI18n();
    return (
        <div className="page-title-row">
            <div className="page-title">{title}</div>
            <button
                className="btn btn-with-icon"
                title={t("sync_refresh")}
                onClick={onRefresh}
                disabled={busy}
            >
                <IconRefresh spinning={busy} />
                <span>{busy ? t("sync_refreshing") : t("sync_refresh")}</span>
            </button>
        </div>
    );
}

function BangumiSection({
    cfg,
    update,
    addToast,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
    addToast: (msg: string, err?: boolean) => void;
}) {
    const t = useI18n();
    const complete =
        cfg.bangumi_access_token.trim() !== "" &&
        cfg.bangumi_username.trim() !== "" &&
        cfg.bangumi_enable_host.trim() !== "";
    const auth = useSyncAuth(
        "refresh_bangumi_auth",
        "test_bangumi_auth",
        addToast,
        t,
        complete,
    );

    return (
        <>
            <SyncTabHeader
                title={t("sys_bangumi")}
                busy={auth.busy}
                onRefresh={() => void auth.onRefreshClick()}
            />

            <div className="settings-group">
                <TagListRow
                    label={t("sys_bangumi_host")}
                    desc={t("sys_bangumi_host_desc")}
                    tags={parseHostList(cfg.bangumi_enable_host)}
                    placeholder={t("sys_bangumi_host_placeholder")}
                    onAdd={(tag) =>
                        update(
                            "bangumi",
                            "enable_host",
                            [...parseHostList(cfg.bangumi_enable_host), tag].join(", "),
                        )
                    }
                    onRemove={(i) =>
                        update(
                            "bangumi",
                            "enable_host",
                            parseHostList(cfg.bangumi_enable_host)
                                .filter((_, j) => j !== i)
                                .join(", "),
                        )
                    }
                />
                <InputRow
                    label={t("sys_bangumi_user")}
                    desc={t("sys_bangumi_user_desc")}
                    value={cfg.bangumi_username}
                    placeholder={t("sys_bangumi_user_placeholder")}
                    mono
                    onCommit={(v) => update("bangumi", "username", v)}
                />
                <InputRow
                    label={t("sys_bangumi_token")}
                    desc={t("sys_bangumi_token_desc")}
                    value={cfg.bangumi_access_token}
                    placeholder={t("sys_bangumi_token_placeholder")}
                    mono
                    onCommit={(v) => update("bangumi", "access_token", v)}
                />
                <ToggleRow
                    label={t("sys_bangumi_private")}
                    desc={t("sys_bangumi_private_desc")}
                    checked={cfg.bangumi_private}
                    onChange={(v) => update("bangumi", "private", v)}
                />
                <InputRow
                    label={t("sys_bangumi_genres")}
                    desc={t("sys_bangumi_genres_desc")}
                    value={cfg.bangumi_genres}
                    placeholder={t("sys_bangumi_genres_placeholder")}
                    mono
                    onCommit={(v) => update("bangumi", "genres", v)}
                />
                <ButtonRow
                    label={t("sync_test")}
                    desc={t("sync_test_desc")}
                    button={auth.testing ? t("sync_testing") : t("sync_test")}
                    onClick={() => void auth.doTest()}
                />
            </div>

            {auth.confirmRefresh && (
                <ConfirmModal
                    title={t("sync_refresh_confirm_title")}
                    message={t("sync_refresh_confirm_message")}
                    confirmLabel={t("sync_refresh_confirm_ok")}
                    cancelLabel={t("cache_confirm_cancel")}
                    onConfirm={() => void auth.doRefresh()}
                    onCancel={() => auth.setConfirmRefresh(false)}
                />
            )}
        </>
    );
}

// ── Trakt ──────────────────────────────────────────────────────────────────────

/** Redirect URI the user must register on their Trakt application. Must match
 *  the `/trakt_auth` callback the local server serves on the default port. */
const TRAKT_REDIRECT_URI = "http://localhost:58000/trakt_auth";
const TRAKT_APPS_URL = "https://trakt.tv/oauth/applications";

/** Inline setup instructions for creating a Trakt application: a link to the
 *  Trakt apps page and the redirect URI to register, with a copy button. */
function TraktSetupGuide({
    addToast,
}: {
    addToast: (msg: string, err?: boolean) => void;
}) {
    const t = useI18n();
    const copyRedirect = async () => {
        try {
            await navigator.clipboard.writeText(TRAKT_REDIRECT_URI);
            addToast(t("sys_trakt_setup_copied"));
        } catch {
            addToast(t("sys_trakt_setup_copy_failed"), true);
        }
    };
    return (
        <div className="settings-note">
            <ol>
                <li>
                    {t("sys_trakt_setup_step1")}
                    <a
                        href={TRAKT_APPS_URL}
                        onClick={(e) => {
                            e.preventDefault();
                            void openUrl(TRAKT_APPS_URL);
                        }}
                    >
                        {t("sys_trakt_setup_link")}
                    </a>
                </li>
                <li>{t("sys_trakt_setup_step2")}</li>
            </ol>
            <div className="settings-note-copy">
                <code>{TRAKT_REDIRECT_URI}</code>
                <button className="btn" onClick={() => void copyRedirect()}>
                    {t("sys_trakt_setup_copy")}
                </button>
            </div>
        </div>
    );
}

function TraktSection({
    cfg,
    update,
    addToast,
}: {
    cfg: ConfigDto;
    update: (s: string, k: string, v: unknown) => void;
    addToast: (msg: string, err?: boolean) => void;
}) {
    const t = useI18n();
    const complete =
        cfg.trakt_client_id.trim() !== "" &&
        cfg.trakt_client_secret.trim() !== "" &&
        cfg.trakt_user_name.trim() !== "" &&
        cfg.trakt_enable_host.trim() !== "";
    const auth = useSyncAuth(
        "refresh_trakt_auth",
        "test_trakt_auth",
        addToast,
        t,
        complete,
    );

    return (
        <>
            <SyncTabHeader
                title={t("sys_trakt")}
                busy={auth.busy}
                onRefresh={() => void auth.onRefreshClick()}
            />

            <div className="settings-group-title" style={{ marginTop: 0 }}>
                {t("sys_trakt_setup_title")}
            </div>
            <div className="settings-group">
                <TraktSetupGuide addToast={addToast} />
            </div>

            <div className="settings-group">
                <TagListRow
                    label={t("sys_trakt_host")}
                    desc={t("sys_trakt_host_desc")}
                    tags={parseHostList(cfg.trakt_enable_host)}
                    placeholder={t("sys_trakt_host_placeholder")}
                    onAdd={(tag) =>
                        update(
                            "trakt",
                            "enable_host",
                            [...parseHostList(cfg.trakt_enable_host), tag].join(", "),
                        )
                    }
                    onRemove={(i) =>
                        update(
                            "trakt",
                            "enable_host",
                            parseHostList(cfg.trakt_enable_host)
                                .filter((_, j) => j !== i)
                                .join(", "),
                        )
                    }
                />
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
                    label={t("sys_trakt_user")}
                    desc={t("sys_trakt_user_desc")}
                    value={cfg.trakt_user_name}
                    placeholder={t("sys_trakt_user_placeholder")}
                    mono
                    onCommit={(v) => update("trakt", "user_name", v)}
                />
                <ToggleRow
                    label={t("sys_trakt_dup")}
                    desc={t("sys_trakt_dup_desc")}
                    checked={cfg.trakt_allow_duplicate}
                    onChange={(v) => update("trakt", "allow_duplicate", v)}
                />
                <ButtonRow
                    label={t("sync_test")}
                    desc={t("sync_test_desc")}
                    button={auth.testing ? t("sync_testing") : t("sync_test")}
                    onClick={() => void auth.doTest()}
                />
            </div>

            {auth.confirmRefresh && (
                <ConfirmModal
                    title={t("sync_refresh_confirm_title")}
                    message={t("sync_refresh_confirm_message")}
                    confirmLabel={t("sync_refresh_confirm_ok")}
                    cancelLabel={t("cache_confirm_cancel")}
                    onConfirm={() => void auth.doRefresh()}
                    onCancel={() => auth.setConfirmRefresh(false)}
                />
            )}
        </>
    );
}
