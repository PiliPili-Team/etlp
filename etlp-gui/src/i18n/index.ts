import { createContext, useContext } from "react";
import type { LangMode } from "../App";
import { resolveLocale } from "../display";
import { zhCN } from "./zh-CN";

export type Messages = typeof zhCN;
export type I18nKey = keyof Messages;
export type T = (key: I18nKey, vars?: Record<string, string | number>) => string;

// Each locale is a separate dynamic chunk — only the active one is loaded.
const LOADERS: Record<string, () => Promise<Messages>> = {
    "zh-CN": async () => (await import("./zh-CN")).zhCN,
    "zh-TW": async () => (await import("./zh-TW")).zhTW,
    en: async () => (await import("./en")).en,
    ja: async () => (await import("./ja")).ja,
    ko: async () => (await import("./ko")).ko,
    de: async () => (await import("./de")).de,
    it: async () => (await import("./it")).it,
    fr: async () => (await import("./fr")).fr,
    ar: async () => (await import("./ar")).ar,
    es: async () => (await import("./es")).es,
    ru: async () => (await import("./ru")).ru,
    pt: async () => (await import("./pt")).pt,
    sk: async () => (await import("./sk")).sk,
    uk: async () => (await import("./uk")).uk,
    sr: async () => (await import("./sr")).sr,
    tr: async () => (await import("./tr")).tr,
    he: async () => (await import("./he")).he,
    th: async () => (await import("./th")).th,
    pl: async () => (await import("./pl")).pl,
    id: async () => (await import("./id")).id,
};

export async function loadMessages(lang: LangMode): Promise<Messages> {
    const locale = resolveLocale(lang);
    return (await LOADERS[locale]?.()) ?? zhCN;
}

export function makeT(messages: Messages): T {
    return (key, vars) => {
        let str = (messages[key] ?? zhCN[key] ?? key) as string;
        if (vars) {
            for (const [k, v] of Object.entries(vars)) {
                str = str.replace(`{${k}}`, String(v));
            }
        }
        return str;
    };
}

export const I18nContext = createContext<T>(makeT(zhCN));

export function useI18n(): T {
    return useContext(I18nContext);
}
