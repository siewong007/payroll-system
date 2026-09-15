import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, ArrowRight, Ban, Info } from 'lucide-react';
import { getPayrollOverview } from '@/api/payroll';
import { formatMYR } from '@/lib/utils';
import { formatPeriod } from '@/lib/format';
import { PAYROLL_STATUS_STYLES, payrollStatusLabel } from './statusMeta';
import type { PayrollActionItem } from '@/types';

const SEVERITY_STYLES: Record<string, { badge: string; icon: typeof AlertTriangle }> = {
  blocking: { badge: 'bg-red-50 text-red-700 border-red-100', icon: Ban },
  warning: { badge: 'bg-amber-50 text-amber-700 border-amber-100', icon: AlertTriangle },
  info: { badge: 'bg-blue-50 text-blue-700 border-blue-100', icon: Info },
};

function KpiCard({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
      <p className="text-xs text-gray-400">{label}</p>
      <p className="text-lg font-bold mt-1 tabular-nums">{value}</p>
      {sub && <p className="text-xs text-gray-400 mt-0.5">{sub}</p>}
    </div>
  );
}

function ActionRow({ item }: { item: PayrollActionItem }) {
  const { t } = useTranslation();
  const style = SEVERITY_STYLES[item.severity] ?? SEVERITY_STYLES.info;
  const Icon = style.icon;
  const inner = (
    <div className={`flex items-start gap-3 rounded-xl border px-4 py-3 ${style.badge}`}>
      <Icon className="w-4 h-4 mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">
            {t(`payroll.actions.${item.code}`, { detail: item.detail, defaultValue: item.message })}
          </span>
          <span className="text-xs font-semibold rounded-full bg-white/70 px-2 py-0.5">
            {item.count}
          </span>
        </div>
        {item.employees.length > 0 && (
          <p className="text-xs mt-1 opacity-80 truncate">
            {item.employees.map((e) => `${e.employee_name} (${e.employee_number})`).join(', ')}
            {item.count > item.employees.length && `, ${t('payroll.overview.moreEmployees', { count: item.count - item.employees.length })}`}
          </p>
        )}
      </div>
      {item.link && <ArrowRight className="w-4 h-4 mt-0.5 shrink-0 opacity-60" />}
    </div>
  );
  return item.link ? <Link to={item.link}>{inner}</Link> : inner;
}

