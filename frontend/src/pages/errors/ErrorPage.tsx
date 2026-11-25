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

  return (
    <main className="relative flex min-h-screen items-center justify-center overflow-hidden bg-gray-50 dark:bg-slate-950 px-4 py-12">
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-gray-300 dark:via-slate-700 to-transparent" />
      <div className="absolute -left-32 top-20 h-72 w-72 rounded-full bg-gray-200/50 dark:bg-indigo-500/10 blur-3xl" />
      <div className="absolute -right-32 bottom-20 h-72 w-72 rounded-full bg-gray-200/50 dark:bg-teal-500/10 blur-3xl" />

      <section className="relative w-full max-w-lg text-center" aria-labelledby="error-title">
        <BrandLogo variant="lockup-dark" className="mx-auto mb-10 h-10 w-auto rounded-xl" />

        <div className={`mx-auto mb-6 flex h-14 w-14 items-center justify-center rounded-2xl ring-8 ${iconClassName}`}>
          <Icon className="h-7 w-7" aria-hidden="true" />
        </div>

        <p className="text-sm font-semibold tracking-[0.2em] text-gray-500 dark:text-slate-400">{t('errors.label', { status })}</p>
        <h1 id="error-title" className="mt-3 text-3xl font-bold tracking-tight text-gray-950 dark:text-slate-50 sm:text-4xl">
          {title}
        </h1>
        <p className="mx-auto mt-4 max-w-md text-base leading-7 text-gray-600 dark:text-slate-300">{description}</p>

        {path && (
          <p className="mx-auto mt-4 max-w-full truncate rounded-lg border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 px-3 py-2 font-mono text-xs text-gray-500 dark:text-slate-400">
            {path}
          </p>
        )}

        <div className="mt-8 flex flex-col-reverse justify-center gap-3 sm:flex-row">
          <button type="button" onClick={() => navigate(-1)} className="btn-secondary">
            <ArrowLeft className="h-4 w-4" aria-hidden="true" />
            {t('common.goBack')}
          </button>
          <Link to={homePath} className="btn-primary">
            <Home className="h-4 w-4" aria-hidden="true" />
            {t('common.goHome')}
          </Link>
        </div>
      </section>
    </main>
  );
}

function useHomePath() {
  const { user } = useAuth();

  if (!user) return '/login';
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
