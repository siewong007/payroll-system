import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Calculator, Pencil, Plus, Trash2, X } from 'lucide-react';
import { getEmployees } from '@/api/employees';
import {
  createPayrollEntry,
  deletePayrollEntry,
  getPayrollEntries,
  getPayrollGroups,
  processPayroll,
  updatePayrollEntry,
} from '@/api/payroll';
import { formatMYR, getErrorMessage } from '@/lib/utils';
import type { PayrollEntryWithEmployee } from '@/types';

const MONTHS = [
  { value: 1, label: 'January' }, { value: 2, label: 'February' },
  { value: 3, label: 'March' }, { value: 4, label: 'April' },
  { value: 5, label: 'May' }, { value: 6, label: 'June' },
  { value: 7, label: 'July' }, { value: 8, label: 'August' },
  { value: 9, label: 'September' }, { value: 10, label: 'October' },
  { value: 11, label: 'November' }, { value: 12, label: 'December' },
];

type EntryKind = 'monthly_allowance' | 'other_earning' | 'deduction';

function getEntryKind(entry: Pick<PayrollEntryWithEmployee, 'category' | 'item_type'>): EntryKind {
  if (entry.category === 'earning' && ['allowance', 'monthly_allowance'].includes(entry.item_type)) {
    return 'monthly_allowance';
  }
  return entry.category === 'deduction' ? 'deduction' : 'other_earning';
}

function getEntryKindLabel(entry: Pick<PayrollEntryWithEmployee, 'category' | 'item_type'>) {
  const kind = getEntryKind(entry);
  if (kind === 'monthly_allowance') return 'Monthly allowance';
  if (kind === 'deduction') return 'Deduction';
  return 'Other earning';
}

