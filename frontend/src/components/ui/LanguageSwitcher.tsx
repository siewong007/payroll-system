import { useTranslation } from 'react-i18next';
import { Languages } from 'lucide-react';
import { LOCALE_LABELS, SUPPORTED_LOCALES, type AppLocale } from '@/i18n';

/**
 * Native <select> on purpose: it is keyboard-accessible, needs no popover
 * positioning inside a scrollable sidebar, and the mobile bottom-bar layouts
 * get an OS picker sheet for free. `changeLanguage` only swaps resources —
 * routes, forms and query caches are untouched, so no state is lost.
 */
export function LanguageSwitcher({ dark = false }: { dark?: boolean }) {
  const { t, i18n } = useTranslation();
  return (
    <label
      className={`flex items-center gap-2 text-xs ${
        dark ? 'text-zinc-400' : 'text-[var(--muted-foreground)]'
      }`}
    >
      <Languages className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
      <span className="sr-only">{t('language.label')}</span>
      <select
        aria-label={t('language.label')}
        value={i18n.language}
        onChange={(e) => i18n.changeLanguage(e.target.value as AppLocale)}
        className={`w-full min-w-0 rounded-md border px-2 py-1 text-xs font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-[var(--ring)] ${
          dark
            ? 'border-white/15 bg-white/5 text-zinc-200 hover:bg-white/10 [&>option]:bg-zinc-900'
            : 'border-[var(--border-strong)] bg-[var(--card)] text-[var(--foreground)] hover:border-[var(--muted-foreground)] dark:[&>option]:bg-zinc-900'
        }`}
      >
        {SUPPORTED_LOCALES.map((locale) => (
          <option key={locale} value={locale}>
            {LOCALE_LABELS[locale]}
          </option>
        ))}
      </select>
    </label>
  );
}
