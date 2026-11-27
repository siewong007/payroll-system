import { Link, Navigate, useSearchParams } from 'react-router';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  AlertTriangle,
  ArrowRight,
  Building2,
  Calculator,
  Calendar,
  CheckCircle2,
  ClipboardCheck,
  Clock,
  MapPinOff,
  Receipt,
  ScanLine,
  ShieldCheck,
  UserCircle,
  Banknote,
  UserCheck,
  Users,
} from 'lucide-react';
import { getDashboardSummary } from '@/api/dashboard';
import { useAuth } from '@/context/AuthContext';
import { SUPER_ADMIN_ROLES, hasAnyRole } from '@/lib/roles';
import { userCan } from '@/lib/usePermissions';
import { formatNumber, formatPeriod } from '@/lib/format';
import { formatMYR, getErrorMessage } from '@/lib/utils';
import { PageHeader } from '@/components/ui/PageHeader';
import { KpiSkeletonGrid, Skeleton } from '@/components/ui/Skeleton';
import { EmptyState } from '@/components/ui/EmptyState';

const WINDOW_CHOICES = [7, 14, 30] as const;

function formatDashboardPeriod(rawPeriod: string): string {
  const match = rawPeriod.match(/^(\d{4})-(\d{2})$/);
  if (!match) return rawPeriod;
  return formatPeriod(Number(match[1]), Number(match[2]));
}

