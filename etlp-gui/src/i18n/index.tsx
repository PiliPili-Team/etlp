import React, { createContext, useContext, useMemo } from "react";
import type { LangMode } from "../App";
import { zhCN } from "./zh-CN";
import { zhTW } from "./zh-TW";
import { en } from "./en";

export type Messages = typeof zhCN;
export type I18nKey = keyof Messages;
export type T = (key: I18nKey, vars?: Record<string, string | number>) => string;

const MESSAGES: Record<string, Messages> = {
    "zh-CN": zhCN,
    "zh-TW": zhTW,
    en,
};

function resolveLocale(lang: LangMode): string {
    if (lang !== "system") return lang;
    const sys = navigator.language;
    if (/^zh-(TW|HK|MO)/i.test(sys)) return "zh-TW";
    if (/^zh/i.test(sys)) return "zh-CN";
    return "en";
}

function makeT(lang: LangMode): T {
    const locale = resolveLocale(lang);
    const msgs = MESSAGES[locale] ?? zhCN;
    return (key, vars) => {
        let str = (msgs[key] ?? zhCN[key] ?? key) as string;
        if (vars) {
            for (const [k, v] of Object.entries(vars)) {
                str = str.replace(`{${k}}`, String(v));
            }
        }
        return str;
    };
}

const I18nContext = createContext<T>(makeT("zh-CN"));

export function I18nProvider({
    lang,
    children,
}: {
    lang: LangMode;
    children: React.ReactNode;
}) {
    const t = useMemo(() => makeT(lang), [lang]);
    return <I18nContext.Provider value={t}>{children}</I18nContext.Provider>;
}

export function useI18n(): T {
    return useContext(I18nContext);
}
