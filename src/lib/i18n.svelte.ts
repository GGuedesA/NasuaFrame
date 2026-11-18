import en from "#lib/locales/en.json";
import pt from "#lib/locales/pt.json";

export type Key = keyof typeof en;

const locales: Record<string, Record<Key, string>> = { en, pt };

let language = $state("en");

export function setLanguage(code: string) {
  language = code;
  document.documentElement.lang = code;
}

/** `{name}` in the text is replaced by `values.name` */
export function t(key: Key, values: Record<string, string | number> = {}): string {
  const text = locales[language]?.[key] ?? en[key];
  return text.replace(/\{(\w+)\}/g, (match, name: string) => String(values[name] ?? match));
}
