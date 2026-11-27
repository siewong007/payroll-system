import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';

interface AuthLayoutProps {
  children: ReactNode;
  variant?: 'subtle';
  footerExtra?: ReactNode;
}

/**
 * Shared standalone shell for unauthenticated and recovery flows.
 * Intentionally avoids `useAuth()` so standalone unit tests and pre-session
 * callback/link routes can mount cleanly without an `AuthProvider`.
 */
export function AuthLayout({
  children,
  footerExtra,
}: AuthLayoutProps) {
  const { t } = useTranslation();

  return (
    <div className="relative flex min-h-[100dvh] flex-col justify-between overflow-x-hidden bg-[var(--background)] text-[var(--foreground)] px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-[max(1rem,env(safe-area-inset-bottom))]">
      <header className="mx-auto flex w-full max-w-[400px] items-center justify-end pb-4">
        <LanguageSwitcher />
      </header>

      <main className="flex flex-1 items-center justify-center py-2 sm:py-4">
        <div className="w-full max-w-[400px]">{children}</div>
      </main>

      <footer className="mx-auto w-full max-w-[400px] pt-6 text-center space-y-2">
        {footerExtra}
        <p className="text-xs text-[var(--muted-foreground)]">
          {t('auth.footerTagline')}
        </p>
        <nav
          aria-label={t('legal.header.badge')}
          className="flex flex-wrap items-center justify-center gap-x-2 gap-y-1 text-xs text-[var(--muted-foreground)]"
        >
          <Link
            to="/terms"
            className="inline-flex min-h-11 sm:min-h-0 items-center px-1.5 rounded-md transition-colors hover:text-[var(--foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            {t('legal.links.terms')}
          </Link>
          <span aria-hidden="true" className="text-[var(--border-strong)]">
            ·
          </span>
          <Link
            to="/privacy"
            className="inline-flex min-h-11 sm:min-h-0 items-center px-1.5 rounded-md transition-colors hover:text-[var(--foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            {t('legal.links.privacy')}
          </Link>
          <span aria-hidden="true" className="text-[var(--border-strong)]">
            ·
          </span>
          <Link
            to="/compliance"
            className="inline-flex min-h-11 sm:min-h-0 items-center px-1.5 rounded-md transition-colors hover:text-[var(--foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            {t('legal.links.compliance')}
          </Link>
        </nav>
      </footer>
    </div>
  );
}
