import { useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Eye, Trash2 } from 'lucide-react';
import { deletePayrollRun, getPayrollGroups, getPayrollRuns } from '@/api/payroll';
import { formatMYR, getErrorMessage } from '@/lib/utils';
import { formatPeriod } from '@/lib/format';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { useAuth } from '@/context/AuthContext';
import { canPreparePayroll } from '@/lib/roles';
import type { PayrollRun } from '@/types';
import { PAYROLL_STATUSES, PAYROLL_STATUS_STYLES, payrollStatusLabel } from './statusMeta';

const canDeletePayrollRun = (run: PayrollRun) => ['draft', 'processed', 'cancelled'].includes(run.status);

const buildColumns = (groupName: (id: string) => string, t: TFunction): Column<PayrollRun>[] => [
  {
    key: 'period',
    header: t('payroll.columns.period'),
    render: (run) => <span className="font-medium">{formatPeriod(run.period_year, run.period_month)}</span>,
  },
  {
    // Without this the period is the only identifier, so two payroll groups
    // processed for the same month render as indistinguishable rows.
    key: 'group',
    header: t('payroll.columns.group'),
    render: (run) => (
      <div>
        <div className="text-sm">{groupName(run.payroll_group_id)}</div>
        {run.version > 1 && <div className="text-xs text-gray-400">{t('payroll.version', { n: run.version })}</div>}
      </div>
    ),
  },
  {
    key: 'employees',
    header: t('payroll.columns.employees'),
    align: 'center',
    render: (run) => run.employee_count,
  },
  {
    key: 'gross',
    header: t('payroll.columns.gross'),
    align: 'right',
    render: (run) => formatMYR(run.total_gross),
  },
  {
    key: 'net',
    header: t('payroll.columns.net'),
    align: 'right',
    render: (run) => <span className="font-medium">{formatMYR(run.total_net)}</span>,
  },
  {
    key: 'employer_cost',
    header: t('payroll.columns.employerCost'),
    align: 'right',
    render: (run) => formatMYR(run.total_employer_cost),
  },
  {
    key: 'status',
    header: t('common.status'),
    align: 'center',
    render: (run) => (
      <span className={`inline-flex px-2 py-1 rounded-full text-xs font-medium ${PAYROLL_STATUS_STYLES[run.status] || ''}`}>
        {payrollStatusLabel(run.status)}
      </span>
    ),
  },
];

export function PayrollList() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const canPrepare = canPreparePayroll(user);
  const [statusFilter, setStatusFilter] = useState('');
  const [yearFilter, setYearFilter] = useState('');
  const [groupFilter, setGroupFilter] = useState('');

  const { data: runs, isLoading } = useQuery({
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
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['payrollRuns'] }),
    onError: (error: unknown) => {
      alert(getErrorMessage(error, t('payroll.deleteFailed')));
    },
  });

  const handleDelete = (run: PayrollRun) => {
    if (confirm(t('payroll.deleteConfirm', { period: formatPeriod(run.period_year, run.period_month) }))) {
      deleteMutation.mutate(run.id);
    }
  };

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <h1 className="text-xl sm:text-2xl font-bold text-gray-900">{t('payroll.listTitle')}</h1>
        {canPrepare && (
          <Link
            to="/payroll/process"
            className="flex items-center justify-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
          >
            <Plus className="w-4 h-4" />
            {t('payroll.processButton')}
          </Link>
        )}
      </div>

      <div className="mb-4 grid grid-cols-1 gap-3 sm:grid-cols-3">
        <select
          value={groupFilter}
          onChange={(e) => setGroupFilter(e.target.value)}
          className="w-full rounded-lg border border-gray-200 p-2 text-sm outline-none focus:ring-1 focus:ring-black"
          aria-label={t('payroll.filterByGroup')}
        >
          <option value="">{t('payroll.allGroups')}</option>
          {groups?.map((group) => (
            <option key={group.id} value={group.id}>{group.name}</option>
          ))}
        </select>
        <select
          value={yearFilter}
          onChange={(e) => setYearFilter(e.target.value)}
          className="w-full rounded-lg border border-gray-200 p-2 text-sm outline-none focus:ring-1 focus:ring-black"
          aria-label={t('payroll.filterByYear')}
        >
          <option value="">{t('payroll.allYears')}</option>
          {years.map((year) => (
            <option key={year} value={year}>{year}</option>
          ))}
        </select>
        <select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value)}
          className="w-full rounded-lg border border-gray-200 p-2 text-sm outline-none focus:ring-1 focus:ring-black"
          aria-label={t('payroll.filterByStatus')}
        >
          <option value="">{t('payroll.allStatuses')}</option>
          {PAYROLL_STATUSES.map((value) => (
            <option key={value} value={value}>{payrollStatusLabel(value)}</option>
          ))}
        </select>
      </div>

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
        summaryTitle={(run) => t('payroll.summaryTitle', { period: formatPeriod(run.period_year, run.period_month) })}
        renderSummary={(run) => (
          <div className="space-y-5">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-lg font-semibold">{formatPeriod(run.period_year, run.period_month)}</span>
                <p className="text-xs text-gray-400">{groupName(run.payroll_group_id)}</p>
              </div>
              <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-medium ${PAYROLL_STATUS_STYLES[run.status] || ''}`}>
                {payrollStatusLabel(run.status)}
              </span>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <SummaryField label={t('payroll.columns.employees')} value={String(run.employee_count)} />
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
                <p className="text-xs text-gray-400 uppercase tracking-wide">{t('common.notes')}</p>
                <p className="text-sm text-gray-700 mt-0.5">{run.notes}</p>
              </div>
            )}
            <Link
              to={`/payroll/${run.id}`}
              className="flex items-center justify-center gap-1.5 w-full py-2 text-sm font-medium text-gray-900 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors"
            >
              <Eye className="w-4 h-4" /> {t('payroll.viewFullDetails')}
            </Link>
            {canPrepare && canDeletePayrollRun(run) && (
              <button
                type="button"
                onClick={() => handleDelete(run)}
                disabled={deleteMutation.isPending}
                className="flex items-center justify-center gap-1.5 w-full py-2 text-sm font-medium text-red-600 border border-red-200 rounded-lg hover:bg-red-50 transition-colors disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" /> {t('payroll.deleteRun')}
              </button>
            )}
          </div>
        )}
        renderActions={(run) => (
          <div className="flex items-center justify-center gap-2">
            <Link
              to={`/payroll/${run.id}`}
              className="inline-flex items-center gap-1 text-sm text-gray-900 hover:text-gray-600"
            >
              <Eye className="w-4 h-4" /> {t('common.view')}
            </Link>
            {canPrepare && canDeletePayrollRun(run) && (
              <button
                type="button"
                onClick={() => handleDelete(run)}
                disabled={deleteMutation.isPending}
                className="inline-flex items-center gap-1 text-sm text-red-600 hover:text-red-700 disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" /> {t('common.delete')}
              </button>
            )}
          </div>
        )}
      />
    </div>
  );
}

function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 mt-0.5">{value}</p>
    </div>
  );
}
