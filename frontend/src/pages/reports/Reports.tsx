import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { FileText, Download } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import {
  getPayrollSummary,
  getPayrollByDepartment,
  getLeaveReport,
  getClaimsReport,
  getStatutoryReport,
  type PayrollSummaryRow,
  type DepartmentPayrollRow,
  type LeaveReportRow,
  type ClaimsReportRow,
  type StatutoryReportRow,
} from '@/api/reports';
import { DataTable, type Column } from '@/components/ui/DataTable';

const fmt = (sen: number) => `RM ${(sen / 100).toLocaleString('en-MY', { minimumFractionDigits: 2 })}`;
const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

type ReportTab = 'payroll' | 'department' | 'statutory' | 'leave' | 'claims';

const payrollColumns: Column<PayrollSummaryRow>[] = [
  { key: 'period', header: 'Period', render: (r) => <span className="font-semibold text-gray-900">{r.period}</span> },
  { key: 'employees', header: 'Employees', align: 'right', render: (r) => r.employee_count },
  { key: 'gross', header: 'Gross', align: 'right', render: (r) => fmt(r.total_gross) },
  { key: 'net', header: 'Net', align: 'right', render: (r) => fmt(r.total_net) },
  { key: 'epf_ee', header: 'EPF (EE)', align: 'right', render: (r) => fmt(r.total_epf_employee) },
  { key: 'epf_er', header: 'EPF (ER)', align: 'right', render: (r) => fmt(r.total_epf_employer) },
  { key: 'socso', header: 'SOCSO', align: 'right', render: (r) => fmt(r.total_socso_employee + r.total_socso_employer) },
  { key: 'pcb', header: 'PCB', align: 'right', render: (r) => fmt(r.total_pcb) },
  { key: 'cost', header: 'Total Cost', align: 'right', render: (r) => <span className="font-semibold">{fmt(r.total_employer_cost)}</span> },
];

const deptColumns: Column<DepartmentPayrollRow>[] = [
  { key: 'department', header: 'Department', render: (r) => <span className="font-semibold">{r.department || 'Unassigned'}</span> },
  { key: 'employees', header: 'Employees', align: 'right', render: (r) => r.employee_count },
  { key: 'gross', header: 'Total Gross', align: 'right', render: (r) => fmt(r.total_gross) },
  { key: 'net', header: 'Total Net', align: 'right', render: (r) => fmt(r.total_net) },
  { key: 'cost', header: 'Employer Cost', align: 'right', render: (r) => <span className="font-semibold">{fmt(r.total_employer_cost)}</span> },
];

const statutoryColumns: Column<StatutoryReportRow>[] = [
  {
    key: 'employee', header: 'Employee', render: (r) => (
      <div>
        <div className="font-semibold text-gray-900">{r.employee_name}</div>
        <div className="text-xs text-gray-400">{r.employee_number}</div>
      </div>
    ),
  },
  { key: 'epf_no', header: 'EPF No', render: (r) => <span className="text-gray-500">{r.epf_number || '\u2014'}</span> },
  { key: 'gross', header: 'Gross', align: 'right', render: (r) => fmt(r.gross_salary) },
  { key: 'epf_ee', header: 'EPF (EE)', align: 'right', render: (r) => fmt(r.epf_employee) },
  { key: 'epf_er', header: 'EPF (ER)', align: 'right', render: (r) => fmt(r.epf_employer) },
  { key: 'socso_ee', header: 'SOCSO (EE)', align: 'right', render: (r) => fmt(r.socso_employee) },
  { key: 'socso_er', header: 'SOCSO (ER)', align: 'right', render: (r) => fmt(r.socso_employer) },
  { key: 'eis', header: 'EIS', align: 'right', render: (r) => fmt(r.eis_employee + r.eis_employer) },
  { key: 'pcb', header: 'PCB', align: 'right', render: (r) => fmt(r.pcb_amount) },
  { key: 'zakat', header: 'Zakat', align: 'right', render: (r) => fmt(r.zakat_amount) },
];

