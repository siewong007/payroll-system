import { ArrowLeft, Home, LockKeyhole, SearchX } from 'lucide-react';
import { Link, useLocation, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { useAuth } from '@/context/AuthContext';
import { canUseAdminConsole, defaultAdminHome } from '@/lib/roles';

type ErrorStatus = 403 | 404;

interface ErrorPageProps {
  status: ErrorStatus;
  title: string;
  description: string;
  homePath: string;
  path?: string;
}

const errorStyles = {
  403: {
    icon: LockKeyhole,
    iconClassName: 'bg-amber-50 dark:bg-amber-500/15 text-amber-700 dark:text-amber-300 ring-amber-100 dark:ring-amber-500/20',
  },
  404: {
    icon: SearchX,
    iconClassName: 'bg-blue-50 dark:bg-blue-500/15 text-blue-700 dark:text-blue-300 ring-blue-100 dark:ring-blue-500/20',
  },
} satisfies Record<ErrorStatus, {
  icon: typeof LockKeyhole;
  iconClassName: string;
}>;

export function ErrorPage({ status, title, description, homePath, path }: ErrorPageProps) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { icon: Icon, iconClassName } = errorStyles[status];
  const canGoBack =
    typeof window === 'undefined' || !window.history.state || window.history.state.idx > 0;

  return (
    <main className="relative flex min-h-[100dvh] items-center justify-center overflow-hidden bg-[var(--background)] px-4 pt-[max(3rem,env(safe-area-inset-top))] pb-[max(3rem,env(safe-area-inset-bottom))]">
      <section className="relative w-full max-w-lg text-center" aria-labelledby="error-title">
        <BrandLogo variant="lockup-auto" className="mx-auto mb-8 h-9 w-auto rounded-lg" />

        <div className={`mx-auto mb-6 flex h-12 w-12 items-center justify-center rounded-xl border border-[var(--border)] ${iconClassName}`}>
          <Icon className="h-6 w-6" aria-hidden="true" />
        </div>

        <p className="text-xs font-semibold uppercase tracking-[0.2em] text-[var(--muted-foreground)]">{t('errors.label', { status })}</p>
        <h1 id="error-title" className="mt-3 text-3xl font-bold tracking-tight text-[var(--foreground)] sm:text-4xl">
          {title}
        </h1>
        <p className="mx-auto mt-4 max-w-md text-base leading-7 text-[var(--muted-foreground)]">{description}</p>

        {path && (
          <p className="mx-auto mt-4 max-w-full truncate rounded-lg border border-[var(--border)] bg-[var(--card)] px-3 py-2 font-mono text-xs text-[var(--muted-foreground)]">
            {path}
          </p>
        )}

        <div className="mt-8 flex flex-col-reverse justify-center gap-3 sm:flex-row">
          {canGoBack && (
            <button type="button" onClick={() => navigate(-1)} className="btn-secondary w-full sm:w-auto">
              <ArrowLeft className="h-4 w-4" aria-hidden="true" />
              {t('common.goBack')}
            </button>
          )}
          <Link to={homePath} className="btn-primary w-full sm:w-auto">
            <Home className="h-4 w-4" aria-hidden="true" />
            {t('common.goHome')}
          </Link>
        </div>
      </section>
    </main>
  );
}

function useHomePath() {
  const { user, isLoading } = useAuth();

  if (isLoading || !user) return '/login';
  if (!canUseAdminConsole(user)) return '/portal/profile';
  const home = defaultAdminHome(user);
  return home === '/dashboard' ? '/' : home;
}

export function ForbiddenPage() {
  const location = useLocation();
  const homePath = useHomePath();
  const { t } = useTranslation();
  const from = (location.state as { from?: string } | null)?.from;

  return (
    <ErrorPage
      status={403}
      title={t('errors.forbidden.title')}
      description={t('errors.forbidden.body')}
      homePath={homePath}
      path={from}
    />
  );
}

export function NotFoundPage() {
  const location = useLocation();
  const homePath = useHomePath();
  const { t } = useTranslation();

  return (
    <ErrorPage
      status={404}
      title={t('errors.notFound.title')}
      description={t('errors.notFound.body')}
      homePath={homePath}
      path={location.pathname}
    />
  );
}
