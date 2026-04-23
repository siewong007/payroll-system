import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Eye, Trash2 } from 'lucide-react';
import { deletePayrollRun, getPayrollRuns } from '@/api/payroll';
import { formatMYR } from '@/lib/utils';
import { DataTable, type Column } from '@/components/ui/DataTable';
import type { PayrollRun } from '@/types';

const STATUS_STYLES: Record<string, string> = {
  draft: 'bg-gray-100 text-gray-600',
  processing: 'bg-yellow-50 text-yellow-700',
  processed: 'bg-gray-100 text-gray-900',
  approved: 'bg-green-50 text-green-700',
  paid: 'bg-emerald-50 text-emerald-700',
  cancelled: 'bg-red-50 text-red-700',
};

const MONTHS = [
  '', 'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

const canDeletePayrollRun = (run: PayrollRun) => ['draft', 'processed', 'cancelled', 'approved', 'paid'].includes(run.status);

const columns: Column<PayrollRun>[] = [
  {
    key: 'period',
    header: 'Period',
    render: (run) => <span className="font-medium">{MONTHS[run.period_month]} {run.period_year}</span>,
  },
  {
    key: 'employees',
    header: 'Employees',
    align: 'center',
    render: (run) => run.employee_count,
  },
  {
    key: 'gross',
    header: 'Gross',
    align: 'right',
    render: (run) => formatMYR(run.total_gross),
  },
  {
    key: 'net',
    header: 'Net',
    align: 'right',
    render: (run) => <span className="font-medium">{formatMYR(run.total_net)}</span>,
  },
  {
    key: 'employer_cost',
    header: 'Employer Cost',
    align: 'right',
    render: (run) => formatMYR(run.total_employer_cost),
  },
  {
    key: 'status',
    header: 'Status',
    align: 'center',
    render: (run) => (
      <span className={`inline-flex px-2 py-1 rounded-full text-xs font-medium ${STATUS_STYLES[run.status] || ''}`}>
        {run.status}
      </span>
    ),
  },
];

export function PayrollList() {
  const queryClient = useQueryClient();
  const { data: runs, isLoading } = useQuery({
    queryKey: ['payrollRuns'],
    queryFn: getPayrollRuns,
  });

  const deleteMutation = useMutation({
    mutationFn: deletePayrollRun,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['payrollRuns'] }),
    onError: (error: any) => {
      alert(error?.response?.data?.message || error?.message || 'Failed to delete payroll run');
    },
  });

  const handleDelete = (run: PayrollRun) => {
    if (confirm(`Delete ${MONTHS[run.period_month]} ${run.period_year} payroll run? This cannot be undone.`)) {
      deleteMutation.mutate(run.id);
    }
  };

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <h1 className="text-xl sm:text-2xl font-bold text-gray-900">Payroll Runs</h1>
        <Link
          to="/payroll/process"
          className="flex items-center justify-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
        >
          <Plus className="w-4 h-4" />
          Process Payroll
        </Link>
      </div>

      <DataTable
        columns={columns}
        data={runs ?? []}
        perPage={10}
        isLoading={isLoading}
        emptyMessage='No payroll runs yet. Click "Process Payroll" to start.'
        summaryTitle={(run) => `${MONTHS[run.period_month]} ${run.period_year} Payroll`}
        renderSummary={(run) => (
          <div className="space-y-5">
            <div className="flex items-center justify-between">
              <span className="text-lg font-semibold">{MONTHS[run.period_month]} {run.period_year}</span>
              <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-medium ${STATUS_STYLES[run.status] || ''}`}>
                {run.status}
              </span>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <SummaryField label="Employees" value={String(run.employee_count)} />
              <SummaryField label="Gross" value={formatMYR(run.total_gross)} />
              <SummaryField label="Net" value={formatMYR(run.total_net)} />
              <SummaryField label="Employer Cost" value={formatMYR(run.total_employer_cost)} />
              <SummaryField label="EPF (Employee)" value={formatMYR(run.total_epf_employee)} />
              <SummaryField label="EPF (Employer)" value={formatMYR(run.total_epf_employer)} />
              <SummaryField label="SOCSO (Employee)" value={formatMYR(run.total_socso_employee)} />
              <SummaryField label="SOCSO (Employer)" value={formatMYR(run.total_socso_employer)} />
              <SummaryField label="EIS (Employee)" value={formatMYR(run.total_eis_employee)} />
              <SummaryField label="EIS (Employer)" value={formatMYR(run.total_eis_employer)} />
              <SummaryField label="PCB" value={formatMYR(run.total_pcb)} />
              <SummaryField label="Zakat" value={formatMYR(run.total_zakat)} />
            </div>
            {run.notes && (
              <div>
                <p className="text-xs text-gray-400 uppercase tracking-wide">Notes</p>
                <p className="text-sm text-gray-700 mt-0.5">{run.notes}</p>
              </div>
            )}
            <Link
              to={`/payroll/${run.id}`}
              className="flex items-center justify-center gap-1.5 w-full py-2 text-sm font-medium text-gray-900 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors"
            >
              <Eye className="w-4 h-4" /> View Full Details
            </Link>
            {canDeletePayrollRun(run) && (
              <button
                type="button"
                onClick={() => handleDelete(run)}
                disabled={deleteMutation.isPending}
                className="flex items-center justify-center gap-1.5 w-full py-2 text-sm font-medium text-red-600 border border-red-200 rounded-lg hover:bg-red-50 transition-colors disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" /> Delete Payroll Run
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
              <Eye className="w-4 h-4" /> View
            </Link>
            {canDeletePayrollRun(run) && (
              <button
                type="button"
                onClick={() => handleDelete(run)}
                disabled={deleteMutation.isPending}
                className="inline-flex items-center gap-1 text-sm text-red-600 hover:text-red-700 disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" /> Delete
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
