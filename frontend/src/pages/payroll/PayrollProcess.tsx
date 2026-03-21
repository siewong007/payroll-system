import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Calculator } from 'lucide-react';
import { getPayrollGroups, processPayroll } from '@/api/payroll';

const MONTHS = [
  { value: 1, label: 'January' }, { value: 2, label: 'February' },
  { value: 3, label: 'March' }, { value: 4, label: 'April' },
  { value: 5, label: 'May' }, { value: 6, label: 'June' },
  { value: 7, label: 'July' }, { value: 8, label: 'August' },
  { value: 9, label: 'September' }, { value: 10, label: 'October' },
  { value: 11, label: 'November' }, { value: 12, label: 'December' },
];

export function PayrollProcess() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const now = new Date();

  const [groupId, setGroupId] = useState('');
  const [year, setYear] = useState(now.getFullYear());
  const [month, setMonth] = useState(now.getMonth() + 1);
  const [notes, setNotes] = useState('');

  const { data: groups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
  });

  const mutation = useMutation({
    mutationFn: processPayroll,
    onSuccess: (run) => {
      queryClient.invalidateQueries({ queryKey: ['payrollRuns'] });
      navigate(`/payroll/${run.id}`);
    },
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

  return (
    <div className="max-w-2xl">
      <button
        onClick={() => navigate('/payroll')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> Back to Payroll
      </button>

      <h1 className="text-2xl font-bold text-gray-900 mb-6">Process Monthly Payroll</h1>

      <div className="bg-white rounded-2xl shadow border border-gray-200 p-6 space-y-6">
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
    </div>
  );
}