const leaveColumns: Column<LeaveReportRow>[] = [
  {
    key: 'employee', header: 'Employee', render: (r) => (
      <div>
        <div className="font-semibold text-gray-900">{r.employee_name}</div>
        <div className="text-xs text-gray-400">{r.employee_number}</div>
      </div>
    ),
  },
  { key: 'department', header: 'Department', render: (r) => <span className="text-gray-500">{r.department || '\u2014'}</span> },
  { key: 'type', header: 'Leave Type', render: (r) => r.leave_type_name },
  { key: 'entitled', header: 'Entitled', align: 'right', render: (r) => r.entitled_days },
  { key: 'taken', header: 'Taken', align: 'right', render: (r) => r.taken_days },
  { key: 'pending', header: 'Pending', align: 'right', render: (r) => <span className="text-amber-600">{r.pending_days}</span> },
  {
    key: 'balance', header: 'Balance', align: 'right', render: (r) => (
      <span className={`font-semibold ${Number(r.balance) <= 0 ? 'text-red-600' : 'text-emerald-600'}`}>{r.balance}</span>
    ),
  },
];

const claimsColumns: Column<ClaimsReportRow>[] = [
  {
    key: 'employee', header: 'Employee', render: (r) => (
      <div>
        <div className="font-semibold text-gray-900">{r.employee_name}</div>
        <div className="text-xs text-gray-400">{r.employee_number}</div>
      </div>
    ),
  },
  { key: 'department', header: 'Department', render: (r) => <span className="text-gray-500">{r.department || '\u2014'}</span> },
  { key: 'total', header: 'Total Claims', align: 'right', render: (r) => r.total_claims },
  { key: 'amount', header: 'Total Amount', align: 'right', render: (r) => <span className="font-semibold">{fmt(r.total_amount)}</span> },
  { key: 'approved', header: 'Approved', align: 'right', render: (r) => <span className="text-emerald-600">{r.approved_count} ({fmt(r.approved_amount)})</span> },
  { key: 'pending', header: 'Pending', align: 'right', render: (r) => <span className="text-amber-600">{r.pending_count} ({fmt(r.pending_amount)})</span> },
  { key: 'rejected', header: 'Rejected', align: 'right', render: (r) => <span className="text-red-600">{r.rejected_count}</span> },
];

const PAYROLL_TABS: ReportTab[] = ['payroll', 'department', 'statutory'];

