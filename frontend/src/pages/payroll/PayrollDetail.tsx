import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, CheckCircle, Lock, Download, Trash2, Pencil, Save, X } from 'lucide-react';
import {
  getPayrollRun,
  approvePayroll,
  lockPayroll,
  downloadRunPayslips,
  deletePayrollRun,
  updatePayrollItemPcb,
} from '@/api/payroll';
import { formatMYR, getErrorMessage } from '@/lib/utils';

const MONTHS = [
  '', 'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

const canDeletePayrollRun = (status: string) => ['draft', 'processed', 'cancelled', 'approved', 'paid'].includes(status);

export function PayrollDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [editingPcbEmployeeId, setEditingPcbEmployeeId] = useState<string | null>(null);
  const [pcbInput, setPcbInput] = useState('');
  const [pcbError, setPcbError] = useState('');

  const { data, isLoading } = useQuery({
    queryKey: ['payrollRun', id],
    queryFn: () => getPayrollRun(id!),
    enabled: !!id,
  });

  const approveMutation = useMutation({
    mutationFn: () => approvePayroll(id!),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['payrollRun', id] }),
  });

  const lockMutation = useMutation({
    mutationFn: () => lockPayroll(id!),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['payrollRun', id] }),
  });

  const deleteMutation = useMutation({
    mutationFn: () => deletePayrollRun(id!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      navigate('/payroll');
    },
  });

  const updatePcbMutation = useMutation({
    mutationFn: ({ employeeId, pcbAmount }: { employeeId: string; pcbAmount: number }) =>
      updatePayrollItemPcb(id!, employeeId, { pcb_amount: pcbAmount }),
    onSuccess: (summary) => {
      queryClient.setQueryData(['payrollRun', id], summary);
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      setEditingPcbEmployeeId(null);
      setPcbInput('');
      setPcbError('');
    },
    onError: (err: unknown) => {
      setPcbError(getErrorMessage(err, 'Failed to update PCB'));
    },
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  if (!data) return <div className="text-center text-gray-500 py-12">Payroll run not found</div>;

  const { payroll_run: run, items } = data;
  const canEditPcb = run.status === 'processed';

  const startEditPcb = (employeeId: string, pcbAmount: number) => {
    setEditingPcbEmployeeId(employeeId);
    setPcbInput((pcbAmount / 100).toFixed(2));
    setPcbError('');
  };

  const savePcb = (employeeId: string) => {
    const amount = Number(pcbInput);
    if (!Number.isFinite(amount) || amount < 0) {
      setPcbError('PCB amount must be zero or greater');
      return;
    }
    updatePcbMutation.mutate({ employeeId, pcbAmount: Math.round(amount * 100) });
  };

  return (
    <div>
      <button
        onClick={() => navigate('/payroll')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> Back to Payroll
      </button>

      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">
            {MONTHS[run.period_month]} {run.period_year} Payroll
          </h1>
          <p className="text-gray-500">{run.employee_count} employees processed</p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => downloadRunPayslips(id!)}
            className="flex items-center gap-2 bg-white border border-gray-200 text-gray-700 px-4 py-2 rounded-lg hover:bg-gray-50 text-sm font-medium"
          >
            <Download className="w-4 h-4" /> Download Payslips
          </button>
          {canDeletePayrollRun(run.status) && (
            <button
              onClick={() => {
                if (confirm(`Delete ${MONTHS[run.period_month]} ${run.period_year} payroll run? This cannot be undone.`)) {
                  deleteMutation.mutate();
                }
              }}
              disabled={deleteMutation.isPending}
              className="flex items-center gap-2 bg-white border border-red-200 text-red-600 px-4 py-2 rounded-lg hover:bg-red-50 disabled:opacity-50 text-sm font-medium"
            >
              <Trash2 className="w-4 h-4" />
              {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
            </button>
          )}
          {run.status === 'processed' && (
            <button
              onClick={() => approveMutation.mutate()}
              disabled={approveMutation.isPending}
              className="flex items-center gap-2 bg-green-600 text-white px-4 py-2 rounded-lg hover:bg-green-700 disabled:opacity-50 text-sm font-medium"
            >
              <CheckCircle className="w-4 h-4" />
              {approveMutation.isPending ? 'Approving...' : 'Approve'}
            </button>
          )}
          {run.status === 'approved' && (
            <button
              onClick={() => lockMutation.mutate()}
              disabled={lockMutation.isPending}
              className="flex items-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 disabled:opacity-50 text-sm font-medium"
            >
              <Lock className="w-4 h-4" />
              {lockMutation.isPending ? 'Locking...' : 'Lock & Mark Paid'}
            </button>
          )}
          <span className={`px-3 py-1 rounded-full text-sm font-medium ${
            run.status === 'paid' ? 'bg-emerald-50 text-emerald-700' :
            run.status === 'approved' ? 'bg-green-50 text-green-700' :
            'bg-gray-100 text-gray-900'
          }`}>
            {run.status}
          </span>
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">Total Gross</p>
          <p className="text-lg font-bold mt-1">{formatMYR(run.total_gross)}</p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">Total Net</p>
          <p className="text-lg font-bold mt-1 text-green-600">{formatMYR(run.total_net)}</p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">Employer Cost</p>
          <p className="text-lg font-bold mt-1">{formatMYR(run.total_employer_cost)}</p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">Total PCB</p>
          <p className="text-lg font-bold mt-1">{formatMYR(run.total_pcb)}</p>
        </div>
      </div>

      {/* Statutory Summary */}
      <div className="grid grid-cols-3 gap-4 mb-6">
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">EPF (Employee + Employer)</p>
          <p className="text-sm font-bold mt-1">
            {formatMYR(run.total_epf_employee)} + {formatMYR(run.total_epf_employer)}
          </p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">SOCSO (Employee + Employer)</p>
          <p className="text-sm font-bold mt-1">
            {formatMYR(run.total_socso_employee)} + {formatMYR(run.total_socso_employer)}
          </p>
        </div>
        <div className="bg-white rounded-2xl shadow border border-gray-200 p-4">
          <p className="text-xs text-gray-400">EIS (Employee + Employer)</p>
          <p className="text-sm font-bold mt-1">
            {formatMYR(run.total_eis_employee)} + {formatMYR(run.total_eis_employer)}
          </p>
        </div>
      </div>

      {/* Employee Details Table */}
      <div className="bg-white rounded-2xl shadow border border-gray-200 overflow-hidden">
        <div className="px-6 py-4 border-b border-gray-200">
          <h2 className="font-semibold">Employee Breakdown</h2>
          {canEditPcb && (
            <p className="mt-1 text-sm text-gray-500">
              PCB can be edited while this payroll run is processed and before it is approved.
            </p>
          )}
          {pcbError && (
            <div className="mt-3 rounded-lg border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
              {pcbError}
            </div>
          )}
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 border-b border-gray-200">
              <tr>
                <th className="text-left px-4 py-3 text-xs font-medium text-gray-500 uppercase">Employee</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">Basic</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">Allowances</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">OT</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">Gross</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">EPF</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">SOCSO</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">EIS</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">PCB</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">Deductions</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">Claims</th>
                <th className="text-right px-4 py-3 text-xs font-medium text-gray-500 uppercase">Net</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {items.map((item) => (
                <tr key={item.employee_id} className="hover:bg-gray-50">
                  <td className="px-4 py-3">
                    <div className="text-sm font-medium">{item.employee_name}</div>
                    <div className="text-xs text-gray-400">{item.employee_number}</div>
                  </td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.basic_salary)}</td>
                  <td className="px-4 py-3 text-sm text-right">{item.total_allowances > 0 ? formatMYR(item.total_allowances) : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right">{item.total_overtime > 0 ? formatMYR(item.total_overtime) : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.gross_salary)}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.epf_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.socso_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right">{formatMYR(item.eis_employee)}</td>
                  <td className="px-4 py-3 text-sm text-right">
                    {editingPcbEmployeeId === item.employee_id ? (
                      <div className="flex min-w-40 items-center justify-end gap-2">
                        <input
                          type="number"
                          min="0"
                          step="0.01"
                          value={pcbInput}
                          onChange={(e) => setPcbInput(e.target.value)}
                          className="w-24 rounded-lg border border-gray-200 px-2 py-1 text-right text-sm outline-none focus:border-gray-400"
                        />
                        <button
                          type="button"
                          onClick={() => savePcb(item.employee_id)}
                          disabled={updatePcbMutation.isPending}
                          className="rounded-lg p-1.5 text-green-600 hover:bg-green-50 disabled:opacity-50"
                          title="Save PCB"
                        >
                          <Save className="w-4 h-4" />
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setEditingPcbEmployeeId(null);
                            setPcbInput('');
                            setPcbError('');
                          }}
                          className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100"
                          title="Cancel PCB edit"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    ) : (
                      <div className="flex items-center justify-end gap-2">
                        <span>{formatMYR(item.pcb_amount)}</span>
                        {canEditPcb && (
                          <button
                            type="button"
                            onClick={() => startEditPcb(item.employee_id, item.pcb_amount)}
                            className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100 hover:text-black"
                            title="Edit PCB"
                          >
                            <Pencil className="w-4 h-4" />
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-3 text-sm text-right text-red-600">{formatMYR(item.total_deductions)}</td>
                  <td className="px-4 py-3 text-sm text-right">{item.total_claims > 0 ? <span className="text-blue-600">{formatMYR(item.total_claims)}</span> : '-'}</td>
                  <td className="px-4 py-3 text-sm text-right font-bold text-green-600">{formatMYR(item.net_salary)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