export function DashboardPage() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const [searchParams, setSearchParams] = useSearchParams();

  const rawWindow = Number(searchParams.get('window'));
  const windowDays = WINDOW_CHOICES.includes(rawWindow as (typeof WINDOW_CHOICES)[number])
    ? rawWindow
    : 7;

  const canViewPayroll = userCan(user, 'view_payroll');
  const canViewApprovals = userCan(user, 'view_approvals');
  const canViewAttendance = userCan(user, 'view_attendance');
  const canViewEmployees = userCan(user, 'view_employees');
  const unassignedSuperAdmin = Boolean(user) && hasAnyRole(user, SUPER_ADMIN_ROLES) && !user?.company_id;
  const subtitle = canViewPayroll ? t('ux.dashboard.subtitle') : t('ux.dashboard.subtitleNoPayroll');

  const { data, isLoading, isPlaceholderData, error, refetch } = useQuery({
    queryKey: ['dashboard-summary', windowDays],
    queryFn: () => getDashboardSummary(windowDays),
    enabled: !unassignedSuperAdmin,
    placeholderData: keepPreviousData,
  });

  if (unassignedSuperAdmin) {
    return <Navigate to="/companies" replace />;
  }

  const handleWindowChange = (days: number) => {
    const next = new URLSearchParams(searchParams);
    if (days === 7) {
      next.delete('window');
    } else {
      next.set('window', String(days));
    }
    setSearchParams(next, { replace: true });
  };

  if (isLoading) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('ux.dashboard.title')} subtitle={subtitle} />
        <KpiSkeletonGrid count={canViewPayroll ? 4 : 2} />
        <div className="card space-y-4">
          <Skeleton className="h-5 w-44" />
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-16 w-full" />
          </div>
        </div>
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('ux.dashboard.title')} subtitle={subtitle} />
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('ux.dashboard.loadFailed')}
            description={getErrorMessage(error, t('ux.dashboard.loadFailed'))}
            action={
              <button type="button" onClick={() => void refetch()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  const attention = data.needs_attention;
  const ytdEmployerStatutory =
    data.ytd_total_epf_employer + data.ytd_total_socso_employer + data.ytd_total_eis_employer;

  const attentionItems = [
    ...(canViewApprovals
      ? [
          {
            key: 'pendingLeave',
            label: t('ux.dashboard.pendingLeave'),
            count: attention.pending_leave,
            icon: Calendar,
            href: '/approvals?tab=leave',
            tone: 'amber' as const,
          },
          {
            key: 'pendingClaims',
            label: t('ux.dashboard.pendingClaims'),
            count: attention.pending_claims,
            icon: Receipt,
            href: '/approvals?tab=claims',
            tone: 'amber' as const,
          },
          {
            key: 'pendingOvertime',
            label: t('ux.dashboard.pendingOvertime'),
            count: attention.pending_overtime,
            icon: Clock,
            href: '/approvals?tab=overtime',
            tone: 'amber' as const,
          },
        ]
      : []),
    {
      key: 'openSessions',
      label: t('ux.dashboard.openSessions'),
      count: attention.open_session_count,
      icon: ScanLine,
      href: canViewAttendance ? '/attendance?open_only=1' : undefined,
      tone: 'red' as const,
    },
    {
      key: 'absentRecords',
      label: t('ux.dashboard.absentRecords'),
      count: attention.absent_count,
      icon: AlertTriangle,
      href: canViewAttendance ? '/attendance?status=absent' : undefined,
      tone: 'red' as const,
    },
    {
      key: 'lateArrivals',
      label: t('ux.dashboard.lateArrivals'),
      count: attention.late_count,
      icon: Clock,
      href: canViewAttendance ? '/attendance?status=late' : undefined,
      tone: 'accent' as const,
    },
    {
      key: 'outsideGeofence',
      label: t('ux.dashboard.outsideGeofence'),
      count: attention.outside_geofence_count,
      icon: MapPinOff,
      href: canViewAttendance ? '/attendance?outside_geofence=1' : undefined,
      tone: 'amber' as const,
    },
    {
      key: 'manualEntries',
      label: t('ux.dashboard.manualEntries'),
      count: attention.manual_entry_count,
      icon: UserCheck,
      href: canViewAttendance ? '/attendance' : undefined,
      tone: 'accent' as const,
    },
  ].filter((item) => item.count > 0);

  const maxDeptCount = Math.max(1, ...data.departments.map((d) => d.count));

  const shortcuts = [
    canViewEmployees && { label: t('nav.employees'), href: '/employees', icon: Users },
    canViewPayroll && { label: t('nav.payroll'), href: '/payroll', icon: Calculator },
    canViewApprovals && { label: t('nav.approvals'), href: '/approvals', icon: ClipboardCheck },
    canViewAttendance && { label: t('nav.attendance'), href: '/attendance', icon: ScanLine },
    { label: t('nav.company'), href: '/company', icon: Building2 },
    Boolean(user?.employee_id) && { label: t('nav.myPortal'), href: '/portal/profile', icon: UserCircle },
  ].filter(Boolean) as { label: string; href: string; icon: typeof Users }[];

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('ux.dashboard.title')}
        subtitle={subtitle}
        actions={
          <div
            role="group"
            aria-label={t('ux.dashboard.windowGroupLabel')}
            className="tab-group"
          >
            {WINDOW_CHOICES.map((days) => {
              const active = windowDays === days;
              return (
                <button
                  key={days}
                  type="button"
                  onClick={() => handleWindowChange(days)}
                  aria-pressed={active}
                  className={`min-h-11 sm:min-h-9 rounded-lg px-3 py-1.5 text-xs font-semibold transition-all ${
                    active
                      ? 'bg-[var(--card)] text-[var(--foreground)] border border-[var(--border)] shadow-2xs'
                      : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
                  }`}
                >
                  {t('ux.dashboard.windowDays', { days })}
                </button>
              );
            })}
          </div>
        }
      />

      {/* KPI Strip */}
      <div
        aria-busy={isPlaceholderData}
        className={`grid grid-cols-1 gap-4 sm:grid-cols-2 ${canViewPayroll ? 'xl:grid-cols-4' : 'lg:grid-cols-2'} ${
          isPlaceholderData ? 'opacity-75 transition-opacity' : ''
        }`}
      >
        <div className="kpi-card flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
              {t('ux.dashboard.activeHeadcount')}
            </p>
            <p className="mt-1.5 text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
              {formatNumber(data.active_employees)}
            </p>
            <p className="mt-1 text-xs text-gray-500 dark:text-slate-400">
              {t('ux.dashboard.ofTotalHeadcount', { total: formatNumber(data.total_employees) })}
            </p>
          </div>
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent-1)]">
            <Users className="h-5 w-5" aria-hidden="true" />
          </div>
        </div>

        {canViewPayroll ? (
          <>
            <div className="kpi-card flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.latestPayroll')}
                </p>
                <p
                  title={
                    data.last_payroll_total_net !== null
                      ? formatMYR(data.last_payroll_total_net)
                      : undefined
                  }
                  className="mt-1.5 truncate text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100"
                >
                  {data.last_payroll_total_net !== null ? formatMYR(data.last_payroll_total_net) : '—'}
                </p>
                <p className="mt-1 text-xs text-gray-500 dark:text-slate-400">
                  {data.last_payroll_period
                    ? t('ux.dashboard.latestPayrollSub', {
                        period: formatDashboardPeriod(data.last_payroll_period),
                        paidCount: formatNumber(data.last_payroll_employee_count ?? 0),
                      })
                    : t('ux.dashboard.noPayrollYet')}
                </p>
              </div>
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                <Calculator className="h-5 w-5" aria-hidden="true" />
              </div>
            </div>

            <div className="kpi-card flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.ytdGross')}
                </p>
                <p
                  title={formatMYR(data.ytd_total_gross)}
                  className="mt-1.5 truncate text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100"
                >
                  {formatMYR(data.ytd_total_gross)}
                </p>
                <p className="mt-1 text-xs text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.ytdGrossSub')}
                </p>
              </div>
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent-1)]">
                <Banknote className="h-4 w-4" aria-hidden="true" />
              </div>
            </div>

            <div className="kpi-card flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.ytdStatutory')}
                </p>
                <p
                  title={formatMYR(ytdEmployerStatutory)}
                  className="mt-1.5 truncate text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100"
                >
                  {formatMYR(ytdEmployerStatutory)}
                </p>
                <p className="mt-1 text-xs text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.ytdStatutorySub')}
                </p>
              </div>
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[var(--surface-subtle)] border border-[var(--border)] text-[var(--muted-foreground)]">
                <ShieldCheck className="h-5 w-5" aria-hidden="true" />
              </div>
            </div>
          </>
        ) : (
          <div className="kpi-card flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                {t('ux.dashboard.departmentsTitle')}
              </p>
              <p className="mt-1.5 text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
                {formatNumber(data.departments.length)}
              </p>
              <p className="mt-1 text-xs text-gray-500 dark:text-slate-400">
                {t('ux.dashboard.departmentsSub')}
              </p>
            </div>
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent-1)]">
              <Building2 className="h-5 w-5" aria-hidden="true" />
            </div>
          </div>
        )}
      </div>

      {/* Needs Attention Action Queue */}
      <section
        className={`card ${isPlaceholderData ? 'opacity-75 transition-opacity' : ''}`}
        aria-labelledby="needs-attention-heading"
      >
        <div className="mb-4 flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 id="needs-attention-heading" className="section-title">
              {t('ux.dashboard.needsAttentionTitle')}
            </h2>
            <p className="text-xs text-gray-500 dark:text-slate-400">
              {t('ux.dashboard.needsAttentionSub', { days: attention.window_days })}
            </p>
          </div>
        </div>

        {attention.all_clear || attentionItems.length === 0 ? (
          <div className="flex items-center gap-3.5 rounded-xl border border-emerald-200 dark:border-emerald-500/30 bg-emerald-50 dark:bg-emerald-500/10 px-4 py-3.5 text-emerald-900 dark:text-emerald-200">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-emerald-500/15 text-emerald-600 dark:text-emerald-400">
              <CheckCircle2 className="h-5 w-5" aria-hidden="true" />
            </div>
            <div>
              <p className="text-sm font-semibold">{t('ux.dashboard.allClearTitle')}</p>
              <p className="text-xs opacity-85">
                {t('ux.dashboard.allClearBody', { days: attention.window_days })}
              </p>
            </div>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
            {attentionItems.map((item) => {
              const Icon = item.icon;
              const toneClass =
                item.tone === 'red'
                  ? 'border-red-200/70 bg-red-50/70 text-red-900 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-200'
                  : item.tone === 'amber'
                    ? 'border-amber-200/70 bg-amber-50/70 text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200'
                    : 'border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--foreground)]';
              const content = (
                <div
                  className={`flex items-center justify-between gap-3 rounded-lg border px-3.5 py-3 transition-colors ${
                    item.href ? 'hover:bg-[var(--surface-subtle)]' : ''
                  } ${toneClass}`}
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <Icon className="h-5 w-5 shrink-0 opacity-85" aria-hidden="true" />
                    <div className="min-w-0">
                      <p className="truncate text-xs font-medium opacity-85">{item.label}</p>
                      <p className="text-xl font-bold tabular-nums leading-tight">{formatNumber(item.count)}</p>
                    </div>
                  </div>
                  {item.href && <ArrowRight className="h-4 w-4 shrink-0 opacity-60" aria-hidden="true" />}
                </div>
              );
              return item.href ? (
                <Link
                  key={item.key}
                  to={item.href}
                  className="block rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent-1)]"
                >
                  {content}
                </Link>
              ) : (
                <div key={item.key}>{content}</div>
              );
            })}
          </div>
        )}
      </section>

      {/* Department Headcount + Statutory Breakdown / Quick Shortcuts */}
      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        <section className="card xl:col-span-2" aria-labelledby="dept-heading">
          <h2 id="dept-heading" className="section-title mb-4">
            {t('ux.dashboard.departmentsTitle')}
          </h2>
          {data.departments.length === 0 ? (
            <EmptyState compact icon={Users} title={t('ux.dashboard.noDepartments')} />
          ) : (
            <div className="space-y-3">
              {data.departments.map((dept) => {
                const label =
                  dept.department === 'Unassigned' ? t('common.unassigned') : dept.department;
                const widthPct = Math.max(6, Math.round((dept.count / maxDeptCount) * 100));
                return (
                  <div key={dept.department} className="space-y-1">
                    <div className="flex items-center justify-between text-sm">
                      <span className="font-medium text-gray-800 dark:text-slate-200 truncate">
                        {label}
                      </span>
                      <span className="font-semibold tabular-nums text-gray-600 dark:text-slate-300">
                        {formatNumber(dept.count)}
                      </span>
                    </div>
                    <div className="h-2 w-full overflow-hidden rounded-full bg-gray-100 dark:bg-slate-800">
                      <div
                        className="h-full rounded-full bg-[var(--accent-1)] transition-all duration-300"
                        style={{ width: `${widthPct}%` }}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <div className="space-y-6">
          {canViewPayroll && (
            <section className="card" aria-labelledby="statutory-heading">
              <h2 id="statutory-heading" className="section-title mb-4">
                {t('ux.dashboard.statutoryBreakdownTitle')}
              </h2>
              <div className="divide-y divide-gray-100 dark:divide-slate-800 text-sm">
                <div className="flex items-center justify-between py-2.5">
                  <span className="text-gray-600 dark:text-slate-400">{t('ux.dashboard.epfEmployer')}</span>
                  <span className="font-semibold tabular-nums text-gray-900 dark:text-slate-100">
                    {formatMYR(data.ytd_total_epf_employer)}
                  </span>
                </div>
                <div className="flex items-center justify-between py-2.5">
                  <span className="text-gray-600 dark:text-slate-400">{t('ux.dashboard.socsoEmployer')}</span>
                  <span className="font-semibold tabular-nums text-gray-900 dark:text-slate-100">
                    {formatMYR(data.ytd_total_socso_employer)}
                  </span>
                </div>
                <div className="flex items-center justify-between py-2.5">
                  <span className="text-gray-600 dark:text-slate-400">{t('ux.dashboard.eisEmployer')}</span>
                  <span className="font-semibold tabular-nums text-gray-900 dark:text-slate-100">
                    {formatMYR(data.ytd_total_eis_employer)}
                  </span>
                </div>
              </div>
            </section>
          )}

          <section className="card" aria-labelledby="shortcuts-heading">
            <h2 id="shortcuts-heading" className="section-title mb-3">
              {t('ux.dashboard.quickLinksTitle')}
            </h2>
            <div className="grid grid-cols-1 gap-2">
              {shortcuts.map((item) => (
                <Link
                  key={item.href}
                  to={item.href}
                  className="flex min-h-11 sm:min-h-0 items-center justify-between rounded-lg border border-[var(--border)] bg-[var(--card)] px-3.5 py-2.5 text-sm font-medium text-[var(--foreground)] transition-colors hover:bg-[var(--surface-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  <span className="inline-flex items-center gap-2.5">
                    <item.icon className="h-4 w-4 text-[var(--accent-1)]" aria-hidden="true" />
                    {item.label}
                  </span>
                  <ArrowRight className="h-4 w-4 text-gray-500 dark:text-slate-400" aria-hidden="true" />
                </Link>
              ))}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