export function PayrollOverview() {
  const { t } = useTranslation();
  const { data, isLoading, error } = useQuery({
    queryKey: ['payrollOverview'],
    queryFn: getPayrollOverview,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  if (error || !data) {
    return <div className="text-center text-gray-500 py-12">{t('payroll.overview.loadFailed')}</div>;
  }

  const current = data.current_period;
  const variance = data.variance;

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">{t('payroll.overview.title')}</h1>
          {current && (
            <p className="text-gray-500 text-sm">
              {current.run_count > 1
                ? t('payroll.overview.latestPeriodRuns', { period: formatPeriod(current.period_year, current.period_month), count: current.run_count })
                : t('payroll.overview.latestPeriod', { period: formatPeriod(current.period_year, current.period_month) })}
            </p>
          )}
        </div>
        <Link
          to="/payroll/runs"
          className="flex items-center justify-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
        >
          {t('payroll.overview.allRuns')} <ArrowRight className="w-4 h-4" />
        </Link>
      </div>

      {!current ? (
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-8 text-center">
          <p className="text-gray-700 font-medium">{t('payroll.overview.emptyTitle')}</p>
          <p className="text-gray-500 text-sm mt-1">
            {t('payroll.overview.emptyBody')}
          </p>
          <Link
            to="/payroll/runs"
            className="inline-flex items-center gap-2 mt-4 text-sm font-medium text-gray-900 underline"
          >
            {t('payroll.overview.goToRuns')} <ArrowRight className="w-4 h-4" />
          </Link>
        </div>
      ) : (
        <>
          {/* KPI cards — all figures committed, never estimated */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
            <KpiCard
              label={t('payroll.overview.kpi.employeesPaid')}
              value={String(current.employee_count)}
              sub={variance && variance.headcount_delta !== 0
                ? t('payroll.overview.vsPrior', {
                    delta: `${variance.headcount_delta > 0 ? '+' : ''}${variance.headcount_delta}`,
                    period: formatPeriod(variance.previous_period_year, variance.previous_period_month),
                  })
                : undefined}
            />
            <KpiCard
              label={t('payroll.overview.kpi.grossPayroll')}
              value={formatMYR(current.total_gross)}
              sub={variance?.gross_change_pct
                ? t('payroll.overview.pctVsPrior', { pct: `${Number(variance.gross_change_pct) > 0 ? '+' : ''}${variance.gross_change_pct}` })
                : undefined}
            />
            <KpiCard
              label={t('payroll.overview.kpi.netPayroll')}
              value={formatMYR(current.total_net)}
              sub={variance?.net_change_pct
                ? t('payroll.overview.pctVsPrior', { pct: `${Number(variance.net_change_pct) > 0 ? '+' : ''}${variance.net_change_pct}` })
                : undefined}
            />
            <KpiCard label={t('payroll.overview.kpi.totalLabourCost')} value={formatMYR(current.total_employer_cost)} />
            <KpiCard label={t('payroll.overview.kpi.overtime')} value={formatMYR(current.total_overtime)} />
            <KpiCard label={t('payroll.overview.kpi.employeeDeductions')} value={formatMYR(current.total_deductions)} />
            <KpiCard
              label={t('payroll.overview.kpi.employerStatutory')}
              value={formatMYR(
                current.total_epf_employer + current.total_socso_employer + current.total_eis_employer,
              )}
            />
            <KpiCard label={t('payroll.overview.kpi.pcbWithheld')} value={formatMYR(current.total_pcb)} />
          </div>
        </>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
        {/* Pipeline */}
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-5">
          <h2 className="text-sm font-semibold text-gray-900 mb-3">{t('payroll.overview.pipelineTitle')}</h2>
          {data.pipeline.length === 0 ? (
            <p className="text-sm text-gray-400">{t('payroll.overview.pipelineEmpty')}</p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {data.pipeline.map((row) => (
                <span
                  key={row.status}
                  className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium ${PAYROLL_STATUS_STYLES[row.status] ?? 'bg-gray-100 text-gray-700'}`}
                >
                  {payrollStatusLabel(row.status)}
                  <span className="font-bold">{row.count}</span>
                </span>
              ))}
            </div>
          )}
          {data.recent_runs.length > 0 && (
            <ul className="mt-4 divide-y divide-gray-100">
              {data.recent_runs.map((run) => (
                <li key={run.id}>
                  <Link
                    to={`/payroll/${run.id}`}
                    className="flex items-center justify-between py-2 text-sm hover:bg-gray-50 -mx-2 px-2 rounded-lg"
                  >
                    <span className="font-medium">
                      {formatPeriod(run.period_year, run.period_month)}
                    </span>
                    <span className="text-gray-500 tabular-nums">{formatMYR(run.total_net)}</span>
                    <span className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${PAYROLL_STATUS_STYLES[run.status] ?? ''}`}>
                      {payrollStatusLabel(run.status)}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Action queue */}
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-5">
          <h2 className="text-sm font-semibold text-gray-900 mb-3">{t('payroll.overview.attentionTitle')}</h2>
          {data.action_queue.length === 0 ? (
            <p className="text-sm text-gray-400">
              {t('payroll.overview.attentionEmpty')}
            </p>
          ) : (
            <div className="space-y-2">
              {data.action_queue.map((item) => (
                <ActionRow key={item.code} item={item} />
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Trend */}
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-5 overflow-x-auto">
          <h2 className="text-sm font-semibold text-gray-900 mb-3">{t('payroll.overview.trendTitle')}</h2>
          {data.trend.length === 0 ? (
            <p className="text-sm text-gray-400">{t('payroll.overview.trendEmpty')}</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-gray-400 border-b border-gray-100">
                  <th className="py-2 font-medium">{t('payroll.columns.period')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.employees')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.gross')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.net')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.employerCost')}</th>
                </tr>
              </thead>
              <tbody>
                {data.trend.map((period) => (
                  <tr
                    key={`${period.period_year}-${period.period_month}`}
                    className="border-b border-gray-50 last:border-0"
                  >
                    <td className="py-2 font-medium">
                      {formatPeriod(period.period_year, period.period_month)}
                    </td>
                    <td className="py-2 text-right tabular-nums">{period.employee_count}</td>
                    <td className="py-2 text-right tabular-nums">{formatMYR(period.total_gross)}</td>
                    <td className="py-2 text-right tabular-nums">{formatMYR(period.total_net)}</td>
                    <td className="py-2 text-right tabular-nums">{formatMYR(period.total_employer_cost)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        {/* Department split */}
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-5 overflow-x-auto">
          <h2 className="text-sm font-semibold text-gray-900 mb-3">
            {t('payroll.overview.deptTitle')}
            {current && (
              <span className="text-gray-400 font-normal">
                {' '}— {formatPeriod(current.period_year, current.period_month)}
              </span>
            )}
          </h2>
          {data.departments.length === 0 ? (
            <p className="text-sm text-gray-400">{t('payroll.overview.deptEmpty')}</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-gray-400 border-b border-gray-100">
                  <th className="py-2 font-medium">{t('employees.fields.department')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.employees')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.gross')}</th>
                  <th className="py-2 font-medium text-right">{t('payroll.columns.employerCost')}</th>
                </tr>
              </thead>
              <tbody>
                {data.departments.map((row) => (
                  <tr key={row.department ?? '—'} className="border-b border-gray-50 last:border-0">
                    <td className="py-2 font-medium">{row.department ?? t('payroll.overview.unassigned')}</td>
                    <td className="py-2 text-right tabular-nums">{row.employee_count}</td>
                    <td className="py-2 text-right tabular-nums">{formatMYR(row.total_gross)}</td>
                    <td className="py-2 text-right tabular-nums">{formatMYR(row.total_employer_cost)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