export function PayrollProcess() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const now = new Date();

  const [groupId, setGroupId] = useState('');
  const [year, setYear] = useState(now.getFullYear());
  const [month, setMonth] = useState(now.getMonth() + 1);
  const [notes, setNotes] = useState('');
  const [editingEntry, setEditingEntry] = useState<PayrollEntryWithEmployee | null>(null);
  const [entryError, setEntryError] = useState('');
  const [entryForm, setEntryForm] = useState({
    employee_id: '',
    category: 'earning' as 'earning' | 'deduction',
    item_type: 'monthly_allowance',
    description: 'Monthly allowance',
    amount: '',
    quantity: '',
    rate: '',
    is_taxable: true,
  });

  const { data: groups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
  });

  const { data: employeeResp } = useQuery({
    queryKey: ['payroll-entry-employees'],
    queryFn: () => getEmployees({ is_active: true, page: 1, per_page: 200 }),
  });

  const employees = employeeResp?.data ?? [];

  const { data: entries = [] } = useQuery({
    queryKey: ['payrollEntries', year, month],
    queryFn: () => getPayrollEntries({ period_year: year, period_month: month }),
  });

  const mutation = useMutation({
    mutationFn: processPayroll,
    onSuccess: (run) => {
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      navigate(`/payroll/${run.id}`);
    },
  });

  const saveEntryMutation = useMutation({
    mutationFn: () => {
      const amount = Math.round(Number(entryForm.amount) * 100);
      const payload = {
        employee_id: entryForm.employee_id,
        period_year: year,
        period_month: month,
        category: entryForm.category,
        item_type: entryForm.item_type,
        description: entryForm.description,
        amount,
        quantity: entryForm.quantity ? Number(entryForm.quantity) : undefined,
        rate: entryForm.rate ? Math.round(Number(entryForm.rate) * 100) : undefined,
        is_taxable: entryForm.is_taxable,
      };

      return editingEntry
        ? updatePayrollEntry(editingEntry.id, payload)
        : createPayrollEntry(payload);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payrollEntries'] });
      resetEntryForm();
    },
    onError: (err: unknown) => {
      setEntryError(getErrorMessage(err, 'Failed to save payroll entry'));
    },
  });

  const deleteEntryMutation = useMutation({
    mutationFn: deletePayrollEntry,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['payrollEntries'] }),
  });

  const handleProcess = () => {
    if (!groupId) return;
    mutation.mutate({
      payroll_group_id: groupId,
      period_year: year,
      period_month: month,
      notes: notes || undefined,
    });
  };

  const resetEntryForm = () => {
    setEditingEntry(null);
    setEntryError('');
    setEntryForm({
      employee_id: '',
      category: 'earning',
      item_type: 'monthly_allowance',
      description: 'Monthly allowance',
      amount: '',
      quantity: '',
      rate: '',
      is_taxable: true,
    });
  };

  const startEditEntry = (entry: PayrollEntryWithEmployee) => {
    setEditingEntry(entry);
    setEntryError('');
    setEntryForm({
      employee_id: entry.employee_id,
      category: entry.category,
      item_type: entry.item_type,
      description: entry.description,
      amount: (entry.amount / 100).toFixed(2),
      quantity: entry.quantity ? String(entry.quantity) : '',
      rate: entry.rate ? (entry.rate / 100).toFixed(2) : '',
      is_taxable: entry.is_taxable ?? true,
    });
  };

  const handleSaveEntry = () => {
    setEntryError('');
    if (!entryForm.employee_id || !entryForm.description || !entryForm.amount) {
      setEntryError('Employee, description, and amount are required');
      return;
    }
    if (Number(entryForm.amount) <= 0) {
      setEntryError('Amount must be greater than zero');
      return;
    }
    saveEntryMutation.mutate();
  };

  const selectedEntryKind = getEntryKind(entryForm);

  return (
    <div className="max-w-6xl space-y-6">
      <button
        onClick={() => navigate('/payroll')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> Back to Payroll
      </button>

      <h1 className="text-2xl font-bold text-gray-900 mb-6">Process Monthly Payroll</h1>

      <div className="bg-white rounded-2xl shadow border border-gray-200 p-6 space-y-6 max-w-2xl">
        {mutation.isError && (
          <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-lg">
            {(mutation.error as Error & { response?: { data?: { error?: string } } })?.response?.data?.error ||
              'Failed to process payroll'}
          </div>
        )}

        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Payroll Group *</label>
          <select
            value={groupId}
            onChange={(e) => setGroupId(e.target.value)}
            className="w-full border border-gray-200 p-2 rounded-lg focus:ring-1 focus:ring-black outline-none"
          >
            <option value="">Select payroll group</option>
            {groups?.map((g) => (
              <option key={g.id} value={g.id}>
                {g.name} (cutoff: day {g.cutoff_day}, pay: day {g.payment_day})
              </option>
            ))}
          </select>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Year *</label>
            <select
              value={year}
              onChange={(e) => setYear(parseInt(e.target.value))}
              className="w-full border border-gray-200 p-2 rounded-lg focus:ring-1 focus:ring-black outline-none"
            >
              {[year - 1, year, year + 1].map((y) => (
                <option key={y} value={y}>{y}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Month *</label>
            <select
              value={month}
              onChange={(e) => setMonth(parseInt(e.target.value))}
              className="w-full border border-gray-200 p-2 rounded-lg focus:ring-1 focus:ring-black outline-none"
            >
              {MONTHS.map((m) => (
                <option key={m.value} value={m.value}>{m.label}</option>
              ))}
            </select>
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Notes (optional)</label>
          <textarea
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={3}
            className="w-full border border-gray-200 p-2 rounded-lg focus:ring-1 focus:ring-black outline-none"
            placeholder="Any notes for this payroll run..."
          />
        </div>

        <div className="bg-amber-50 border border-amber-200 rounded-lg p-4 text-sm text-amber-800">
          This will calculate EPF, SOCSO, EIS, PCB, and all deductions for every active employee
          in the selected payroll group for the selected period.
        </div>

        <button
          onClick={handleProcess}
          disabled={!groupId || mutation.isPending}
          className="flex items-center gap-2 bg-black text-white px-6 py-2.5 rounded-lg font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors"
        >
          <Calculator className="w-4 h-4" />
          {mutation.isPending ? 'Processing...' : 'Process Payroll'}
        </button>
      </div>

      <div className="bg-white rounded-2xl shadow border border-gray-200 overflow-hidden">
        <div className="flex flex-col gap-2 border-b border-gray-100 px-6 py-4 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="font-semibold text-gray-900">Monthly Allowances & Payroll Adjustments</h2>
            <p className="text-sm text-gray-500">
              Create employee-specific allowances for this month, or add other one-off earnings and deductions before processing payroll.
            </p>
          </div>
          {editingEntry && (
            <button type="button" onClick={resetEntryForm} className="btn-secondary !py-2 text-sm">
              <X className="w-4 h-4" /> Cancel Edit
            </button>
          )}
        </div>

        <div className="grid grid-cols-1 gap-4 border-b border-gray-100 p-6 lg:grid-cols-6">
          <div className="lg:col-span-2">
            <label className="form-label">Employee *</label>
            <select
              value={entryForm.employee_id}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, employee_id: e.target.value }))}
              className="form-input"
            >
              <option value="">Select employee...</option>
              {employees.map((employee) => (
                <option key={employee.id} value={employee.id}>
                  {employee.employee_number} - {employee.full_name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="form-label">Entry Kind *</label>
            <select
              value={selectedEntryKind}
              onChange={(e) => {
                const kind = e.target.value as EntryKind;
                setEntryForm((prev) => ({
                  ...prev,
                  category: kind === 'deduction' ? 'deduction' : 'earning',
                  item_type: kind === 'monthly_allowance'
                    ? 'monthly_allowance'
                    : kind === 'deduction'
                      ? 'manual_deduction'
                      : 'manual_adjustment',
                  description: kind === 'monthly_allowance' && !prev.description
                    ? 'Monthly allowance'
                    : prev.description,
                }));
              }}
              className="form-input"
            >
              <option value="monthly_allowance">Monthly Allowance</option>
              <option value="other_earning">Other Earning</option>
              <option value="deduction">Deduction</option>
            </select>
          </div>
          <div>
            <label className="form-label">Item Type *</label>
            <input
              value={entryForm.item_type}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, item_type: e.target.value }))}
              disabled={selectedEntryKind === 'monthly_allowance'}
              className="form-input disabled:bg-gray-50 disabled:text-gray-500"
              placeholder="monthly_allowance"
            />
          </div>
          <div>
            <label className="form-label">Amount (RM) *</label>
            <input
              type="number"
              min="0"
              step="0.01"
              value={entryForm.amount}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, amount: e.target.value }))}
              className="form-input"
              placeholder="0.00"
            />
          </div>
          <div className="flex items-end">
            <button
              type="button"
              onClick={handleSaveEntry}
              disabled={saveEntryMutation.isPending}
              className="btn-primary w-full"
            >
              <Plus className="w-4 h-4" />
              {saveEntryMutation.isPending ? 'Saving...' : editingEntry ? 'Update' : 'Add'}
            </button>
          </div>
          <div className="lg:col-span-3">
            <label className="form-label">Description *</label>
            <input
              value={entryForm.description}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, description: e.target.value }))}
              className="form-input"
              placeholder="e.g., Performance bonus"
            />
          </div>
          <div>
            <label className="form-label">Quantity</label>
            <input
              type="number"
              min="0"
              step="0.01"
              value={entryForm.quantity}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, quantity: e.target.value }))}
              className="form-input"
            />
          </div>
          <div>
            <label className="form-label">Rate (RM)</label>
            <input
              type="number"
              min="0"
              step="0.01"
              value={entryForm.rate}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, rate: e.target.value }))}
              className="form-input"
            />
          </div>
          <label className="flex items-center gap-2 pt-7 text-sm font-medium text-gray-600">
            <input
              type="checkbox"
              checked={entryForm.is_taxable}
              onChange={(e) => setEntryForm((prev) => ({ ...prev, is_taxable: e.target.checked }))}
              className="h-4 w-4 rounded border-gray-300 text-gray-900 focus:ring-gray-900"
            />
            Taxable
          </label>
          {entryError && (
            <div className="lg:col-span-6 rounded-lg border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
              {entryError}
            </div>
          )}
        </div>

        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 text-left">
              <tr>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wide text-gray-500">Employee</th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wide text-gray-500">Type</th>
                <th className="px-6 py-3 text-xs font-medium uppercase tracking-wide text-gray-500">Description</th>
                <th className="px-6 py-3 text-right text-xs font-medium uppercase tracking-wide text-gray-500">Amount</th>
                <th className="px-6 py-3 text-center text-xs font-medium uppercase tracking-wide text-gray-500">Taxable</th>
                <th className="px-6 py-3 text-center text-xs font-medium uppercase tracking-wide text-gray-500">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {entries.length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-6 py-10 text-center text-sm text-gray-400">
                    No adjustments for {MONTHS.find((item) => item.value === month)?.label} {year}.
                  </td>
                </tr>
              ) : entries.map((entry) => (
                <tr key={entry.id} className="hover:bg-gray-50">
                  <td className="px-6 py-4 text-sm">
                    <div className="font-medium text-gray-900">{entry.employee_name}</div>
                    <div className="text-xs text-gray-400">{entry.employee_number}</div>
                  </td>
                  <td className="px-6 py-4 text-sm">
                    <span className={`badge ${entry.category === 'earning' ? 'badge-approved' : 'badge-rejected'}`}>
                      {getEntryKindLabel(entry)}
                    </span>
                    <div className="mt-1 text-xs text-gray-400">{entry.item_type}</div>
                  </td>
                  <td className="px-6 py-4 text-sm text-gray-600">{entry.description}</td>
                  <td className="px-6 py-4 text-right text-sm font-semibold">{formatMYR(entry.amount)}</td>
                  <td className="px-6 py-4 text-center text-sm text-gray-500">{entry.is_taxable ? 'Yes' : 'No'}</td>
                  <td className="px-6 py-4 text-center">
                    <div className="flex items-center justify-center gap-2">
                      <button
                        type="button"
                        onClick={() => startEditEntry(entry)}
                        className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100 hover:text-black"
                        title="Edit"
                      >
                        <Pencil className="w-4 h-4" />
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          if (confirm('Delete this payroll adjustment?')) deleteEntryMutation.mutate(entry.id);
                        }}
                        disabled={deleteEntryMutation.isPending}
                        className="rounded-lg p-1.5 text-red-500 hover:bg-red-50 hover:text-red-700 disabled:opacity-50"
                        title="Delete"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
