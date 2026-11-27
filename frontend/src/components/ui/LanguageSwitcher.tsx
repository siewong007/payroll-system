import { useTranslation } from 'react-i18next';
import { Languages } from 'lucide-react';
import { LOCALE_LABELS, SUPPORTED_LOCALES, type AppLocale } from '@/i18n';

/**
 * Compact icon-only language switcher backed by a visually overlaid native
 * `<select>` so navigation chrome stays minimal while keyboard access, screen
 * readers (`aria-label`), and mobile OS picker sheets continue to work cleanly.
 */
export function LanguageSwitcher({ dark = false }: { dark?: boolean }) {
  const { t, i18n } = useTranslation();
  const resolved = i18n.resolvedLanguage ?? i18n.language;
  const currentLocale: AppLocale = SUPPORTED_LOCALES.includes(resolved as AppLocale)
    ? (resolved as AppLocale)
    : 'en';
  const tooltip = `${t('language.label')}: ${LOCALE_LABELS[currentLocale]}`;

  return (
    <span
      title={tooltip}
      className={`relative inline-flex min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 sm:h-8 sm:w-8 shrink-0 items-center justify-center rounded-md border transition-colors focus-within:ring-2 focus-within:ring-[var(--ring)] ${
        dark
          ? 'border-white/15 bg-white/5 text-zinc-200 hover:bg-white/10'
          : 'border-[var(--border-strong)] bg-[var(--card)] text-[var(--muted-foreground)] hover:border-[var(--muted-foreground)] hover:text-[var(--foreground)]'
      }`}
    >
      <Languages className="h-4 w-4 pointer-events-none" aria-hidden="true" />
      <span className="sr-only">{t('language.label')}</span>
      <select
        aria-label={t('language.label')}
        title={tooltip}
        value={currentLocale}
        onChange={(e) => void i18n.changeLanguage(e.target.value as AppLocale)}
        className="absolute inset-0 h-full w-full cursor-pointer appearance-none opacity-0"
      >
        {SUPPORTED_LOCALES.map((locale) => (
          <option
            key={locale}
            value={locale}
            className="bg-white text-gray-900 dark:bg-zinc-900 dark:text-zinc-100"
          >
            {LOCALE_LABELS[locale]}
          </option>
        ))}
      </select>
    </span>
  );
}
