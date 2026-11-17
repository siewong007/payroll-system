import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { ChevronRight } from 'lucide-react';

export interface BreadcrumbItem {
  label: string;
  href?: string;
}

interface PageHeaderProps {
  title: string;
  subtitle?: ReactNode;
  breadcrumbs?: BreadcrumbItem[];
  actions?: ReactNode;
  className?: string;
}

export function PageHeader({
  title,
  subtitle,
  breadcrumbs,
  actions,
  className = 'mb-6',
}: PageHeaderProps) {
  const { t } = useTranslation();
  return (
    <div className={className}>
      {breadcrumbs && breadcrumbs.length > 0 && (
        <nav aria-label={t('ux.commandPalette.groupPages')} className="mb-2 flex flex-wrap items-center gap-1.5 text-xs text-gray-500 dark:text-slate-400">
          {breadcrumbs.map((crumb, index) => (
            <span key={`${crumb.label}-${index}`} className="inline-flex items-center gap-1.5">
              {index > 0 && <ChevronRight className="h-3.5 w-3.5 text-gray-400" aria-hidden="true" />}
              {crumb.href ? (
                <Link
                  to={crumb.href}
                  className="font-medium text-gray-500 transition-colors hover:text-[var(--accent-1)] dark:text-slate-400"
                >
                  {crumb.label}
                </Link>
              ) : (
                <span className="font-semibold text-gray-800 dark:text-slate-200">{crumb.label}</span>
              )}
            </span>
          ))}
        </nav>
      )}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-xl font-bold tracking-tight text-gray-900 dark:text-slate-100 sm:text-2xl">
            {title}
          </h1>
          {subtitle && (
            <div className="mt-1 text-sm text-gray-500 dark:text-slate-400">{subtitle}</div>
          )}
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2 shrink-0">{actions}</div>}
      </div>
    </div>
  );
}
