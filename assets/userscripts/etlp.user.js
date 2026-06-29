// ==UserScript==
// @name         etlp
// @name:zh-CN   etlp
// @namespace    https://github.com/PiliPili-Team/etlp
// @version      2026.06.29
// @description  Send Emby, Jellyfin, and Plex playback to etlp.
// @author       PiliPili-Team
// @icon         data:image/png;base64,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
// @match        *://*/web/index.html*
// @match        *://*/*/web/index.html*
// @match        *://*/web/
// @match        *://*/*/web/
// @match        https://app.emby.media/*
// @match        https://app.plex.tv/*
// @grant        unsafeWindow
// @grant        GM_info
// @grant        GM_xmlhttpRequest
// @grant        GM_registerMenuCommand
// @grant        GM_unregisterMenuCommand
// @grant        GM_getValue
// @grant        GM_setValue
// @grant        GM_deleteValue
// @run-at       document-start
// @connect      127.0.0.1
// @license      MIT
// ==/UserScript==

/* global ApiClient */

(function () {
    "use strict";

    const IDS = Object.freeze({
        runtime: "__etlp_userscript_runtime__",
        notice: "etlp-notice",
        noticeStyle: "etlp-notice-style",
    });

    const ICON_BASE64 =
        "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";

    const ASSETS = Object.freeze({
        icon: `data:image/png;base64,${ICON_BASE64}`,
    });

    const LOCALE = Object.freeze({
        zhHans: "zh-Hans",
        zhHant: "zh-Hant",
        en: "en",
    });

    const TEXT = Object.freeze({
        "zh-Hans": {
            normalTitle: "原神启动！",
            normalBody: "旅行者，我们去那边看看吧！",
            serviceTitle: "原神启动不能！",
            serviceBody: "旅行者，您需要先开启服务！",
            serviceHelp: "旅行者，我们先把服务打开吧！",
            errorTitle: "原神启动不能！",
            errorBody: "以普遍理性而言，旅行者，配套程序存在某些问题。",
            portTitle: "端口设置",
            portBody: "脚本端口必须和配套程序设置端口一致。",
            invalidPort: "端口无效，请输入 1 到 65535 之间的整数。",
            portReset: "已恢复默认端口 58000，请确认配套程序使用相同端口。",
            portSaved:
                "已设置脚本端口为 {port}，请确认配套程序使用相同端口。",
            seriesHint: "请在需要隐藏的电视剧条目根页面操作。",
            seriesHidden: "已隐藏该电视剧。SeriesId={id}",
            seriesExists: "该电视剧已在隐藏列表中。",
            seriesReset: "已重置隐藏设置，刷新页面后生效。",
            trailer: "etlp 不支持 Trailers 插件，请禁用后再试。",
            menuCurrentPort: "当前端口：{port}（必须和配套程序设置端口一致）",
            menuPortNotice: "当前脚本端口：{port}",
            menuSetPort: "设置 etlp 端口",
            menuResetPort: "重置端口为 58000",
            menuServerScript: "当前服务端脚本",
            menuMountDisk: "挂载磁盘模式",
            menuEnabled: "已启用",
            menuDisabled: "已禁用",
            menuOpenCacheTasks: "打开缓存任务",
            menuHideSeries: "继续观看：隐藏当前电视剧",
            menuResetSeries: "继续观看：重置隐藏电视剧",
            portPrompt:
                "请输入 etlp 端口。\n脚本端口必须和配套程序设置端口一致。",
        },
        "zh-Hant": {
            normalTitle: "原神啟動！",
            normalBody: "旅行者，我們去那邊看看吧！",
            serviceTitle: "原神啟動不能！",
            serviceBody: "旅行者，您需要先開啟服務！",
            serviceHelp: "旅行者，我們先把服務打開吧！",
            errorTitle: "原神啟動不能！",
            errorBody: "以普遍理性而言，旅行者，配套程式存在某些問題。",
            portTitle: "連接埠設定",
            portBody: "腳本連接埠必須和配套程式設定連接埠一致。",
            invalidPort: "連接埠無效，請輸入 1 到 65535 之間的整數。",
            portReset:
                "已恢復預設連接埠 58000，請確認配套程式使用相同連接埠。",
            portSaved:
                "已設定腳本連接埠為 {port}，" +
                "請確認配套程式使用相同連接埠。",
            seriesHint: "請在需要隱藏的電視劇條目根頁面操作。",
            seriesHidden: "已隱藏該電視劇。SeriesId={id}",
            seriesExists: "該電視劇已在隱藏列表中。",
            seriesReset: "已重置隱藏設定，重新整理頁面後生效。",
            trailer: "etlp 不支援 Trailers 外掛，請停用後再試。",
            menuCurrentPort:
                "目前連接埠：{port}（必須和配套程式設定連接埠一致）",
            menuPortNotice: "目前腳本連接埠：{port}",
            menuSetPort: "設定 etlp 連接埠",
            menuResetPort: "重設連接埠為 58000",
            menuServerScript: "目前伺服器腳本",
            menuMountDisk: "掛載磁碟模式",
            menuEnabled: "已啟用",
            menuDisabled: "已停用",
            menuOpenCacheTasks: "開啟快取任務",
            menuHideSeries: "繼續觀看：隱藏目前電視劇",
            menuResetSeries: "繼續觀看：重設隱藏電視劇",
            portPrompt:
                "請輸入 etlp 連接埠。\n" +
                "腳本連接埠必須和配套程式設定連接埠一致。",
        },
        en: {
            normalTitle: "Genshin Impact, start!",
            normalBody: "Traveler, let's go take a look over there!",
            serviceTitle: "Genshin Impact cannot start!",
            serviceBody: "Traveler, please start the service first!",
            serviceHelp: "Traveler, let's open the service first!",
            errorTitle: "Genshin Impact cannot start!",
            errorBody:
                "In common rational terms, Traveler, the companion app has a problem.",
            portTitle: "Port setting",
            portBody:
                "The userscript port must match the companion app port setting.",
            invalidPort: "Invalid port. Enter an integer between 1 and 65535.",
            portReset:
                "Port reset to 58000. Ensure the companion app uses the same port.",
            portSaved:
                "Userscript port set to {port}. Ensure the companion app uses it.",
            seriesHint: "Open the root page of the series you want to hide.",
            seriesHidden: "Series hidden. SeriesId={id}",
            seriesExists: "This series is already hidden.",
            seriesReset: "Hidden series settings reset. Refresh the page to apply.",
            trailer: "etlp does not support the Trailers plugin. Disable it first.",
            menuCurrentPort:
                "Current port: {port} (must match the companion app port)",
            menuPortNotice: "Current userscript port: {port}",
            menuSetPort: "Set etlp port",
            menuResetPort: "Reset port to 58000",
            menuServerScript: "Current server script",
            menuMountDisk: "Mount disk mode",
            menuEnabled: "enabled",
            menuDisabled: "disabled",
            menuOpenCacheTasks: "Open cache tasks",
            menuHideSeries: "Resume: hide this series",
            menuResetSeries: "Resume: reset hidden series",
            portPrompt:
                "Enter the etlp port.\nIt must match the companion app port setting.",
        },
    });

    const ROUTES = Object.freeze({
        embyPrimary: "etlp",
        embyLegacy: "embyToLocalPlayer",
        plex: "plexToLocalPlayer",
        openFolder: "openFolder",
    });

    const STORAGE = Object.freeze({
        webPlayerEnabled: "webPlayerEnable",
        mountDiskEnabled: "mountDiskEnable",
        crackFullPath: "etlpCrackFullPath",
        resumeHideEnabled: "etlpResumeHideSomeSeries",
        resumeCacheIds: "etlpCacheResumeIds",
        hiddenSeriesIds: "etlpResumeHideSeriesIds",
        localPort: "etlpLocalPort",
        logLevel: "etlpLogLevel",
    });

    const SETTINGS = {
        defaultPort: 58000,
        logLevel: 2,
        disableOpenFolder: false,
        crackFullPath: false,
        useWebPlayerForLiveTv: false,
        reorderResume: true,
        hideResumeSeries: false,
        requestTimeoutMs: 3000,
        resumeRecentDays: 3,
        resumeWarmCount: 5,
    };

    const LEVELS = Object.freeze({
        error: 1,
        info: 2,
        debug: 3,
        tracing: 4,
    });

    const LOG_STYLE = Object.freeze({
        error:
            "color:#fff;background:#b42318;font-weight:700;" +
            "padding:2px 6px;border-radius:4px;",
        info:
            "color:#fff;background:#0f766e;font-weight:700;" +
            "padding:2px 6px;border-radius:4px;",
        debug:
            "color:#111827;background:#facc15;font-weight:700;" +
            "padding:2px 6px;border-radius:4px;",
        tracing:
            "color:#fff;background:#475569;font-weight:700;" +
            "padding:2px 6px;border-radius:4px;",
    });

    const hostWindow = typeof unsafeWindow === "undefined" ? window : unsafeWindow;
    const rawFetch = hostWindow.fetch
        ? hostWindow.fetch.bind(hostWindow)
        : window.fetch.bind(window);
    const rawOpen = XMLHttpRequest.prototype.open;
    const rawSend = XMLHttpRequest.prototype.send;
    const rawSetHeader = XMLHttpRequest.prototype.setRequestHeader;

    const runtime = hostWindow[IDS.runtime] || {
        installed: false,
        firstPlay: true,
        menus: [],
        serverName: null,
        playlistCache: null,
        resumeRawCache: null,
        resumeItemCache: {},
        resumePlaybackCache: {},
        allItemCache: {},
        allPlaybackCache: {},
        episodesCache: [],
        episodesWithPathCache: {},
        metadataMayChange: false,
        recentPosts: {},
        playbackErrorSuppressUntil: 0,
        playbackErrorSuppressObserver: null,
    };

    hostWindow[IDS.runtime] = runtime;

    if (runtime.installed) {
        log("info", "bootstrap", "Script is already installed.", {});
        return;
    }

    runtime.installed = true;

    function log(level, domain, message, detail) {
        const gate = LEVELS[level] || LEVELS.info;
        if (SETTINGS.logLevel < gate) {
            return;
        }

        const prefix = `etlp ${level} ${domain}`;
        const output = detail === undefined ? [message] : [message, detail];
        console.log(`%c${prefix}`, LOG_STYLE[level] || LOG_STYLE.info, ...output);
    }

    function readLocal(key, fallback) {
        try {
            const value = localStorage.getItem(key);
            return value === null ? fallback : value;
        } catch (error) {
            log("error", "storage", "Failed to read localStorage.", { key, error });
            return fallback;
        }
    }

    function writeLocal(key, value) {
        try {
            localStorage.setItem(key, value);
            return true;
        } catch (error) {
            log("error", "storage", "Failed to write localStorage.", { key, error });
            return false;
        }
    }

    function deleteLocal(key) {
        try {
            localStorage.removeItem(key);
            return true;
        } catch (error) {
            log("error", "storage", "Failed to delete localStorage.", { key, error });
            return false;
        }
    }

    function readGm(key, fallback) {
        try {
            if (typeof GM_getValue !== "function") {
                return fallback;
            }
            return GM_getValue(key, fallback);
        } catch (error) {
            log("error", "storage", "Failed to read userscript storage.", {
                key,
                error,
            });
            return fallback;
        }
    }

    function writeGm(key, value) {
        try {
            if (typeof GM_setValue === "function") {
                GM_setValue(key, value);
            }
            return true;
        } catch (error) {
            log("error", "storage", "Failed to write userscript storage.", {
                key,
                error,
            });
            return false;
        }
    }

    function deleteGm(key) {
        try {
            if (typeof GM_deleteValue === "function") {
                GM_deleteValue(key);
            }
            return true;
        } catch (error) {
            log("error", "storage", "Failed to delete userscript storage.", {
                key,
                error,
            });
            return false;
        }
    }

    function syncSettings() {
        const rawLevel = readLocal(STORAGE.logLevel, String(SETTINGS.logLevel));
        const parsedLevel = Number.parseInt(rawLevel, 10);
        if (Number.isInteger(parsedLevel) && parsedLevel >= 1 && parsedLevel <= 4) {
            SETTINGS.logLevel = parsedLevel;
        }

        syncBooleanSetting(STORAGE.crackFullPath, "crackFullPath");
        syncBooleanSetting(STORAGE.resumeHideEnabled, "hideResumeSeries");
    }

    function syncBooleanSetting(key, settingName) {
        const localValue = readLocal(key, null);
        if (localValue === "true") {
            writeGm(key, true);
        } else if (localValue === "false") {
            writeGm(key, false);
        }

        const gmValue = readGm(key, null);
        if (gmValue !== null) {
            SETTINGS[settingName] = gmValue === true;
        }
    }

    function currentPort() {
        const gmPort = readGm(STORAGE.localPort, null);
        const localPort = readLocal(STORAGE.localPort, null);
        const candidate = gmPort === null ? localPort : gmPort;
        return parsePort(candidate) || SETTINGS.defaultPort;
    }

    function parsePort(input) {
        const text = String(input || "").trim();
        if (!/^[1-9]\d{0,4}$/.test(text)) {
            return null;
        }

        const port = Number.parseInt(text, 10);
        if (!Number.isInteger(port) || port < 1 || port > 65535) {
            return null;
        }

        return port;
    }

    function setPort(port) {
        const value = String(port);
        writeLocal(STORAGE.localPort, value);
        writeGm(STORAGE.localPort, value);
        registerMenus();
    }

    function resetPort() {
        deleteLocal(STORAGE.localPort);
        deleteGm(STORAGE.localPort);
        registerMenus();
    }

    function buildLocalUrl(route, withTrailingSlash) {
        const suffix = withTrailingSlash ? "/" : "";
        return `http://127.0.0.1:${currentPort()}/${route}${suffix}`;
    }

    function postLocal(data, route) {
        const dedupeKey = localPostKey(data, route);
        if (isDuplicatePost(dedupeKey)) {
            log("debug", "local-http", "Skipped duplicate local request.", {
                route,
            });
            return Promise.resolve(null);
        }

        const usesLegacyShape = route !== ROUTES.embyPrimary;
        return postLocalRoute(data, route, usesLegacyShape).catch((firstError) => {
            if (route !== ROUTES.embyPrimary) {
                throw firstError;
            }

            log("info", "route", "Primary route failed, trying legacy route.", {
                error: firstError,
            });
            return postLocalRoute(data, ROUTES.embyLegacy, true);
        });
    }

    function localPostKey(data, route) {
        const playbackUrl = data && data.playbackUrl ? data.playbackUrl : "";
        const showTaskManager = data && data.showTaskManager ? "task" : "";
        const folder = data && data.full_path ? data.full_path : "";
        return [route, playbackUrl, showTaskManager, folder].join("|");
    }

    function isDuplicatePost(key) {
        const now = Date.now();
        const last = runtime.recentPosts[key] || 0;
        runtime.recentPosts[key] = now;
        return now - last < 1500;
    }

    function postLocalRoute(data, route, withTrailingSlash) {
        return new Promise((resolve, reject) => {
            const url = buildLocalUrl(route, withTrailingSlash);
            const request = {
                method: "POST",
                url,
                data: JSON.stringify(data || {}),
                headers: { "Content-Type": "application/json" },
                timeout: SETTINGS.requestTimeoutMs,
                onload: (response) => {
                    const status = Number(response && response.status);
                    if (status >= 200 && status < 300) {
                        log("info", "local-http", "Local request succeeded.", {
                            route,
                            status,
                        });
                        resolve(response);
                        return;
                    }

                    const error = new Error(`Unexpected local status: ${status}`);
                    error.status = status;
                    reject(error);
                },
                onerror: (error) => reject(error || new Error("Local request failed.")),
                ontimeout: () => reject(new Error("Local request timed out.")),
            };

            try {
                GM_xmlhttpRequest(request);
                log("debug", "local-http", "Local request sent.", { route, url });
            } catch (error) {
                reject(error);
            }
        });
    }

    function showLocalError(route, error) {
        const url = buildLocalUrl(route, route !== ROUTES.embyPrimary);
        log("error", "local-http", "Local service is unavailable.", { url, error });
        if (isServiceDownError(error)) {
            showNotice("service", null, { url });
            return;
        }

        showNotice("error", null, { url });
    }

    function isServiceDownError(error) {
        if (!error) {
            return true;
        }
        if (error.status === 0 || error.status === undefined) {
            return true;
        }
        const message = String(error.message || "");
        return /timed out|failed|network/i.test(message);
    }

    function currentLocale() {
        const candidates = [];
        const htmlLang = document.documentElement
            ? document.documentElement.lang
            : "";
        candidates.push(htmlLang);
        candidates.push(navigator.language || "");
        if (Array.isArray(navigator.languages)) {
            candidates.push(...navigator.languages);
        }
        if (typeof Intl !== "undefined" && Intl.DateTimeFormat) {
            const locale = Intl.DateTimeFormat().resolvedOptions().locale;
            candidates.push(locale || "");
        }
        candidates.push(readLocal("language", ""));
        candidates.push(readLocal("emby.language", ""));
        candidates.push(readLocal("jellyfin_language", ""));

        const normalized = candidates
            .filter(Boolean)
            .map((value) => String(value).toLowerCase());
        if (normalized.some((value) => /^zh-(tw|hk|mo|hant)/.test(value))) {
            return LOCALE.zhHant;
        }
        if (normalized.some((value) => /^zh/.test(value))) {
            return LOCALE.zhHans;
        }
        return LOCALE.en;
    }

    function t(key, params) {
        const bundle = TEXT[currentLocale()] || TEXT.en;
        const template = bundle[key] || TEXT.en[key] || key;
        return String(template).replace(/\{(\w+)\}/g, (_match, name) => {
            return params && params[name] !== undefined ? String(params[name]) : "";
        });
    }

    function isMeaningful(value) {
        if (Array.isArray(value)) {
            return value.length > 0;
        }
        if (value !== null && typeof value === "object") {
            return Object.keys(value).length > 0;
        }
        return Boolean(value);
    }

    function sleep(ms) {
        return new Promise((resolve) => window.setTimeout(resolve, ms));
    }

    function parseJson(text, fallback) {
        try {
            return JSON.parse(text);
        } catch (error) {
            log("error", "parse", "Failed to parse JSON.", { error });
            return fallback;
        }
    }

    function responseJson(response, fallback) {
        if (!response || typeof response.clone !== "function") {
            return Promise.resolve(fallback);
        }

        return response
            .clone()
            .json()
            .catch((error) => {
                log("error", "parse", "Failed to parse response JSON.", { error });
                return fallback;
            });
    }

    function visibleElement(nodes) {
        if (!nodes) {
            return null;
        }

        if (nodes instanceof NodeList) {
            for (const node of nodes) {
                if (node && node.offsetParent !== null) {
                    return node;
                }
            }
            return null;
        }

        return nodes;
    }

    function throttle(fn, delay) {
        let lastTime = 0;
        return function (...args) {
            const now = Date.now();
            if (now - lastTime < delay) {
                return undefined;
            }

            lastTime = now;
            return fn.apply(this, args);
        };
    }

    function createElement(tag, className, text) {
        const element = document.createElement(tag);
        if (className) {
            element.className = className;
        }
        if (text !== undefined) {
            element.textContent = text;
        }
        return element;
    }

    function installNoticeStyle() {
        if (document.getElementById(IDS.noticeStyle)) {
            return;
        }

        const style = document.createElement("style");
        style.id = IDS.noticeStyle;
        style.textContent = `
@keyframes etlpNoticeIn {
    from { opacity: 0; transform: translate3d(24px, 12px, 0) scale(0.98); }
    to { opacity: 1; transform: translate3d(0, 0, 0) scale(1); }
}
@keyframes etlpNoticeOut {
    from { opacity: 1; transform: translate3d(0, 0, 0) scale(1); }
    to { opacity: 0; transform: translate3d(18px, 8px, 0) scale(0.98); }
}
#${IDS.notice} {
    position: fixed;
    right: 28px;
    bottom: 28px;
    z-index: 2147483647;
    width: min(340px, calc(100vw - 32px));
    min-height: 92px;
    overflow: hidden;
    border-radius: 18px;
    color: #ffffff;
    background: #111827;
    box-shadow: 0 22px 50px rgba(0, 0, 0, 0.34);
    animation: etlpNoticeIn 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
#${IDS.notice}.etlp-notice-hide {
    animation: etlpNoticeOut 180ms ease-in forwards;
}
.etlp-notice-bg {
    position: absolute;
    inset: -28px;
    background-image: var(--etlp-notice-icon);
    background-size: cover;
    background-position: center;
    filter: blur(22px) saturate(1.18);
    opacity: 0.62;
    transform: scale(1.08);
}
.etlp-notice-shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgba(10, 10, 12, 0.72), rgba(10, 10, 12, 0.38));
}
.etlp-notice-content {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 92px;
    padding: 16px 18px;
}
.etlp-notice-icon {
    width: 58px;
    height: 58px;
    flex: 0 0 auto;
    border-radius: 15px;
    box-shadow: 0 10px 22px rgba(0, 0, 0, 0.28);
}
.etlp-notice-title {
    font-size: 20px;
    font-weight: 800;
    line-height: 1.2;
}
.etlp-notice-subtitle {
    margin-top: 4px;
    font-size: 13px;
    line-height: 1.35;
    opacity: 0.86;
}
@media (max-width: 520px) {
    #${IDS.notice} {
        right: 16px;
        bottom: 16px;
    }
}
`;
        document.head.appendChild(style);
    }

    function armPlaybackErrorSuppression() {
        runtime.playbackErrorSuppressUntil = Date.now() + 4000;
        suppressPlaybackErrorDialogs();

        if (runtime.playbackErrorSuppressObserver || !document.body) {
            return;
        }

        runtime.playbackErrorSuppressObserver = new MutationObserver(() => {
            suppressPlaybackErrorDialogs();
            stopPlaybackErrorSuppressionIfExpired();
        });
        runtime.playbackErrorSuppressObserver.observe(document.body, {
            childList: true,
            subtree: true,
        });
    }

    function stopPlaybackErrorSuppressionIfExpired() {
        if (Date.now() <= runtime.playbackErrorSuppressUntil) {
            return;
        }

        const observer = runtime.playbackErrorSuppressObserver;
        if (observer) {
            observer.disconnect();
            runtime.playbackErrorSuppressObserver = null;
        }
    }

    function suppressPlaybackErrorDialogs() {
        if (Date.now() > runtime.playbackErrorSuppressUntil || !document.body) {
            return;
        }

        const dialogs = document.querySelectorAll(".alertDialog");
        for (const dialog of dialogs) {
            if (isPlaybackErrorDialog(dialog)) {
                closePlaybackErrorDialog(dialog);
            }
        }
        removeSuppressedBackdrops();
    }

    function isPlaybackErrorDialog(node) {
        if (!node || node.nodeType !== 1 || !node.classList) {
            return false;
        }
        if (!node.classList.contains("alertDialog")) {
            return false;
        }

        const title = node.querySelector(".formDialogHeaderTitle");
        const body = node.querySelector(".dialogContentInner");
        const text = `${safeText(title)} ${safeText(body)}`;
        return /播放错误|播放錯誤|playback error/i.test(text) ||
            /处理请求时出错|處理請求時出錯|incompatible|compatible/i.test(text);
    }

    function safeText(node) {
        return node && typeof node.textContent === "string" ? node.textContent : "";
    }

    function closePlaybackErrorDialog(dialog) {
        dialog.setAttribute("data-etlp-suppressed", "true");

        const button = dialog.querySelector('button[data-id="ok"]');
        if (button && typeof button.click === "function") {
            button.click();
        }

        const container = closestElement(dialog, ".dialogContainer.dialogBackdrop");
        const removable = container || dialog;
        removable.setAttribute("data-etlp-suppressed", "true");
        removable.style.setProperty("display", "none", "important");

        if (typeof removable.remove === "function") {
            removable.remove();
        }
    }

    function removeSuppressedBackdrops() {
        const containers = document.querySelectorAll(
            ".dialogContainer.dialogBackdrop"
        );
        for (const container of containers) {
            const suppressed = container.querySelector(
                '.alertDialog[data-etlp-suppressed="true"]'
            );
            if (suppressed && typeof container.remove === "function") {
                container.remove();
            }
        }
    }

    function closestElement(node, selector) {
        if (!node || typeof node.closest !== "function") {
            return null;
        }
        return node.closest(selector);
    }

    function showNotice(kind, detail, params) {
        const notice = noticeContent(kind, detail, params);
        showNoticeFrame(notice.title, notice.body);
    }

    function noticeContent(kind, detail, params) {
        if (kind === "normal") {
            return {
                title: t("normalTitle"),
                body: t("normalBody"),
            };
        }
        if (kind === "service") {
            return {
                title: t("serviceTitle"),
                body: t("serviceBody", params),
            };
        }
        if (kind === "plain") {
            return {
                title: t("portTitle"),
                body: detail || t("portBody", params),
            };
        }
        return {
            title: t("errorTitle"),
            body: t("errorBody"),
        };
    }

    function showNoticeFrame(titleText, bodyText) {
        const render = () => {
            installNoticeStyle();

            const existing = document.getElementById(IDS.notice);
            if (existing) {
                existing.remove();
            }

            const notice = createElement("div", "", "");
            notice.id = IDS.notice;
            notice.style.setProperty("--etlp-notice-icon", `url("${ASSETS.icon}")`);

            const bg = createElement("div", "etlp-notice-bg", "");
            const shade = createElement("div", "etlp-notice-shade", "");
            const content = createElement("div", "etlp-notice-content", "");
            const icon = createElement("img", "etlp-notice-icon", "");
            const textWrap = createElement("div", "", "");
            const title = createElement("div", "etlp-notice-title", titleText);
            const desc = createElement(
                "div",
                "etlp-notice-subtitle",
                bodyText || ""
            );

            icon.src = ASSETS.icon;
            icon.alt = "etlp";
            icon.decoding = "async";

            textWrap.appendChild(title);
            textWrap.appendChild(desc);
            content.appendChild(icon);
            content.appendChild(textWrap);
            notice.appendChild(bg);
            notice.appendChild(shade);
            notice.appendChild(content);
            document.body.appendChild(notice);

            window.setTimeout(() => {
                notice.classList.add("etlp-notice-hide");
                window.setTimeout(() => notice.remove(), 220);
            }, 2600);
        };

        if (document.body) {
            render();
            return;
        }

        document.addEventListener("DOMContentLoaded", render, { once: true });
    }

    function registerMenus() {
        if (typeof GM_registerMenuCommand !== "function") {
            log("debug", "menu", "Userscript menu API is unavailable.", {});
            return;
        }

        for (const id of runtime.menus) {
            try {
                if (typeof GM_unregisterMenuCommand === "function") {
                    GM_unregisterMenuCommand(id);
                }
            } catch (error) {
                log("error", "menu", "Failed to unregister menu item.", { id, error });
            }
        }

        runtime.menus = [];
        addMenu(t("menuCurrentPort", { port: currentPort() }), () => {
            showNotice("plain", t("menuPortNotice", { port: currentPort() }));
        });
        addMenu(t("menuSetPort"), setPortMenu);
        addMenu(t("menuResetPort"), () => {
            resetPort();
            showNotice("plain", t("portReset"));
        });
        addToggleMenu(
            STORAGE.webPlayerEnabled,
            t("menuServerScript"),
            t("menuDisabled"),
            t("menuEnabled")
        );
        addToggleMenu(
            STORAGE.mountDiskEnabled,
            t("menuMountDisk"),
            t("menuEnabled"),
            t("menuDisabled")
        );

        if (readLocal("etlpTaskManager", null) !== null) {
            addMenu(t("menuOpenCacheTasks"), () => {
                postLocal({ showTaskManager: true }, ROUTES.embyLegacy).catch(
                    (error) => showLocalError(ROUTES.embyLegacy, error)
                );
            });
        }

        if (
            SETTINGS.hideResumeSeries ||
            readLocal(STORAGE.resumeHideEnabled, "false") === "true"
        ) {
            addMenu(t("menuHideSeries"), hideCurrentSeries);
            addMenu(t("menuResetSeries"), resetHiddenSeries);
        }
    }

    function addMenu(title, callback) {
        try {
            const id = GM_registerMenuCommand(title, callback);
            runtime.menus.push(id);
        } catch (error) {
            log("error", "menu", "Failed to register menu item.", { title, error });
        }
    }

    function addToggleMenu(key, prefix, trueText, falseText) {
        const enabled = readLocal(key, "false") === "true";
        const state = enabled ? trueText : falseText;
        addMenu(`${prefix}: ${state}`, () => {
            writeLocal(key, enabled ? "false" : "true");
            registerMenus();
        });
    }

    function setPortMenu() {
        const input = prompt(t("portPrompt"), String(currentPort()));

        if (input === null) {
            return;
        }

        const port = parsePort(input);
        if (!port) {
            showNotice("plain", t("invalidPort"));
            return;
        }

        setPort(port);
        showNotice("plain", t("portSaved", { port }));
    }

    function hideCurrentSeries() {
        const match = window.location.href.match(/id=(\d+)/);
        if (!match || !match[1]) {
            showNotice("plain", t("seriesHint"));
            return;
        }

        const seriesId = match[1];
        const raw = readLocal(STORAGE.hiddenSeriesIds, "[]");
        const list = parseJson(raw, []);
        if (!Array.isArray(list)) {
            writeLocal(STORAGE.hiddenSeriesIds, JSON.stringify([seriesId]));
            showNotice("plain", t("seriesHidden", { id: seriesId }));
            return;
        }

        if (list.includes(seriesId)) {
            showNotice("plain", t("seriesExists"));
            return;
        }

        list.push(seriesId);
        writeLocal(STORAGE.hiddenSeriesIds, JSON.stringify(list));
        showNotice("plain", t("seriesHidden", { id: seriesId }));
    }

    function resetHiddenSeries() {
        deleteLocal(STORAGE.hiddenSeriesIds);
        log("info", "resume", "Hidden series settings reset.", {});
        showNotice("plain", t("seriesReset"));
    }

    function getApiClient() {
        if (typeof ApiClient !== "undefined") {
            return ApiClient;
        }
        if (hostWindow && hostWindow.ApiClient) {
            return hostWindow.ApiClient;
        }
        return null;
    }

    function apiClientPayload() {
        const client = getApiClient();
        if (!client) {
            return {};
        }

        return {
            _serverAddress: client._serverAddress || "",
            _serverVersion: client._serverVersion || "",
        };
    }

    function updateServerName() {
        const client = getApiClient();
        if (!client) {
            return;
        }

        if (!runtime.serverName && typeof client._appName === "string") {
            const name = client._appName.split(" ")[0];
            runtime.serverName = name ? name.toLowerCase() : null;
            log("debug", "server", "Detected server name.", {
                serverName: runtime.serverName,
            });
        }

        if (
            client._deviceName !== "embyToLocalPlayer" &&
            readLocal(STORAGE.webPlayerEnabled, "false") !== "true"
        ) {
            client._deviceName = "embyToLocalPlayer";
            warmResumeCache();
        }
    }

    function buildEmbyPayload(playbackUrl, request, playbackData, extraData) {
        return {
            ApiClient: apiClientPayload(),
            playbackData: playbackData || {},
            playbackUrl: playbackUrl || "",
            request: normalizeRequest(request),
            mountDiskEnable: readLocal(STORAGE.mountDiskEnabled, null),
            extraData: extraData || {},
            fistTime: runtime.firstPlay,
        };
    }

    function normalizeRequest(request) {
        if (!request) {
            return {};
        }

        if (request.headers && typeof request.headers === "object") {
            return { headers: normalizeHeaders(request.headers) };
        }

        return request;
    }

    function normalizeHeaders(headers) {
        const output = {};
        if (!headers) {
            return output;
        }

        if (typeof headers.forEach === "function") {
            headers.forEach((value, key) => {
                output[key] = value;
            });
            return output;
        }

        for (const key of Object.keys(headers)) {
            const value = headers[key];
            if (value !== undefined && value !== null) {
                output[key] = String(value);
            }
        }
        return output;
    }

    function sendEmbyPlayback(playbackUrl, request, playbackData, extraData) {
        const payload = buildEmbyPayload(
            playbackUrl,
            request,
            playbackData,
            extraData
        );
        armPlaybackErrorSuppression();
        return postLocal(payload, ROUTES.embyPrimary)
            .then(() => {
                runtime.firstPlay = false;
                showNotice("normal", noticeSubtitle(extraData));
            })
            .catch((error) => {
                showLocalError(ROUTES.embyPrimary, error);
            });
    }

    function noticeSubtitle(extraData) {
        const main = extraData && extraData.mainEpInfo;
        if (!main) {
            return "Handing playback to etlp";
        }

        if (main.SeriesName && main.Name) {
            return `${main.SeriesName} - ${main.Name}`;
        }

        return main.Name || main.SeriesName || "Handing playback to etlp";
    }

    function getPlaybackInfo(itemId) {
        return apiClientCached(
            itemId,
            [runtime.resumePlaybackCache, runtime.allPlaybackCache],
            "playback"
        );
    }

    function getItemInfo(itemId) {
        return apiClientCached(
            itemId,
            [runtime.resumeItemCache, runtime.allItemCache],
            "item"
        );
    }

    function getEpisodesInfo(seasonId) {
        return apiClientCached(seasonId, [runtime.episodesWithPathCache], "episodes");
    }

    async function apiClientCached(itemId, caches, kind) {
        if (!itemId) {
            log("debug", "cache", "Skipped empty item id.", { kind });
            return null;
        }

        for (const cache of caches) {
            if (cache && Object.prototype.hasOwnProperty.call(cache, itemId)) {
                log("tracing", "cache", "Cache hit.", { kind, itemId });
                return cache[itemId];
            }
        }

        const client = getApiClient();
        if (!client) {
            log("error", "cache", "ApiClient is unavailable.", { kind, itemId });
            return null;
        }

        let result = null;
        try {
            if (kind === "playback" && typeof client.getPlaybackInfo === "function") {
                result = await client.getPlaybackInfo(itemId);
            } else if (kind === "item" && typeof client.getItem === "function") {
                const userId = client._serverInfo && client._serverInfo.UserId;
                result = await client.getItem(userId, itemId);
            } else if (
                kind === "episodes" &&
                typeof client.getEpisodes === "function"
            ) {
                result = await client.getEpisodes(itemId, {
                    Fields: "MediaSources,Path,ProviderIds",
                    SeasonId: itemId,
                });
            }
        } catch (error) {
            log("error", "cache", "ApiClient request failed.", {
                kind,
                itemId,
                error,
            });
        }

        if (!result) {
            return null;
        }

        for (const cache of caches) {
            if (cache) {
                cache[itemId] = result;
            }
        }

        return result;
    }

    function correctResumeItemId(itemId) {
        if (runtime.serverName !== "emby") {
            return itemId;
        }
        if (!Array.isArray(runtime.resumeRawCache)) {
            return itemId;
        }
        if (!Array.isArray(runtime.episodesCache)) {
            return itemId;
        }

        const resumeIds = runtime.resumeRawCache
            .map((item) => item && item.Id)
            .filter(Boolean);
        if (resumeIds.includes(itemId)) {
            return itemId;
        }

        const pageMatch = window.location.href.match(/\/item\?id=(\d+)/);
        const pageId = pageMatch && pageMatch[1];
        const firstEpisode = runtime.episodesCache[0];
        if (
            pageId &&
            resumeIds.includes(pageId) &&
            firstEpisode &&
            itemId === firstEpisode.Id
        ) {
            return pageId;
        }

        const currentEpisode = runtime.episodesCache.find(
            (item) => item && item.Id === itemId
        );
        const seasonId = currentEpisode && currentEpisode.SeasonId;
        const resumeItem = runtime.resumeRawCache.find(
            (item) => item && item.SeasonId === seasonId
        );
        if (seasonId && resumeItem && resumeItem.Id) {
            log("info", "resume", "Corrected resume item id.", {
                oldId: itemId,
                newId: resumeItem.Id,
            });
            return resumeItem.Id;
        }

        return itemId;
    }

    async function handlePlaybackInfo(rawInput, urlText, options) {
        const rawId = extractPlaybackItemId(urlText);
        if (!rawId) {
            log("error", "playback", "Failed to extract item id from URL.", {
                urlText,
            });
            return false;
        }

        const episodeResponse = Array.isArray(runtime.episodesCache)
            ? runtime.episodesCache[1]
            : null;
        let itemId = rawId;
        const [playbackData, mainEpInfo, episodesData] = await Promise.all([
            getPlaybackInfo(itemId),
            getItemInfo(itemId),
            episodeResponse ? responseJson(episodeResponse, null) : Promise.resolve(null),
        ]);

        let episodesInfo =
            episodesData && Array.isArray(episodesData.Items)
                ? episodesData.Items
                : null;
        runtime.episodesCache = Array.isArray(episodesInfo) ? episodesInfo : [];

        const correctId = correctResumeItemId(itemId);
        let playbackUrl = urlText.replace(`/${rawId}/`, `/${correctId}/`);
        if (correctId !== itemId) {
            itemId = correctId;
            const [newPlayback, newItem] = await Promise.all([
                getPlaybackInfo(itemId),
                getItemInfo(itemId),
            ]);
            const startTicks =
                newItem && newItem.UserData
                    ? newItem.UserData.PlaybackPositionTicks
                    : null;
            if (startTicks !== null && startTicks !== undefined) {
                playbackUrl = playbackUrl.replace(
                    "StartTimeTicks=0",
                    `StartTimeTicks=${startTicks}`
                );
            }
            return sendCorrectedPlayback(
                playbackUrl,
                options || rawInput,
                newPlayback,
                newItem
            );
        }

        return sendCorrectedPlayback(
            playbackUrl,
            options || rawInput,
            playbackData,
            mainEpInfo
        );

        async function sendCorrectedPlayback(url, request, media, item) {
            if (!media || !item) {
                log("error", "playback", "Playback payload is incomplete.", {
                    itemId,
                });
                return false;
            }

            const playlist =
                runtime.playlistCache && runtime.playlistCache.Items
                    ? runtime.playlistCache.Items
                    : null;
            runtime.playlistCache = null;

            const extraData = {
                mainEpInfo: item,
                episodesInfo,
                playlistInfo: playlist,
                gmInfo: typeof GM_info === "undefined" ? null : GM_info,
                userAgent: navigator.userAgent,
            };

            if (item.Type === "Trailer") {
                showNotice("error", t("trailer"));
                return false;
            }

            if (SETTINGS.useWebPlayerForLiveTv && item.Type === "TvChannel") {
                return "web-live-tv";
            }

            const sources = Array.isArray(media.MediaSources)
                ? media.MediaSources
                : [];
            const source = sources[0];
            const path = source && typeof source.Path === "string" ? source.Path : "";
            if (/\Wbackdrop/i.test(path)) {
                return false;
            }

            await sendEmbyPlayback(url, request, media, extraData);
            return true;
        }
    }

    function extractPlaybackItemId(urlText) {
        const match = String(urlText || "").match(/\/Items\/(\w+)\/PlaybackInfo/);
        return match && match[1] ? match[1] : null;
    }

    async function handleItemClick(item) {
        if (!item || !item.Id) {
            return;
        }

        const client = getApiClient();
        if (!client) {
            log("error", "click", "ApiClient is unavailable.", {});
            return;
        }

        const itemId = item.Id;
        const seasonId = item.SeasonId;
        const [mainEpInfo, playbackData, episodesData] = await Promise.all([
            getItemInfo(itemId),
            getPlaybackInfo(itemId),
            seasonId ? getEpisodesInfo(seasonId) : Promise.resolve(null),
        ]);

        if (!mainEpInfo || !playbackData) {
            log("error", "click", "Clicked item playback data is incomplete.", {
                itemId,
            });
            return;
        }

        const userData = item.UserData || {};
        const serverInfo = client._serverInfo || {};
        const authInfo = client._userAuthInfo || {};
        const accessToken = authInfo.AccessToken || serverInfo.AccessToken;
        if (!accessToken) {
            log("error", "click", "Access token is unavailable.", { itemId });
            return;
        }

        const params = new URLSearchParams({
            "X-Emby-Device-Id": client._deviceId || "",
            StartTimeTicks: String(userData.PlaybackPositionTicks || 0),
            "X-Emby-Token": accessToken,
            UserId: serverInfo.UserId || "",
            IsPlayback: "true",
        });
        const baseUrl = `${window.location.origin}/emby/Items/${itemId}`;
        const playbackUrl = `${baseUrl}/PlaybackInfo?${params.toString()}`;
        const episodesInfo =
            episodesData && Array.isArray(episodesData.Items)
                ? episodesData.Items
                : [];
        const extraData = {
            mainEpInfo,
            episodesInfo,
            playlistInfo: [],
            gmInfo: typeof GM_info === "undefined" ? null : GM_info,
            userAgent: navigator.userAgent,
        };

        await sendEmbyPlayback(playbackUrl, {}, playbackData, extraData);
    }

    function installClickCapture() {
        document.addEventListener(
            "click",
            (event) => {
                if (readLocal(STORAGE.webPlayerEnabled, "false") === "true") {
                    return;
                }

                const target = event.target;
                if (!target || typeof target.closest !== "function") {
                    return;
                }

                const playButton = target.closest(
                    'button.cardOverlayFab-primary[data-action="play"]'
                );
                if (!playButton) {
                    return;
                }

                const container = target.closest('div[is="emby-itemscontainer"]');
                if (!container || (!container._itemSource && !container.items)) {
                    log("debug", "click", "No playable item container found.", {});
                    return;
                }

                const card = target.closest(
                    ".virtualScrollItem.card, .backdropCard[data-index]"
                );
                if (!card) {
                    return;
                }

                const indexText =
                    card._dataItemIndex !== undefined
                        ? card._dataItemIndex
                        : card.dataset && card.dataset.index;
                const index = Number.parseInt(String(indexText), 10);
                const itemList = container._itemSource || container.items || [];
                if (!Number.isInteger(index) || index < 0 || index >= itemList.length) {
                    log("error", "click", "Clicked item index is out of range.", {
                        index,
                        length: itemList.length,
                    });
                    return;
                }

                const item = itemList[index];
                const itemType = item && item.Type;
                if (itemType !== "Movie" && itemType !== "Episode") {
                    log("debug", "click", "Clicked item type is unsupported.", {
                        itemType,
                    });
                    return;
                }

                event.preventDefault();
                event.stopImmediatePropagation();
                log("info", "click", "Captured card play click.", {
                    index,
                    itemId: item.Id,
                });
                handleItemClick(item);
            },
            true
        );
    }

    function clearOptionalCaches() {
        runtime.resumeRawCache = null;
        runtime.resumeItemCache = {};
        runtime.resumePlaybackCache = {};
        runtime.allItemCache = {};
        runtime.allPlaybackCache = {};
        runtime.episodesCache = [];
        runtime.episodesWithPathCache = {};
    }

    async function warmResumeCache() {
        let resumeIds = [];
        if (!isMeaningful(runtime.resumeRawCache)) {
            const raw = readLocal(STORAGE.resumeCacheIds, "[]");
            const parsed = parseJson(raw, []);
            if (!Array.isArray(parsed)) {
                return;
            }
            resumeIds = parsed;
        } else {
            const rawItems = Array.isArray(runtime.resumeRawCache)
                ? runtime.resumeRawCache
                : [];
            resumeIds = rawItems
                .slice(0, SETTINGS.resumeWarmCount)
                .map((item) => item && item.Id)
                .filter(Boolean);
            const seasonIds = rawItems
                .slice(0, SETTINGS.resumeWarmCount)
                .map((item) => item && item.SeasonId)
                .filter(Boolean);
            await Promise.all(seasonIds.map((sid) => getEpisodesInfo(sid)));
            writeLocal(STORAGE.resumeCacheIds, JSON.stringify(resumeIds));
        }

        const jobs = [
            [runtime.resumePlaybackCache, getPlaybackInfo],
            [runtime.resumeItemCache, getItemInfo],
        ];

        for (const [cache, getter] of jobs) {
            const missing = resumeIds.filter(
                (id) => id && !Object.prototype.hasOwnProperty.call(cache, id)
            );
            if (missing.length > 0) {
                await Promise.all(missing.map((id) => getter(id)));
            }
        }
    }

    async function cloneAndCache(response, key, cache) {
        const data = await responseJson(response, null);
        if (data && key) {
            cache[key] = data;
        }
        return data;
    }

    function rewrittenJsonResponse(response, data) {
        return new Response(JSON.stringify(data), {
            status: response.status,
            statusText: response.statusText,
            headers: response.headers,
        });
    }

    const addOpenFolderButton = throttle(addOpenFolderButtonInner, 100);

    async function addOpenFolderButtonInner(itemId) {
        if (SETTINGS.disableOpenFolder) {
            return;
        }

        let mediaSources = null;
        for (let count = 0; count < 5; count += 1) {
            await sleep(500);
            mediaSources = visibleElement(document.querySelectorAll("div.mediaSources"));
            if (mediaSources) {
                break;
            }
        }

        if (!mediaSources) {
            return;
        }

        const pathDiv = mediaSources.querySelector(
            'div[class^="sectionTitle sectionTitle-cards"] > div'
        );
        if (!pathDiv || pathDiv.className === "mediaInfoItems") {
            return;
        }
        if (pathDiv.id === "addFileNameElement") {
            return;
        }

        let fullPath = pathDiv.textContent || "";
        const looksLikePath = /[\\/:]/.test(fullPath);
        const looksLikeSize = /\d{1,3}\.?\d{0,2} (MB|GB)/.test(fullPath);
        if (!looksLikePath || looksLikeSize) {
            return;
        }

        const itemData = runtime.allItemCache[itemId] || null;
        const strmFile =
            fullPath.startsWith("http") && itemData ? itemData.Path : null;
        if (strmFile) {
            fullPath = strmFile;
            pathDiv.appendChild(document.createElement("br"));
            pathDiv.appendChild(document.createTextNode(strmFile));
        }

        const button = createElement(
            "a",
            "raised item-tag-button nobackdropfilter",
            "Open Folder"
        );
        button.id = "openFolderButton";
        button.setAttribute("is", "emby-linkbutton");
        button.addEventListener("click", () => {
            postLocal({ full_path: fullPath }, ROUTES.openFolder).catch((error) => {
                showLocalError(ROUTES.openFolder, error);
            });
        });
        pathDiv.parentNode.insertBefore(button, pathDiv);
    }

    async function addFileName(response) {
        let mediaSources = null;
        for (let count = 0; count < 5; count += 1) {
            await sleep(500);
            mediaSources = visibleElement(document.querySelectorAll("div.mediaSources"));
            if (mediaSources) {
                break;
            }
        }

        if (!mediaSources) {
            return;
        }

        let pathDivs = Array.from(
            mediaSources.querySelectorAll(
                'div[class^="sectionTitle sectionTitle-cards"] > div'
            )
        );
        const first = pathDivs[0];
        if (!first || first.id === "addFileNameElement") {
            return;
        }

        const isAdmin = !/\d{4}\/\d+\/\d+/.test(first.textContent || "");
        const isStrm = (first.textContent || "").startsWith("http");
        if (isAdmin) {
            if (!isStrm) {
                return;
            }
            pathDivs = pathDivs.filter((_, index) => index % 2 === 0);
        }

        const data = await responseJson(response, {});
        const sources = Array.isArray(data.MediaSources) ? data.MediaSources : [];
        pathDivs.forEach((pathDiv, index) => {
            const source = sources[index];
            if (!source) {
                return;
            }

            const filePath = typeof source.Path === "string" ? source.Path : "";
            const isRemote = filePath.startsWith("http");
            let fileName = source.Name || "";
            if (!isRemote) {
                const parts = filePath.split(/[\\/]/);
                fileName =
                    SETTINGS.crackFullPath && !isAdmin
                        ? filePath
                        : parts[parts.length - 1] || fileName;
            }

            const fileDiv = createElement("div", "", fileName);
            fileDiv.id = "addFileNameElement";
            if (isRemote && !isAdmin && SETTINGS.crackFullPath && filePath) {
                fileDiv.appendChild(document.createElement("br"));
                fileDiv.appendChild(document.createTextNode(filePath));
            }
            pathDiv.parentNode.insertBefore(fileDiv, pathDiv);
        });
    }

    function requestUrl(input) {
        if (typeof input === "string") {
            return input;
        }
        if (input && typeof input.url === "string") {
            return input.url;
        }
        return "";
    }

    function cloneRequestInput(input, urlText) {
        if (typeof input === "string") {
            return urlText;
        }
        try {
            return new Request(urlText, input);
        } catch (error) {
            log("error", "request", "Failed to rebuild Request input.", { error });
            return input;
        }
    }

    async function handleFetch(input, options) {
        const urlText = requestUrl(input);
        if (!urlText) {
            return rawFetch(input, options);
        }

        updateServerName();

        if (runtime.metadataMayChange && urlText.includes("Items")) {
            if (urlText.includes("reqformat") && !urlText.includes("fields")) {
                clearOptionalCaches();
                runtime.metadataMayChange = false;
                log("info", "cache", "Cleared cache after metadata change.", {});
            }
        }

        const listResponse = await handleListFetch(input, options, urlText);
        if (listResponse) {
            return listResponse;
        }

        const episodeResponse = await handleEpisodesFetch(input, options, urlText);
        if (episodeResponse) {
            return episodeResponse;
        }

        const resumeResponse = await handleResumeFetch(input, options, urlText);
        if (resumeResponse) {
            return resumeResponse;
        }

        const itemResponse = await handleItemFetch(input, options, urlText);
        if (itemResponse) {
            return itemResponse;
        }

        const playbackResponse = await handlePlaybackFetch(input, options, urlText);
        if (playbackResponse !== null) {
            return playbackResponse;
        }

        if (/\/MetadataEditor|\/Refresh\?/.test(urlText)) {
            if (urlText.includes("MetadataEditor")) {
                runtime.metadataMayChange = true;
            } else {
                clearOptionalCaches();
                log("info", "cache", "Cleared cache after refresh.", {});
            }
        }

        return rawFetch(input, options);
    }

    async function handleListFetch(input, options, urlText) {
        const isLargeItems = urlText.includes("Items?");
        const hasPlaylistLimit = /Limit=(300|1000|5\d\d\d)/.test(urlText);
        if (!isLargeItems || !hasPlaylistLimit) {
            return null;
        }

        const response = await rawFetch(input, options);
        if (runtime.serverName === "emby") {
            maybeCacheSeasonList(urlText, response);
        }

        const data = await responseJson(response, null);
        const items = data && Array.isArray(data.Items) ? data.Items : [];
        const first = items[0];
        if (!first) {
            log("error", "playlist", "Playlist is empty.", {});
            return response;
        }

        if (["Movie", "MusicVideo", "Episode"].includes(first.Type)) {
            runtime.playlistCache = data;
            log("debug", "playlist", "Cached playlist payload.", {
                count: items.length,
            });
        }
        return response;
    }

    function maybeCacheSeasonList(urlText, response) {
        const client = getApiClient();
        const promise = client && client._userViewsPromise;
        if (!promise || typeof promise.then !== "function") {
            return;
        }

        promise
            .then((result) => {
                const views = result && Array.isArray(result.Items) ? result.Items : [];
                const ids = views.map((item) => item && item.Id).filter(Boolean);
                if (ids.length === 0) {
                    return;
                }

                const regex = new RegExp(`ParentId=(${ids.join("|")})`);
                if (!regex.test(urlText)) {
                    runtime.episodesCache = ["Items", response.clone()];
                    log("debug", "episodes", "Cached Items episode response.", {});
                }
            })
            .catch((error) => {
                log("error", "episodes", "Failed to read user views.", { error });
            });
    }

    async function handleEpisodesFetch(input, options, urlText) {
        const isEpisodes = /\/Episodes\?IsVirtual/.test(urlText);
        const isNextUp = /\/NextUp\?Series/.test(urlText);
        const isItems = /\/Items\?ParentId=\w+/.test(urlText);
        const isSeasonItems = isItems && /Filters=IsNotFolder/.test(urlText);
        if (!isEpisodes && !isNextUp && !isSeasonItems) {
            return null;
        }

        const response = await rawFetch(input, options);
        const match = urlText.match(/\/(Episodes|NextUp|Items)\?/);
        const kind = match && match[1] ? match[1] : "Items";
        runtime.episodesCache = [kind, response.clone()];
        log("debug", "episodes", "Cached episode response.", { kind });
        return response;
    }

    async function handleResumeFetch(input, options, urlText) {
        const isResume = urlText.includes("Items/Resume");
        const isVideo = urlText.includes("MediaTypes=Video");
        if (!isResume || !isVideo) {
            return null;
        }

        let requestUrlText = urlText;
        if (SETTINGS.reorderResume) {
            requestUrlText = urlText.replace(/Fields=([^&]*)/, "Fields=$1,DateCreated");
        }

        const fetchInput = cloneRequestInput(input, requestUrlText);
        const response = await rawFetch(fetchInput, options);
        const data = await responseJson(response, null);
        if (!data || !Array.isArray(data.Items)) {
            return response;
        }

        data.Items = filterHiddenSeries(data.Items);
        data.Items = reorderResumeItems(data.Items);
        runtime.resumeRawCache = data.Items;
        warmResumeCache();
        return rewrittenJsonResponse(response, data);
    }

    function filterHiddenSeries(items) {
        if (!SETTINGS.hideResumeSeries || items.length === 0) {
            return items;
        }

        const raw = readLocal(STORAGE.hiddenSeriesIds, "[]");
        const hideList = parseJson(raw, []);
        if (!Array.isArray(hideList) || hideList.length === 0) {
            return items;
        }

        const result = items.filter((item) => {
            return !item.SeriesId || !hideList.includes(item.SeriesId);
        });
        const hiddenCount = items.length - result.length;
        if (hiddenCount > 0) {
            log("info", "resume", "Hidden resume items removed.", {
                count: hiddenCount,
            });
        }
        return result;
    }

    function reorderResumeItems(items) {
        if (!SETTINGS.reorderResume || items.length <= 2) {
            return items;
        }

        const now = Date.now();
        const cutoff = now - SETTINGS.resumeRecentDays * 24 * 60 * 60 * 1000;
        const firstTwo = items.slice(0, 2);
        const rest = items.slice(2);
        const recent = [];
        const older = [];
        for (const item of rest) {
            const date = item && item.DateCreated ? Date.parse(item.DateCreated) : 0;
            if (date >= cutoff) {
                recent.push(item);
            } else {
                older.push(item);
            }
        }

        log("debug", "resume", "Reordered resume items.", {
            recent: recent.length,
            older: older.length,
        });
        return firstTwo.concat(recent, older);
    }

    async function handleItemFetch(input, options, urlText) {
        const match = urlText.match(/\/Items\/(\w+)\?/);
        if (!match || !match[1]) {
            return null;
        }

        const response = await rawFetch(input, options);
        await cloneAndCache(response, match[1], runtime.allItemCache);
        return response;
    }

    async function handlePlaybackFetch(input, options, urlText) {
        try {
            if (urlText.includes("/PlaybackInfo?UserId")) {
                const webPlayer = readLocal(STORAGE.webPlayerEnabled, "false") === "true";
                if (urlText.includes("IsPlayback=true") && !webPlayer) {
                    const result = await handlePlaybackInfo(input, urlText, options);
                    if (result && result !== "web-live-tv") {
                        armPlaybackErrorSuppression();
                        return new Response(null, { status: 204 });
                    }
                    return null;
                }

                const itemId = extractPlaybackItemId(urlText);
                const response = await rawFetch(input, options);
                if (itemId) {
                    addFileName(response.clone());
                    addOpenFolderButton(itemId);
                    await cloneAndCache(response.clone(), itemId, runtime.allPlaybackCache);
                }
                return response;
            }

            const blocksStopped =
                urlText.includes("/Playing/Stopped") &&
                readLocal(STORAGE.webPlayerEnabled, "false") !== "true";
            if (blocksStopped) {
                armPlaybackErrorSuppression();
                return new Response(null, { status: 204 });
            }
        } catch (error) {
            log("error", "playback", "Failed to handle playback request.", {
                urlText,
                error,
            });
            return new Response(null, { status: 204 });
        }

        return null;
    }

    function installFetchHook() {
        hostWindow.fetch = function (input, options) {
            return handleFetch(input, options);
        };
    }

    function installXhrHook() {
        XMLHttpRequest.prototype.setRequestHeader = function (header, value) {
            if (!this._etlpHeaders) {
                this._etlpHeaders = {};
            }
            this._etlpHeaders[header] = value;
            return rawSetHeader.apply(this, arguments);
        };

        XMLHttpRequest.prototype.open = function (method, url) {
            this._etlpMethod = method;
            this._etlpUrl = url;
            this._etlpHeaders = {};

            if (runtime.serverName === null && String(url).includes("X-Plex-Product")) {
                runtime.serverName = "plex";
            }

            const catchesPlex =
                runtime.serverName === "plex" &&
                String(url).includes("playQueues?type=video");
            if (catchesPlex && readLocal(STORAGE.webPlayerEnabled, "false") !== "true") {
                this._etlpPlexIntercepted = true;
                handlePlexPlayback(this);
                return undefined;
            }

            return rawOpen.apply(this, arguments);
        };

        XMLHttpRequest.prototype.send = function (body) {
            if (this._etlpPlexIntercepted) {
                return undefined;
            }

            const method = String(this._etlpMethod || "");
            const url = String(this._etlpUrl || "");
            const catchesJellyfin = method === "POST" && url.endsWith("PlaybackInfo");
            if (
                catchesJellyfin &&
                readLocal(STORAGE.webPlayerEnabled, "false") !== "true"
            ) {
                armPlaybackErrorSuppression();
                handleJellyfinPlayback(this, body);
                return undefined;
            }

            return rawSend.apply(this, arguments);
        };
    }

    function handlePlexPlayback(xhr) {
        rawFetch(xhr._etlpUrl, {
            method: xhr._etlpMethod,
            headers: { Accept: "application/json" },
        })
            .then((response) => response.json())
            .then((res) => {
                const data = {
                    playbackData: res,
                    playbackUrl: xhr._etlpUrl,
                    mountDiskEnable: readLocal(STORAGE.mountDiskEnabled, null),
                    extraData: {
                        gmInfo: typeof GM_info === "undefined" ? null : GM_info,
                        userAgent: navigator.userAgent,
                    },
                };
                return postLocal(data, ROUTES.plex);
            })
            .then(() => showNotice("normal", "Plex playback handed to etlp"))
            .catch((error) => showLocalError(ROUTES.plex, error));
    }

    function handleJellyfinPlayback(xhr, body) {
        const parsed = typeof body === "string" ? parseJson(body, {}) : {};
        const allowed = [
            "MediaSourceId",
            "StartTimeTicks",
            "UserId",
            "SubtitleStreamIndex",
            "AudioStreamIndex",
        ];
        const query = {};
        for (const key of allowed) {
            if (parsed[key] !== undefined && parsed[key] !== null) {
                query[key] = parsed[key];
            }
        }

        const params = new URLSearchParams(query);
        const playbackUrl = params.toString()
            ? `${xhr._etlpUrl}?${params.toString()}`
            : xhr._etlpUrl;
        handlePlaybackInfo(playbackUrl, playbackUrl, { headers: xhr._etlpHeaders });
    }

    function start() {
        syncSettings();
        registerMenus();
        installClickCapture();
        installFetchHook();
        installXhrHook();
        log("info", "bootstrap", "etlp userscript started.", {
            port: currentPort(),
        });
    }

    start();
})();
