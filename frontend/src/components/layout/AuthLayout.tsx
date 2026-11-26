import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { AppearanceSwitcher } from '@/components/ui/AppearanceSwitcher';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';

interface AuthLayoutProps {
  children: ReactNode;
  variant?: 'aurora' | 'subtle';
  footerExtra?: ReactNode;
}

/**
 * Shared standalone shell for unauthenticated and recovery flows.
 * Intentionally avoids `useAuth()` so standalone unit tests and pre-session
 * callback/link routes can mount cleanly without an `AuthProvider`.
 */
export function AuthLayout({
  children,
  variant = 'subtle',
  footerExtra,
}: AuthLayoutProps) {
  const { t } = useTranslation();
  const isAurora = variant === 'aurora';

  return (
    <div
      className={`relative isolate flex min-h-[100dvh] flex-col justify-between overflow-x-hidden px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-[max(1rem,env(safe-area-inset-bottom))] ${
        isAurora
          ? 'bg-slate-950 text-slate-100'
          : 'bg-gray-100 text-gray-900 dark:bg-slate-950 dark:text-slate-100'
      }`}
    >
      {isAurora && (
        <div aria-hidden="true" className="aurora-login pointer-events-none absolute inset-0 -z-10" />
      )}

      <header className="mx-auto flex w-full max-w-md flex-wrap items-center justify-between gap-2 pb-4">
        <div className="min-w-0 flex-1 max-w-[202px]">
          <AppearanceSwitcher darkRail={isAurora} />
        </div>
        <div className="w-[118px] sm:w-[132px] shrink-0">
          <LanguageSwitcher dark={isAurora} />
        </div>
      </header>

      <main className="flex flex-1 items-center justify-center py-2 sm:py-6">
        <div className="animate-page-enter w-full max-w-md">{children}</div>
      </main>

      <footer className="mx-auto w-full max-w-md pt-6 text-center space-y-2">
        {footerExtra}
        <p
          className={`text-xs ${
            isAurora ? 'text-slate-400' : 'text-gray-600 dark:text-slate-400'
          }`}
        >
          {t('auth.footerTagline')}
        </p>
        <nav
          aria-label={t('legal.header.badge')}
          className={`flex flex-wrap items-center justify-center gap-x-2 gap-y-1 text-xs ${
            isAurora ? 'text-slate-300' : 'text-gray-600 dark:text-slate-400'
          }`}
        >
          <Link
            to="/terms"
            className={`inline-flex min-h-11 sm:min-h-0 items-center px-1.5 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              isAurora ? 'hover:text-white' : 'hover:text-gray-900 dark:hover:text-slate-200'
            }`}
          >
            {t('legal.links.terms')}
          </Link>
          <span
            aria-hidden="true"
            className={isAurora ? 'text-slate-600' : 'text-gray-300 dark:text-slate-700'}
          >
            ·
          </span>
          <Link
            to="/privacy"
            className={`inline-flex min-h-11 sm:min-h-0 items-center px-1.5 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              isAurora ? 'hover:text-white' : 'hover:text-gray-900 dark:hover:text-slate-200'
            }`}
          >
            {t('legal.links.privacy')}
          </Link>
          <span
            aria-hidden="true"
            className={isAurora ? 'text-slate-600' : 'text-gray-300 dark:text-slate-700'}
          >
            ·
          </span>
          <Link
            to="/compliance"
            className={`inline-flex min-h-11 sm:min-h-0 items-center px-1.5 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
              isAurora ? 'hover:text-white' : 'hover:text-gray-900 dark:hover:text-slate-200'
            }`}
          >
            {t('legal.links.compliance')}
          </Link>
        </nav>
      </footer>
    </div>
  );
}
