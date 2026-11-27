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
  className = 'mb-4 sm:mb-5',
}: PageHeaderProps) {
  const { t } = useTranslation();
  return (
    <div className={className}>
      {breadcrumbs && breadcrumbs.length > 0 && (
        <nav aria-label={t('ux.commandPalette.groupPages')} className="mb-1.5 flex flex-wrap items-center gap-1 text-xs text-[var(--muted-foreground)]">
          {breadcrumbs.map((crumb, index) => (
            <span key={`${crumb.label}-${index}`} className="inline-flex items-center gap-1">
              {index > 0 && <ChevronRight className="h-3.5 w-3.5 text-[var(--subtle-foreground)]" aria-hidden="true" />}
              {crumb.href ? (
                <Link
                  to={crumb.href}
                  className="font-medium text-[var(--muted-foreground)] transition-colors hover:text-[var(--foreground)]"
                >
                  {crumb.label}
                </Link>
              ) : (
                <span className="font-medium text-[var(--foreground)]">{crumb.label}</span>
              )}
            </span>
          ))}
        </nav>
      )}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="page-title">{title}</h1>
          {subtitle && (
            <div className="page-subtitle">{subtitle}</div>
          )}
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2 shrink-0">{actions}</div>}
      </div>
    </div>
  );
}
