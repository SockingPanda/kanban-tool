import { useMemo } from "react";
import { usePreferences } from "../preferences/use-preferences";
import { createTranslator } from "./runtime";
import { createFormatters } from "./format";
import { localeTag } from "./locale";

/** The existing preferences context is the sole owner of the UI language. */
export function useI18n(timeZone?: string) {
  const { locale } = usePreferences();
  return useMemo(() => ({ locale, tag: localeTag(locale), t: createTranslator(locale), format: createFormatters(locale, timeZone) }), [locale, timeZone]);
}
