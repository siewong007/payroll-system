import { Link } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, ArrowRight, Ban, Info } from 'lucide-react';
import { getPayrollOverview } from '@/api/payroll';
import { formatMYR } from '@/lib/utils';
import { PAYROLL_STATUS_LABELS, PAYROLL_STATUS_STYLES } from './statusMeta';
import type { PayrollActionItem } from '@/types';

const MONTHS_SHORT = [
  '', 'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
];

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
  const style = SEVERITY_STYLES[item.severity] ?? SEVERITY_STYLES.info;
  const Icon = style.icon;
  const inner = (
    <div className={`flex items-start gap-3 rounded-xl border px-4 py-3 ${style.badge}`}>
      <Icon className="w-4 h-4 mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{item.message}</span>
          <span className="text-xs font-semibold rounded-full bg-white/70 px-2 py-0.5">
            {item.count}
          </span>
        </div>
        {item.employees.length > 0 && (
          <p className="text-xs mt-1 opacity-80 truncate">
            {item.employees.map((e) => `${e.employee_name} (${e.employee_number})`).join(', ')}
            {item.count > item.employees.length && `, +${item.count - item.employees.length} more`}
          </p>
        )}
      </div>
      {item.link && <ArrowRight className="w-4 h-4 mt-0.5 shrink-0 opacity-60" />}
    </div>
  );
  return item.link ? <Link to={item.link}>{inner}</Link> : inner;
}

export function PayrollOverview() {
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
    return <div className="text-center text-gray-500 py-12">Could not load the payroll overview.</div>;
  }

  const current = data.current_period;
  const variance = data.variance;

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">Payroll Overview</h1>
          {current && (
            <p className="text-gray-500 text-sm">
              Latest committed period: {MONTHS_SHORT[current.period_month]} {current.period_year}
              {current.run_count > 1 && ` across ${current.run_count} runs`}
            </p>
          )}
        </div>
        <Link
          to="/payroll/runs"
          className="flex items-center justify-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
        >
          All payroll runs <ArrowRight className="w-4 h-4" />
        </Link>
      </div>

      {!current ? (
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-8 text-center">
          <p className="text-gray-700 font-medium">No committed payroll yet</p>
          <p className="text-gray-500 text-sm mt-1">
            Process a payroll run to see totals, trends and labour cost here. Draft and processing
            runs never contribute to these figures.
          </p>
          <Link
            to="/payroll/runs"
            className="inline-flex items-center gap-2 mt-4 text-sm font-medium text-gray-900 underline"
          >
            Go to payroll runs <ArrowRight className="w-4 h-4" />
          </Link>
        </div>
      ) : (
        <>
          {/* KPI cards — all figures committed, never estimated */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
            <KpiCard
              label="Employees paid"
              value={String(current.employee_count)}
              sub={variance && variance.headcount_delta !== 0
                ? `${variance.headcount_delta > 0 ? '+' : ''}${variance.headcount_delta} vs ${MONTHS_SHORT[variance.previous_period_month]} ${variance.previous_period_year}`
                : undefined}
            />
            <KpiCard
              label="Gross payroll"
              value={formatMYR(current.total_gross)}
              sub={variance?.gross_change_pct
                ? `${Number(variance.gross_change_pct) > 0 ? '+' : ''}${variance.gross_change_pct}% vs prior period`
                : undefined}
            />
            <KpiCard
              label="Net payroll"
              value={formatMYR(current.total_net)}
              sub={variance?.net_change_pct
                ? `${Number(variance.net_change_pct) > 0 ? '+' : ''}${variance.net_change_pct}% vs prior period`
                : undefined}
            />
            <KpiCard label="Total labour cost" value={formatMYR(current.total_employer_cost)} />
            <KpiCard label="Overtime" value={formatMYR(current.total_overtime)} />
            <KpiCard label="Employee deductions" value={formatMYR(current.total_deductions)} />
            <KpiCard
              label="Employer statutory"
              value={formatMYR(
                current.total_epf_employer + current.total_socso_employer + current.total_eis_employer,
              )}
            />
            <KpiCard label="PCB withheld" value={formatMYR(current.total_pcb)} />
          </div>
        </>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-6">
        {/* Pipeline */}
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-5">
          <h2 className="text-sm font-semibold text-gray-900 mb-3">Run pipeline</h2>
          {data.pipeline.length === 0 ? (
            <p className="text-sm text-gray-400">No payroll runs yet.</p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {data.pipeline.map((row) => (
                <span
                  key={row.status}
                  className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium ${PAYROLL_STATUS_STYLES[row.status] ?? 'bg-gray-100 text-gray-700'}`}
                >
                  {PAYROLL_STATUS_LABELS[row.status] ?? row.status}
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
                      {MONTHS_SHORT[run.period_month]} {run.period_year}
                    </span>
                    <span className="text-gray-500 tabular-nums">{formatMYR(run.total_net)}</span>
                    <span className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${PAYROLL_STATUS_STYLES[run.status] ?? ''}`}>
                      {PAYROLL_STATUS_LABELS[run.status] ?? run.status}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Action queue */}
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-5">
          <h2 className="text-sm font-semibold text-gray-900 mb-3">Needs attention</h2>
          {data.action_queue.length === 0 ? (
            <p className="text-sm text-gray-400">
              Nothing outstanding — no blocking data issues and no runs waiting on a lifecycle step.
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
          <h2 className="text-sm font-semibold text-gray-900 mb-3">Payroll trend</h2>
          {data.trend.length === 0 ? (
            <p className="text-sm text-gray-400">No committed periods to chart.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-gray-400 border-b border-gray-100">
                  <th className="py-2 font-medium">Period</th>
                  <th className="py-2 font-medium text-right">Employees</th>
                  <th className="py-2 font-medium text-right">Gross</th>
                  <th className="py-2 font-medium text-right">Net</th>
                  <th className="py-2 font-medium text-right">Employer cost</th>
                </tr>
              </thead>
              <tbody>
                {data.trend.map((period) => (
                  <tr
                    key={`${period.period_year}-${period.period_month}`}
                    className="border-b border-gray-50 last:border-0"
                  >
                    <td className="py-2 font-medium">
                      {MONTHS_SHORT[period.period_month]} {period.period_year}
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
            Labour cost by department
            {current && (
              <span className="text-gray-400 font-normal">
                {' '}— {MONTHS_SHORT[current.period_month]} {current.period_year}
              </span>
            )}
          </h2>
          {data.departments.length === 0 ? (
            <p className="text-sm text-gray-400">No department breakdown for this period.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-gray-400 border-b border-gray-100">
                  <th className="py-2 font-medium">Department</th>
                  <th className="py-2 font-medium text-right">Employees</th>
                  <th className="py-2 font-medium text-right">Gross</th>
                  <th className="py-2 font-medium text-right">Employer cost</th>
                </tr>
              </thead>
              <tbody>
                {data.departments.map((row) => (
                  <tr key={row.department ?? '—'} className="border-b border-gray-50 last:border-0">
                    <td className="py-2 font-medium">{row.department ?? 'Unassigned'}</td>
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
