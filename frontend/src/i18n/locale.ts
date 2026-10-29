export type Locale = "ja" | "en";
export const languageKey = "mofuvoice.language";

export function resolveLocale(saved: string | null, languages: readonly string[]): Locale {
  if (saved === "ja" || saved === "en") {
    return saved;
  }
  for (const language of languages) {
    const base = language.toLowerCase().split(/[-_]/)[0];
    if (base === "ja" || base === "en") {
      return base;
    }
  }
  return "en";
}

export function readLocale(): Locale {
  let saved: string | null = null;
  try {
    saved = globalThis.localStorage?.getItem(languageKey) ?? null;
  } catch {
    // Storage may be unavailable; use the system preference for this session.
  }
  return resolveLocale(saved, globalThis.navigator?.languages ?? []);
}