export function Reports() {
  const { user } = useAuth();
  const isExec = user?.role === 'exec';
  const [tab, setTab] = useState<ReportTab>(isExec ? 'leave' : 'payroll');
  const [year, setYear] = useState(2026);
  const [month, setMonth] = useState(3);

  const payrollQuery = useQuery({
    queryKey: ['report-payroll', year],
    queryFn: () => getPayrollSummary(year),
    enabled: tab === 'payroll',
  });

  const deptQuery = useQuery({
    queryKey: ['report-dept', year, month],
    queryFn: () => getPayrollByDepartment(year, month),
    enabled: tab === 'department',
  });

  const statutoryQuery = useQuery({
    queryKey: ['report-statutory', year, month],
    queryFn: () => getStatutoryReport(year, month),
    enabled: tab === 'statutory',
  });

  const leaveQuery = useQuery({
    queryKey: ['report-leave', year],
    queryFn: () => getLeaveReport(year),
    enabled: tab === 'leave',
  });

  const claimsQuery = useQuery({
    queryKey: ['report-claims', year],
    queryFn: () => getClaimsReport(`${year}-01-01`, `${year}-12-31`),
    enabled: tab === 'claims',
  });

  const exportCSV = (headers: string[], rows: string[][], filename: string) => {
    const csv = [headers.join(','), ...rows.map((r) => r.join(','))].join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  };

  const tabs = useMemo(() => {
    const all: { key: ReportTab; label: string }[] = [
      { key: 'payroll', label: 'Payroll Summary' },
      { key: 'department', label: 'By Department' },
      { key: 'statutory', label: 'Statutory' },
      { key: 'leave', label: 'Leave Balance' },
      { key: 'claims', label: 'Claims' },
    ];
    return isExec ? all.filter((t) => !PAYROLL_TABS.includes(t.key)) : all;
  }, [isExec]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="page-header">
          <h1 className="page-title">Reports</h1>
          <p className="page-subtitle">Company payroll and HR reports</p>
        </div>
        <div className="flex items-center gap-2">
          <select value={year} onChange={(e) => setYear(Number(e.target.value))} className="form-input !w-auto">
            {[2024, 2025, 2026].map((y) => <option key={y} value={y}>{y}</option>)}
          </select>
          {(tab === 'department' || tab === 'statutory') && (
            <select value={month} onChange={(e) => setMonth(Number(e.target.value))} className="form-input !w-auto">
              {months.map((m, i) => <option key={i} value={i + 1}>{m}</option>)}
            </select>
          )}
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200 overflow-x-auto">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 whitespace-nowrap transition-all-fast ${
              tab === t.key ? 'border-black text-gray-900' : 'border-transparent text-gray-400 hover:text-gray-700'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Payroll Summary */}
      {tab === 'payroll' && (
        <div className="space-y-3">
          {payrollQuery.data && payrollQuery.data.length > 0 && (
            <div className="flex justify-between items-center">
              <h2 className="font-semibold text-gray-900">Monthly Payroll Summary — {year}</h2>
              <button
                onClick={() =>
                  exportCSV(
                    ['Period', 'Employees', 'Gross', 'Net', 'EPF(EE)', 'EPF(ER)', 'SOCSO(EE)', 'SOCSO(ER)', 'EIS(EE)', 'EIS(ER)', 'PCB', 'Zakat', 'Employer Cost'],
                    payrollQuery.data!.map((r) => [
                      r.period, String(r.employee_count), String(r.total_gross / 100), String(r.total_net / 100),
                      String(r.total_epf_employee / 100), String(r.total_epf_employer / 100),
                      String(r.total_socso_employee / 100), String(r.total_socso_employer / 100),
                      String(r.total_eis_employee / 100), String(r.total_eis_employer / 100),
                      String(r.total_pcb / 100), String(r.total_zakat / 100), String(r.total_employer_cost / 100),
                    ]),
                    `payroll-summary-${year}.csv`
                  )
                }
                className="btn-secondary !py-1.5 !px-3 !text-xs"
              >
                <Download className="w-3.5 h-3.5" /> Export CSV
              </button>
            </div>
          )}
          <DataTable
            columns={payrollColumns}
            data={payrollQuery.data ?? []}
            perPage={12}
            isLoading={payrollQuery.isLoading}
            emptyMessage={`No payroll data for ${year}`}
            emptyIcon={<FileText className="w-8 h-8 text-gray-200" />}
            rowKey={(r) => r.period}
            summaryTitle={(r) => `Payroll — ${r.period}`}
            renderSummary={(r) => (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label="Period" value={r.period} />
                <SummaryField label="Employees" value={String(r.employee_count)} />
                <SummaryField label="Total Gross" value={fmt(r.total_gross)} />
                <SummaryField label="Total Net" value={fmt(r.total_net)} />
                <SummaryField label="EPF (Employee)" value={fmt(r.total_epf_employee)} />
                <SummaryField label="EPF (Employer)" value={fmt(r.total_epf_employer)} />
                <SummaryField label="SOCSO (Employee)" value={fmt(r.total_socso_employee)} />
                <SummaryField label="SOCSO (Employer)" value={fmt(r.total_socso_employer)} />
                <SummaryField label="EIS (Employee)" value={fmt(r.total_eis_employee)} />
                <SummaryField label="EIS (Employer)" value={fmt(r.total_eis_employer)} />
                <SummaryField label="PCB" value={fmt(r.total_pcb)} />
                <SummaryField label="Zakat" value={fmt(r.total_zakat)} />
                <div className="col-span-2 pt-2 border-t border-gray-100">
                  <SummaryField label="Total Employer Cost" value={fmt(r.total_employer_cost)} />
                </div>
              </div>
            )}
          />
        </div>
      )}

      {/* Department Breakdown */}
      {tab === 'department' && (
        <div className="space-y-3">
          <h2 className="font-semibold text-gray-900">Payroll by Department — {months[month - 1]} {year}</h2>
          <DataTable
            columns={deptColumns}
            data={deptQuery.data ?? []}
            perPage={10}
            isLoading={deptQuery.isLoading}
            emptyMessage="No data"
            rowKey={(r) => r.department || 'unassigned'}
            summaryTitle={(r) => `${r.department || 'Unassigned'} Department`}
          />
        </div>
      )}

      {/* Statutory Report */}
      {tab === 'statutory' && (
        <div className="space-y-3">
          <div className="flex justify-between items-center">
            <h2 className="font-semibold text-gray-900">Statutory Deductions — {months[month - 1]} {year}</h2>
            {statutoryQuery.data && statutoryQuery.data.length > 0 && (
              <button
                onClick={() =>
                  exportCSV(
                    ['Employee', 'ID', 'IC', 'EPF No', 'Gross', 'EPF(EE)', 'EPF(ER)', 'SOCSO(EE)', 'SOCSO(ER)', 'EIS(EE)', 'EIS(ER)', 'PCB', 'Zakat'],
                    statutoryQuery.data!.map((r) => [
                      `"${r.employee_name}"`, r.employee_number, r.ic_number || '', r.epf_number || '',
                      String(r.gross_salary / 100), String(r.epf_employee / 100), String(r.epf_employer / 100),
                      String(r.socso_employee / 100), String(r.socso_employer / 100),
                      String(r.eis_employee / 100), String(r.eis_employer / 100),
                      String(r.pcb_amount / 100), String(r.zakat_amount / 100),
                    ]),
                    `statutory-${year}-${String(month).padStart(2, '0')}.csv`
                  )
                }
                className="btn-secondary !py-1.5 !px-3 !text-xs"
              >
                <Download className="w-3.5 h-3.5" /> Export CSV
              </button>
            )}
          </div>
          <DataTable
            columns={statutoryColumns}
            data={statutoryQuery.data ?? []}
            perPage={10}
            isLoading={statutoryQuery.isLoading}
            emptyMessage="No data"
            rowKey={(r) => r.employee_number}
            summaryTitle={(r) => `${r.employee_name} — Statutory`}
            renderSummary={(r) => (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label="Employee" value={r.employee_name} />
                <SummaryField label="ID" value={r.employee_number} />
                <SummaryField label="IC Number" value={r.ic_number || '\u2014'} />
                <SummaryField label="EPF Number" value={r.epf_number || '\u2014'} />
                <SummaryField label="Gross Salary" value={fmt(r.gross_salary)} />
                <SummaryField label="Basic Salary" value={fmt(r.basic_salary)} />
                <SummaryField label="EPF (Employee)" value={fmt(r.epf_employee)} />
                <SummaryField label="EPF (Employer)" value={fmt(r.epf_employer)} />
                <SummaryField label="SOCSO (Employee)" value={fmt(r.socso_employee)} />
                <SummaryField label="SOCSO (Employer)" value={fmt(r.socso_employer)} />
                <SummaryField label="EIS (Employee)" value={fmt(r.eis_employee)} />
                <SummaryField label="EIS (Employer)" value={fmt(r.eis_employer)} />
                <SummaryField label="PCB" value={fmt(r.pcb_amount)} />
                <SummaryField label="Zakat" value={fmt(r.zakat_amount)} />
              </div>
            )}
          />
        </div>
      )}

      {/* Leave Report */}
      {tab === 'leave' && (
        <div className="space-y-3">
          <div className="flex justify-between items-center">
            <h2 className="font-semibold text-gray-900">Leave Balances — {year}</h2>
            {leaveQuery.data && leaveQuery.data.length > 0 && (
              <button
                onClick={() =>
                  exportCSV(
                    ['Employee', 'ID', 'Department', 'Leave Type', 'Entitled', 'Taken', 'Pending', 'Balance'],
                    leaveQuery.data!.map((r) => [
                      `"${r.employee_name}"`, r.employee_number, r.department || '',
                      r.leave_type_name, String(r.entitled_days), String(r.taken_days),
                      String(r.pending_days), String(r.balance),
                    ]),
                    `leave-report-${year}.csv`
                  )
                }
                className="btn-secondary !py-1.5 !px-3 !text-xs"
              >
                <Download className="w-3.5 h-3.5" /> Export CSV
              </button>
            )}
          </div>
          <DataTable
            columns={leaveColumns}
            data={leaveQuery.data ?? []}
            perPage={10}
            isLoading={leaveQuery.isLoading}
            emptyMessage="No leave data"
            rowKey={(r, i) => `${r.employee_number}-${r.leave_type_name}-${i}`}
            summaryTitle={(r) => `${r.employee_name} — ${r.leave_type_name}`}
          />
        </div>
      )}

      {/* Claims Report */}
      {tab === 'claims' && (
        <div className="space-y-3">
          <h2 className="font-semibold text-gray-900">Claims Summary — {year}</h2>
          <DataTable
            columns={claimsColumns}
            data={claimsQuery.data ?? []}
            perPage={10}
            isLoading={claimsQuery.isLoading}
            emptyMessage="No claims data"
            rowKey={(r) => r.employee_number}
            summaryTitle={(r) => `${r.employee_name} — Claims`}
            renderSummary={(r) => (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label="Employee" value={r.employee_name} />
                <SummaryField label="Department" value={r.department || '\u2014'} />
                <SummaryField label="Total Claims" value={String(r.total_claims)} />
                <SummaryField label="Total Amount" value={fmt(r.total_amount)} />
                <SummaryField label="Approved" value={`${r.approved_count} (${fmt(r.approved_amount)})`} />
                <SummaryField label="Pending" value={`${r.pending_count} (${fmt(r.pending_amount)})`} />
                <SummaryField label="Rejected" value={String(r.rejected_count)} />
              </div>
            )}
          />
        </div>
      )}
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
