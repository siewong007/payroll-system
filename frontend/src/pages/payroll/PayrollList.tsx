import { Link, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, Eye, Plus, Trash2 } from 'lucide-react';
import { deletePayrollRun, getPayrollGroups, getPayrollRuns } from '@/api/payroll';
import { formatMYR, getErrorMessage } from '@/lib/utils';
import { formatNumber, formatPeriod } from '@/lib/format';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { useConfirm } from '@/lib/confirm';
import { useToast } from '@/lib/toast';
import { usePermissions } from '@/lib/usePermissions';
import type { PayrollRun } from '@/types';
import { PAYROLL_STATUSES, PAYROLL_STATUS_STYLES, payrollStatusLabel } from './statusMeta';

const canDeletePayrollRun = (run: PayrollRun) =>
  ['draft', 'processed'].includes(run.status) && !run.locked_at;

const buildColumns = (groupName: (id: string) => string, t: TFunction): Column<PayrollRun>[] => [
  {
    key: 'period',
    header: t('payroll.columns.period'),
    primary: true,
    sortValue: (run) => run.period_year * 100 + run.period_month,
    render: (run) => (
      <span className="font-medium text-gray-900 dark:text-slate-100 whitespace-nowrap">
        {formatPeriod(run.period_year, run.period_month)}
      </span>
    ),
  },
  {
    // Without this the period is the only identifier, so two payroll groups
    // processed for the same month render as indistinguishable rows.
    key: 'group',
    header: t('payroll.columns.group'),
    sortValue: (run) => groupName(run.payroll_group_id),
    render: (run) => (
      <div>
        <div className="text-sm text-gray-900 dark:text-slate-100">{groupName(run.payroll_group_id)}</div>
        {run.version > 1 && (
          <div className="text-xs text-gray-500 dark:text-slate-400">
            {t('payroll.version', { n: run.version })}
          </div>
        )}
      </div>
    ),
  },
  {
    key: 'employees',
    header: t('payroll.columns.employees'),
    align: 'center',
    sortValue: (run) => run.employee_count,
    render: (run) => <span className="tabular-nums">{formatNumber(run.employee_count)}</span>,
  },
  {
    key: 'gross',
    header: t('payroll.columns.gross'),
    align: 'right',
    sortValue: (run) => run.total_gross,
    render: (run) => <span className="tabular-nums whitespace-nowrap">{formatMYR(run.total_gross)}</span>,
  },
  {
    key: 'net',
    header: t('payroll.columns.net'),
    align: 'right',
    sortValue: (run) => run.total_net,
    render: (run) => (
      <span className="font-medium tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
        {formatMYR(run.total_net)}
      </span>
    ),
  },
  {
    key: 'employer_cost',
    header: t('payroll.columns.employerCost'),
    align: 'right',
    sortValue: (run) => run.total_employer_cost,
    render: (run) => (
      <span className="tabular-nums whitespace-nowrap">{formatMYR(run.total_employer_cost)}</span>
    ),
  },
  {
    key: 'status',
    header: t('common.status'),
    align: 'center',
    sortValue: (run) => payrollStatusLabel(run.status),
    render: (run) => (
      <span className={`inline-flex px-2 py-1 rounded-full text-xs font-medium whitespace-nowrap ${PAYROLL_STATUS_STYLES[run.status] || ''}`}>
        {payrollStatusLabel(run.status)}
      </span>
    ),
  },
];

