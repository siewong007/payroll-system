import i18n, { type BackendModule, type ResourceLanguage } from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

export const SUPPORTED_LOCALES = ['en', 'ms', 'zh-CN', 'zh-TW'] as const;
export type AppLocale = (typeof SUPPORTED_LOCALES)[number];

export const LOCALE_LABELS: Record<AppLocale, string> = {
  en: 'English',
  ms: 'Bahasa Melayu',
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
};

const LOCALE_LOADERS: Record<AppLocale, () => Promise<{ default: ResourceLanguage }>> = {
  en: () => import('./locales/en') as Promise<{ default: ResourceLanguage }>,
  ms: () => import('./locales/ms') as Promise<{ default: ResourceLanguage }>,
  'zh-CN': () => import('./locales/zh-CN') as Promise<{ default: ResourceLanguage }>,
  'zh-TW': () => import('./locales/zh-TW') as Promise<{ default: ResourceLanguage }>,
};

/**
 * The BCP-47 tag handed to `Intl.*` formatters. `en` resolves to `en-MY` —
 * the product's home market — so dates keep the Malaysian D/M/YYYY shape the
 * UI has always shown rather than the US M/D/YYYY.
 */
export function intlLocale(locale: string = i18n.language): string {
  switch (locale) {
    case 'ms':
      return 'ms-MY';
    case 'zh-CN':
      return 'zh-CN';
    case 'zh-TW':
      return 'zh-TW';
    default:
      return 'en-MY';
  }
}

/**
 * Normalizes whatever the browser reports (navigator.languages, stored value,
 * user agents disagreeing on tags) onto exactly one of SUPPORTED_LOCALES.
 * Chinese is resolved by *script*, not region: any Traditional-codepoint
 * locale (TW/HK/MO/Hant) lands on zh-TW; everything else Chinese on zh-CN.
 */
function normalizeLocale(lng: string): AppLocale {
  const l = lng.toLowerCase();
  if (l.startsWith('zh')) {
    return l.includes('hant') || l.endsWith('-tw') || l.endsWith('-hk') || l.endsWith('-mo')
      ? 'zh-TW'
      : 'zh-CN';
  }
  if (l.startsWith('ms')) return 'ms';
  return 'en';
}

const localeBackend: BackendModule = {
  type: 'backend',
  init() {},
  read(language, namespace, callback) {
    const normalized = normalizeLocale(language);
    if (i18n.hasResourceBundle(normalized, namespace)) {
      callback(null, i18n.getResourceBundle(normalized, namespace) as ResourceLanguage);
      return;
    }
    LOCALE_LOADERS[normalized]()
      .then((mod) => callback(null, mod.default))
      .catch((err: unknown) => {
        if (normalized !== 'en') {
          LOCALE_LOADERS.en()
            .then((mod) => callback(null, mod.default))
            .catch(() => callback(err as Error, false));
          return;
        }
        callback(err as Error, false);
      });
  },
};

export const i18nReady: Promise<unknown> = i18n
  .use(LanguageDetector)
  .use(localeBackend)
  .use(initReactI18next)
  .init({
    supportedLngs: [...SUPPORTED_LOCALES],
    load: 'currentOnly',
    // Every supported locale enforces 100% key parity via TypeScript (`Messages`)
    // and `i18n.test.ts`, so non-English boots do not need to preload `en`.
    fallbackLng: (code: string) =>
      SUPPORTED_LOCALES.includes(code as AppLocale) ? [] : ['en'],
    partialBundledLanguages: true,
    interpolation: { escapeValue: false },
    returnEmptyString: false,
    react: { useSuspense: false },
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'payroll.lang',
      convertDetectedLanguage: normalizeLocale,
    },
  });

export async function ensureLocaleLoaded(lng: string): Promise<void> {
  const normalized = normalizeLocale(lng);
  if (i18n.hasResourceBundle(normalized, 'translation')) return;
  const mod = await LOCALE_LOADERS[normalized]();
  i18n.addResourceBundle(normalized, 'translation', mod.default, true, true);
}

export default i18n;
