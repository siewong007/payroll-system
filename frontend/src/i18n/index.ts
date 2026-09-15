import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import en from './locales/en';
import ms from './locales/ms';
import zhCN from './locales/zh-CN';
import zhTW from './locales/zh-TW';

export const SUPPORTED_LOCALES = ['en', 'ms', 'zh-CN', 'zh-TW'] as const;
export type AppLocale = (typeof SUPPORTED_LOCALES)[number];

export const LOCALE_LABELS: Record<AppLocale, string> = {
  en: 'English',
  ms: 'Bahasa Melayu',
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
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
function normalizeLocale(lng: string): string {
  const l = lng.toLowerCase();
  if (l.startsWith('zh')) {
    return l.includes('hant') || l.endsWith('-tw') || l.endsWith('-hk') || l.endsWith('-mo')
      ? 'zh-TW'
      : 'zh-CN';
  }
  if (l.startsWith('ms')) return 'ms';
  return 'en';
}

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      ms: { translation: ms },
      'zh-CN': { translation: zhCN },
      'zh-TW': { translation: zhTW },
    },
    supportedLngs: [...SUPPORTED_LOCALES],
    fallbackLng: 'en',
    interpolation: { escapeValue: false },
    returnEmptyString: false,
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'payroll.lang',
      convertDetectedLanguage: normalizeLocale,
    },
  });

export default i18n;
