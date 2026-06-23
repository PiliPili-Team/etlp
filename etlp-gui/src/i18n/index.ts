import { createContext, useContext } from "react";
import type { LangMode } from "../App";
import { resolveLocale } from "../display";
import { zhCN } from "./zh-CN";
import { zhTW } from "./zh-TW";
import { en } from "./en";
import { ja } from "./ja";
import { ko } from "./ko";
import { de } from "./de";
import { it } from "./it";
import { fr } from "./fr";
import { ar } from "./ar";
import { es } from "./es";
import { ru } from "./ru";
import { pt } from "./pt";
import { sk } from "./sk";
import { uk } from "./uk";
import { sr } from "./sr";
import { tr } from "./tr";
import { he } from "./he";
import { th } from "./th";
import { pl } from "./pl";
import { id } from "./id";

export type Messages = typeof zhCN;
export type I18nKey = keyof Messages;
export type T = (key: I18nKey, vars?: Record<string, string | number>) => string;

const MESSAGES: Record<string, Messages> = {
    "zh-CN": zhCN,
    "zh-TW": zhTW,
    en,
    ja,
    ko,
    de,
    it,
    fr,
    ar,
    es,
    ru,
    pt,
    sk,
    uk,
    sr,
    tr,
    he,
    th,
    pl,
    id,
};

export function makeT(lang: LangMode): T {
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

export const I18nContext = createContext<T>(makeT("zh-CN"));

export function useI18n(): T {
    return useContext(I18nContext);
}
