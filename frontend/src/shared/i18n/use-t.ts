import { createContext, useContext, useMemo } from "react";

import { type Locale, type T, translator } from "./translate";

const LocaleContext = createContext<Locale | null>(null);

/** Set by each element from the closest `[lang]` around it. */
export const LocaleProvider = LocaleContext.Provider;

export function useLocale(): Locale {
  const locale = useContext(LocaleContext);
  if (locale === null) throw new Error("useLocale outside an element root");
  return locale;
}

export function useT(): T {
  const locale = useLocale();
  return useMemo(() => translator(locale), [locale]);
}
