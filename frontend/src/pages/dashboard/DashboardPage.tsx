import { Link, Navigate, useSearchParams } from 'react-router';
import { useQuery } from '@tanstack/react-query';
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
  Sparkles,
  UserCheck,
  Users,
} from 'lucide-react';
import { getDashboardSummary } from '@/api/dashboard';
import { useAuth } from '@/context/AuthContext';
import { SUPER_ADMIN_ROLES, hasAnyRole } from '@/lib/roles';
import { userCan } from '@/lib/usePermissions';
import { formatMYR, getErrorMessage } from '@/lib/utils';
import { PageHeader } from '@/components/ui/PageHeader';
import { KpiSkeletonGrid, Skeleton } from '@/components/ui/Skeleton';
import { EmptyState } from '@/components/ui/EmptyState';

const WINDOW_CHOICES = [7, 14, 30] as const;

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

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['dashboard-summary', windowDays],
    queryFn: () => getDashboardSummary(windowDays),
    enabled: !unassignedSuperAdmin,
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
        <PageHeader title={t('ux.dashboard.title')} subtitle={t('ux.dashboard.subtitle')} />
        <KpiSkeletonGrid count={canViewPayroll ? 4 : 2} />
        <div className="card space-y-4">
          <Skeleton className="h-5 w-44" />
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
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
        <PageHeader title={t('ux.dashboard.title')} subtitle={t('ux.dashboard.subtitle')} />
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
    {
      key: 'pendingLeave',
      label: t('ux.dashboard.pendingLeave'),
      count: attention.pending_leave,
      icon: Calendar,
      href: canViewApprovals ? '/approvals?tab=leave' : undefined,
      tone: 'amber' as const,
    },
    {
      key: 'pendingClaims',
      label: t('ux.dashboard.pendingClaims'),
      count: attention.pending_claims,
      icon: Receipt,
      href: canViewApprovals ? '/approvals?tab=claims' : undefined,
      tone: 'amber' as const,
    },
    {
      key: 'pendingOvertime',
      label: t('ux.dashboard.pendingOvertime'),
      count: attention.pending_overtime,
      icon: Clock,
      href: canViewApprovals ? '/approvals?tab=overtime' : undefined,
      tone: 'amber' as const,
    },
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
      tone: 'indigo' as const,
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
      tone: 'indigo' as const,
    },
  ].filter((item) => item.count > 0);

  const maxDeptCount = Math.max(1, ...data.departments.map((d) => d.count));

  const shortcuts = [
    canViewEmployees && { label: t('nav.employees'), href: '/employees', icon: Users },
    canViewPayroll && { label: t('nav.payroll'), href: '/payroll', icon: Calculator },
    canViewApprovals && { label: t('nav.approvals'), href: '/approvals', icon: ClipboardCheck },
    canViewAttendance && { label: t('nav.attendance'), href: '/attendance', icon: ScanLine },
    { label: t('nav.company'), href: '/company', icon: Building2 },
    Boolean(user?.employee_id) && { label: t('nav.myPortal'), href: '/portal/profile', icon: Sparkles },
  ].filter(Boolean) as { label: string; href: string; icon: typeof Users }[];

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('ux.dashboard.title')}
        subtitle={t('ux.dashboard.subtitle')}
        actions={
          <div className="inline-flex items-center rounded-xl bg-white/80 dark:bg-slate-900/80 p-1 ring-1 ring-gray-200/80 dark:ring-slate-800 shadow-xs">
            {WINDOW_CHOICES.map((days) => {
              const active = windowDays === days;
              return (
                <button
                  key={days}
                  type="button"
                  onClick={() => handleWindowChange(days)}
                  aria-pressed={active}
                  className={`rounded-lg px-3 py-1.5 text-xs font-semibold transition-all ${
                    active
                      ? 'bg-gradient-to-br from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-sm'
                      : 'text-gray-600 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100'
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
      <div className={`grid grid-cols-1 gap-4 sm:grid-cols-2 ${canViewPayroll ? 'lg:grid-cols-4' : 'lg:grid-cols-2'}`}>
        <div className="card hover-lift flex items-start justify-between gap-3">
          <div>
            <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
              {t('ux.dashboard.activeHeadcount')}
            </p>
            <p className="mt-1.5 text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
              {data.active_employees}
            </p>
            <p className="mt-1 text-xs text-gray-400 dark:text-slate-500">
              {t('ux.dashboard.ofTotalHeadcount', { total: data.total_employees })}
            </p>
          </div>
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent-1)]">
            <Users className="h-5 w-5" aria-hidden="true" />
          </div>
        </div>

        {canViewPayroll ? (
          <>
            <div className="card hover-lift flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.latestPayroll')}
                </p>
                <p className="mt-1.5 truncate text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
                  {data.last_payroll_total_net !== null ? formatMYR(data.last_payroll_total_net) : '—'}
                </p>
                <p className="mt-1 text-xs text-gray-400 dark:text-slate-500">
                  {data.last_payroll_period
                    ? t('ux.dashboard.latestPayrollSub', {
                        period: data.last_payroll_period,
                        count: data.last_payroll_employee_count ?? 0,
                      })
                    : t('ux.dashboard.noPayrollYet')}
                </p>
              </div>
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                <Calculator className="h-5 w-5" aria-hidden="true" />
              </div>
            </div>

            <div className="card hover-lift flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.ytdGross')}
                </p>
                <p className="mt-1.5 truncate text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
                  {formatMYR(data.ytd_total_gross)}
                </p>
                <p className="mt-1 text-xs text-gray-400 dark:text-slate-500">
                  {t('ux.dashboard.ytdGrossSub')}
                </p>
              </div>
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-600 dark:text-indigo-400">
                <Sparkles className="h-5 w-5" aria-hidden="true" />
              </div>
            </div>

            <div className="card hover-lift flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                  {t('ux.dashboard.ytdStatutory')}
                </p>
                <p className="mt-1.5 truncate text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
                  {formatMYR(ytdEmployerStatutory)}
                </p>
                <p className="mt-1 text-xs text-gray-400 dark:text-slate-500">
                  {t('ux.dashboard.ytdStatutorySub')}
                </p>
              </div>
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-violet-500/10 text-violet-600 dark:text-violet-400">
                <ShieldCheck className="h-5 w-5" aria-hidden="true" />
              </div>
            </div>
          </>
        ) : (
          <div className="card hover-lift flex items-start justify-between gap-3">
            <div>
              <p className="text-xs font-medium text-gray-500 dark:text-slate-400">
                {t('ux.dashboard.departmentsTitle')}
              </p>
              <p className="mt-1.5 text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">
                {data.departments.length}
              </p>
              <p className="mt-1 text-xs text-gray-400 dark:text-slate-500">
                {t('ux.dashboard.windowDays', { days: attention.window_days })}
              </p>
            </div>
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent-1)]">
              <Building2 className="h-5 w-5" aria-hidden="true" />
            </div>
          </div>
        )}
      </div>

      {/* Needs Attention Action Queue */}
      <section className="card" aria-labelledby="needs-attention-heading">
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
          <div className="flex items-center gap-3.5 rounded-2xl border border-emerald-200/70 bg-emerald-50/70 px-4 py-4 text-emerald-900 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-200">
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
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {attentionItems.map((item) => {
              const Icon = item.icon;
              const toneClass =
                item.tone === 'red'
                  ? 'border-red-200/70 bg-red-50/70 text-red-900 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-200'
                  : item.tone === 'amber'
                    ? 'border-amber-200/70 bg-amber-50/70 text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200'
                    : 'border-indigo-200/70 bg-indigo-50/70 text-indigo-900 dark:border-indigo-500/30 dark:bg-indigo-500/10 dark:text-indigo-200';
              const content = (
                <div className={`flex items-center justify-between gap-3 rounded-2xl border px-4 py-3.5 transition-all hover:-translate-y-0.5 ${toneClass}`}>
                  <div className="flex items-center gap-3 min-w-0">
                    <Icon className="h-5 w-5 shrink-0 opacity-85" aria-hidden="true" />
                    <div className="min-w-0">
                      <p className="truncate text-xs font-medium opacity-85">{item.label}</p>
                      <p className="text-xl font-bold tabular-nums leading-tight">{item.count}</p>
                    </div>
                  </div>
                  {item.href && <ArrowRight className="h-4 w-4 shrink-0 opacity-60" aria-hidden="true" />}
                </div>
              );
              return item.href ? (
                <Link key={item.key} to={item.href}>
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
      <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
        <section className="card lg:col-span-2" aria-labelledby="dept-heading">
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
                        {dept.count}
                      </span>
                    </div>
                    <div className="h-2 w-full overflow-hidden rounded-full bg-gray-100 dark:bg-slate-800">
                      <div
                        className="h-full rounded-full bg-gradient-to-r from-[var(--accent-1)] to-[var(--accent-2)] transition-all duration-500"
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
                  className="flex items-center justify-between rounded-xl border border-gray-100 dark:border-slate-800 bg-white/60 dark:bg-slate-900/50 px-3.5 py-2.5 text-sm font-medium text-gray-700 dark:text-slate-200 transition-all hover:border-[var(--accent-1)]/40 hover:bg-[var(--accent-soft)]"
                >
                  <span className="inline-flex items-center gap-2.5">
                    <item.icon className="h-4 w-4 text-[var(--accent-1)]" aria-hidden="true" />
                    {item.label}
                  </span>
                  <ArrowRight className="h-4 w-4 text-gray-400" aria-hidden="true" />
                </Link>
              ))}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
