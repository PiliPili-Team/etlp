import { useState, useEffect, useMemo, type ReactNode } from "react";
import type { LangMode } from "../App";
import { I18nContext, makeT, loadMessages, type Messages } from ".";
import { zhCN } from "./zh-CN";

/** Provides the active locale's translator to the descendant tree. */
export function I18nProvider({
    lang,
    children,
}: {
    lang: LangMode;
    children: ReactNode;
}) {
    const [messages, setMessages] = useState<Messages>(zhCN);

    useEffect(() => {
        let active = true;
        loadMessages(lang).then((msgs) => {
            if (active) setMessages(msgs);
        });
        return () => {
            active = false;
        };
    }, [lang]);

    const t = useMemo(() => makeT(messages), [messages]);
    return <I18nContext.Provider value={t}>{children}</I18nContext.Provider>;
}