export function PayrollList() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const confirmDialog = useConfirm();
  const toast = useToast();
  const { can } = usePermissions();
  const canManageDraft = can('manage_payroll_draft');
  const [searchParams, setSearchParams] = useSearchParams();

  const rawStatus = searchParams.get('status') ?? '';
  const statusFilter = PAYROLL_STATUSES.includes(rawStatus as (typeof PAYROLL_STATUSES)[number])
    ? rawStatus
    : '';
  const yearFilter = searchParams.get('year') ?? '';
  const groupFilter = searchParams.get('group') ?? '';

  const updateFilter = (key: 'status' | 'year' | 'group', value: string) => {
    const next = new URLSearchParams(searchParams);
    if (value) {
      next.set(key, value);
    } else {
      next.delete(key);
    }
    setSearchParams(next, { replace: true });
  };

  const hasFilters = Boolean(statusFilter || yearFilter || groupFilter);
  const clearFilters = () => {
    const next = new URLSearchParams(searchParams);
    next.delete('status');
    next.delete('year');
    next.delete('group');
    setSearchParams(next, { replace: true });
  };

  const { data: runs, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['payrollRuns'],
    queryFn: getPayrollRuns,
  });

  const { data: groups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
  });

  const groupName = (id: string) =>
    groups?.find((group) => group.id === id)?.name ?? t('payroll.unknownGroup');
  const columns = buildColumns(groupName, t);

  const years = [...new Set((runs ?? []).map((run) => run.period_year))].sort((a, b) => b - a);
  const visibleRuns = (runs ?? []).filter(
    (run) =>
      (!statusFilter || run.status === statusFilter) &&
      (!yearFilter || run.period_year === Number(yearFilter)) &&
      (!groupFilter || run.payroll_group_id === groupFilter),
  );

  const deleteMutation = useMutation({
    mutationFn: deletePayrollRun,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      queryClient.invalidateQueries({ queryKey: ['payrollOverview'] });
      queryClient.invalidateQueries({ queryKey: ['dashboard-summary'] });
      toast.success(t('payroll.deleteSuccess'));
    },
    onError: (err: unknown) => {
      toast.error(getErrorMessage(err, t('payroll.deleteFailed')));
    },
  });

  const handleDelete = async (run: PayrollRun) => {
    const ok = await confirmDialog({
      description: t('payroll.deleteConfirm', { period: formatPeriod(run.period_year, run.period_month) }),
      variant: 'danger',
    });
    if (ok) {
      deleteMutation.mutate(run.id);
    }
  };

  return (
    <div>
      <PageHeader
        title={t('payroll.listTitle')}
        breadcrumbs={[
          { label: t('nav.payroll'), href: '/payroll' },
          { label: t('payroll.listTitle') },
        ]}
        actions={
          canManageDraft ? (
            <Link to="/payroll/process" className="btn-primary w-full sm:w-auto">
              <Plus className="w-4 h-4" aria-hidden="true" />
              {t('payroll.processButton')}
            </Link>
          ) : undefined
        }
      />

      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="grid flex-1 grid-cols-1 gap-3 sm:grid-cols-3">
          <select
            value={groupFilter}
            onChange={(e) => updateFilter('group', e.target.value)}
            className="form-input"
            aria-label={t('payroll.filterByGroup')}
          >
            <option value="">{t('payroll.allGroups')}</option>
            {groups?.map((group) => (
              <option key={group.id} value={group.id}>{group.name}</option>
            ))}
          </select>
          <select
            value={yearFilter}
            onChange={(e) => updateFilter('year', e.target.value)}
            className="form-input"
            aria-label={t('payroll.filterByYear')}
          >
            <option value="">{t('payroll.allYears')}</option>
            {years.map((year) => (
              <option key={year} value={year}>{year}</option>
            ))}
          </select>
          <select
            value={statusFilter}
            onChange={(e) => updateFilter('status', e.target.value)}
            className="form-input"
            aria-label={t('payroll.filterByStatus')}
          >
            <option value="">{t('payroll.allStatuses')}</option>
            {PAYROLL_STATUSES.map((value) => (
              <option key={value} value={value}>{payrollStatusLabel(value)}</option>
            ))}
          </select>
        </div>
        {hasFilters && (
          <button
            type="button"
            onClick={clearFilters}
            className="btn-secondary min-h-11 sm:min-h-10 shrink-0 whitespace-nowrap"
          >
            {t('ux.table.clearFilters')}
          </button>
        )}
      </div>

      {isError ? (
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('payroll.listLoadFailed')}
            description={getErrorMessage(error, t('payroll.listLoadFailed'))}
            action={
              <button type="button" onClick={() => void refetch()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      ) : (
        <DataTable
          columns={columns}
          data={visibleRuns}
          perPage={10}
          isLoading={isLoading}
          emptyMessage={
            runs?.length
              ? t('payroll.emptyFiltered')
              : t('payroll.emptyList')
          }
          emptyAction={
            hasFilters ? (
              <button type="button" onClick={clearFilters} className="btn-secondary">
                {t('ux.table.clearFilters')}
              </button>
            ) : undefined
          }
          summaryTitle={(run) => t('payroll.summaryTitle', { period: formatPeriod(run.period_year, run.period_month) })}
          renderSummary={(run) => (
            <div className="space-y-5">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <span className="text-lg font-semibold text-gray-900 dark:text-slate-100">
                    {formatPeriod(run.period_year, run.period_month)}
                  </span>
                  <p className="text-xs text-gray-500 dark:text-slate-400">{groupName(run.payroll_group_id)}</p>
                </div>
                <span className={`inline-flex shrink-0 px-2.5 py-1 rounded-full text-xs font-medium ${PAYROLL_STATUS_STYLES[run.status] || ''}`}>
                  {payrollStatusLabel(run.status)}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label={t('payroll.columns.employees')} value={formatNumber(run.employee_count)} />
                <SummaryField label={t('payroll.columns.gross')} value={formatMYR(run.total_gross)} />
                <SummaryField label={t('payroll.columns.net')} value={formatMYR(run.total_net)} />
                <SummaryField label={t('payroll.columns.employerCost')} value={formatMYR(run.total_employer_cost)} />
                <SummaryField label={t('payroll.fields.epfEmployee')} value={formatMYR(run.total_epf_employee)} />
                <SummaryField label={t('payroll.fields.epfEmployer')} value={formatMYR(run.total_epf_employer)} />
                <SummaryField label={t('payroll.fields.socsoEmployee')} value={formatMYR(run.total_socso_employee)} />
                <SummaryField label={t('payroll.fields.socsoEmployer')} value={formatMYR(run.total_socso_employer)} />
                <SummaryField label={t('payroll.fields.eisEmployee')} value={formatMYR(run.total_eis_employee)} />
                <SummaryField label={t('payroll.fields.eisEmployer')} value={formatMYR(run.total_eis_employer)} />
                <SummaryField label={t('payroll.fields.pcb')} value={formatMYR(run.total_pcb)} />
                <SummaryField label={t('payroll.fields.zakat')} value={formatMYR(run.total_zakat)} />
              </div>
              {run.notes && (
                <div>
                  <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{t('common.notes')}</p>
                  <p className="text-sm text-gray-700 dark:text-slate-300 mt-0.5">{run.notes}</p>
                </div>
              )}
              <Link
                to={`/payroll/${run.id}`}
                className="flex min-h-11 items-center justify-center gap-1.5 w-full py-2 text-sm font-medium text-gray-900 dark:text-slate-100 border border-gray-200 dark:border-slate-700 rounded-lg hover:bg-gray-50 dark:hover:bg-slate-800 transition-colors"
              >
                <Eye className="w-4 h-4" aria-hidden="true" /> {t('payroll.viewFullDetails')}
              </Link>
              {canManageDraft && canDeletePayrollRun(run) && (
                <button
                  type="button"
                  onClick={() => handleDelete(run)}
                  disabled={deleteMutation.isPending}
                  className="flex min-h-11 items-center justify-center gap-1.5 w-full py-2 text-sm font-medium text-red-600 dark:text-red-400 border border-red-200 dark:border-red-900/60 rounded-lg hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors disabled:opacity-50"
                >
                  <Trash2 className="w-4 h-4" aria-hidden="true" /> {t('payroll.deleteRun')}
                </button>
              )}
            </div>
          )}
          renderActions={(run) => (
            <div className="flex items-center justify-center gap-1">
              <Link
                to={`/payroll/${run.id}`}
                className="inline-flex min-h-11 sm:min-h-9 items-center gap-1 px-2 rounded-lg text-sm text-gray-900 dark:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors"
              >
                <Eye className="w-4 h-4" aria-hidden="true" /> {t('common.view')}
              </Link>
              {canManageDraft && canDeletePayrollRun(run) && (
                <button
                  type="button"
                  onClick={() => handleDelete(run)}
                  disabled={deleteMutation.isPending}
                  className="inline-flex min-h-11 sm:min-h-9 items-center gap-1 px-2 rounded-lg text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 disabled:opacity-50 transition-colors"
                >
                  <Trash2 className="w-4 h-4" aria-hidden="true" /> {t('common.delete')}
                </button>
              )}
            </div>
          )}
        />
      )}
    </div>
  );
}

function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 mt-0.5 tabular-nums">{value}</p>
    </div>
  );
}
