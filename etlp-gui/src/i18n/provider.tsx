import { useMemo, type ReactNode } from "react";
import type { LangMode } from "../App";
import { I18nContext, makeT } from ".";

/** Provides the active locale's translator to the descendant tree. */
export function I18nProvider({
    lang,
    children,
}: {
    lang: LangMode;
    children: ReactNode;
}) {
    const t = useMemo(() => makeT(lang), [lang]);
    return <I18nContext.Provider value={t}>{children}</I18nContext.Provider>;
}
