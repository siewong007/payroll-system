import { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { FileText, Download, ChevronDown, ChevronRight, AlertTriangle } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import {
  getReportPeriods,
  getPayrollSummary,
  getPayrollByDepartment,
  getLeaveReport,
  getClaimsReport,
  getStatutoryReport,
  downloadStatutoryExport,
  getEaEmployees,
  downloadEaForm,
  type PayrollSummaryRow,
  type DepartmentPayrollRow,
  type LeaveReportRow,
  type ClaimsReportRow,
  type StatutoryReportRow,
} from '@/api/reports';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { canAccessPayrollData } from '@/lib/roles';

const fmt = (sen: number) => `RM ${(sen / 100).toLocaleString('en-MY', { minimumFractionDigits: 2 })}`;
const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

type ReportTab = 'payroll' | 'department' | 'statutory' | 'leave' | 'claims' | 'ea_form';

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

const PAYROLL_TABS: ReportTab[] = ['payroll', 'department', 'statutory', 'ea_form'];

// ─── Leave eligibility rules ───

function getLeaveEligibility(
  leaveType: string,
  gender: string | null,
  maritalStatus: string | null,
  numChildren: number | null,
): { eligible: boolean; reason?: string } {
  const name = leaveType.toLowerCase();

  if (name.includes('maternity')) {
    if (gender !== 'female') return { eligible: false, reason: 'Available to female employees.' };
  }

  if (name.includes('paternity')) {
    if (gender !== 'male') return { eligible: false, reason: 'Available to male employees.' };
    if ((numChildren ?? 0) === 0) return { eligible: false, reason: 'Employee has no registered children (requires date of birth).' };
  }

  if (name.includes('marriage')) {
    if (maritalStatus === 'married') return { eligible: false, reason: 'Employee is already married.' };
    if (!maritalStatus || maritalStatus === 'single') return { eligible: true };
  }

  return { eligible: true };
}

// ─── Leave Report: group by employee ───

interface EmployeeLeaveGroup {
  employee_name: string;
  employee_number: string;
  department: string | null;
  gender: string | null;
  marital_status: string | null;
  num_children: number | null;
  leaves: LeaveReportRow[];
}

function groupByEmployee(rows: LeaveReportRow[]): EmployeeLeaveGroup[] {
  const map = new Map<string, EmployeeLeaveGroup>();
  for (const r of rows) {
    let group = map.get(r.employee_number);
    if (!group) {
      group = {
        employee_name: r.employee_name,
        employee_number: r.employee_number,
        department: r.department,
        gender: r.gender,
        marital_status: r.marital_status,
        num_children: r.num_children,
        leaves: [],
      };
      map.set(r.employee_number, group);
    }
    group.leaves.push(r);
  }
  return Array.from(map.values());
}

export function Reports() {
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user?.role);
  const [tab, setTab] = useState<ReportTab>(canViewPayroll ? 'payroll' : 'leave');
  const today = new Date();
  const fallbackYear = today.getFullYear();
  const fallbackMonth = today.getMonth() + 1;
  const [year, setYear] = useState(fallbackYear);
  const [month, setMonth] = useState(fallbackMonth);
  const [hasInitializedPeriods, setHasInitializedPeriods] = useState(false);

  const periodsQuery = useQuery({
    queryKey: ['report-periods'],
    queryFn: getReportPeriods,
  });

  const payrollMonthsByYear = useMemo(
    () => new Map((periodsQuery.data?.payroll_months ?? []).map((period) => [period.year, period.months])),
    [periodsQuery.data?.payroll_months]
  );

  const yearOptions = useMemo(() => {
    const source = (() => {
      switch (tab) {
        case 'payroll':
        case 'department':
        case 'statutory':
          return periodsQuery.data?.payroll_years;
        case 'leave':
          return periodsQuery.data?.leave_years;
        case 'claims':
          return periodsQuery.data?.claims_years;
        case 'ea_form':
          return periodsQuery.data?.ea_form_years;
      }
    })();

    return source && source.length > 0 ? source : [periodsQuery.data?.default_year ?? fallbackYear];
  }, [fallbackYear, periodsQuery.data, tab]);

  const monthOptions = useMemo(() => {
    const availableMonths = payrollMonthsByYear.get(year);
    if (availableMonths && availableMonths.length > 0) {
      return availableMonths;
    }
    return [periodsQuery.data?.default_month ?? fallbackMonth];
  }, [fallbackMonth, payrollMonthsByYear, periodsQuery.data?.default_month, year]);

  useEffect(() => {
    if (!periodsQuery.data || hasInitializedPeriods) {
      return;
    }

    setYear(periodsQuery.data.default_year);
    setMonth(periodsQuery.data.default_month);
    setHasInitializedPeriods(true);
  }, [hasInitializedPeriods, periodsQuery.data]);

  useEffect(() => {
    if (yearOptions.length === 0 || yearOptions.includes(year)) {
      return;
    }

    setYear(yearOptions[yearOptions.length - 1]);
  }, [year, yearOptions]);

  useEffect(() => {
    if (!['department', 'statutory'].includes(tab) || monthOptions.length === 0 || monthOptions.includes(month)) {
      return;
    }

    setMonth(monthOptions[monthOptions.length - 1]);
  }, [month, monthOptions, tab]);

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

  const eaQuery = useQuery({
    queryKey: ['report-ea', year],
    queryFn: () => getEaEmployees(year),
    enabled: tab === 'ea_form',
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
      { key: 'ea_form', label: 'EA Form' },
      { key: 'leave', label: 'Leave Entitlement' },
      { key: 'claims', label: 'Claims' },
    ];
    return canViewPayroll ? all : all.filter((t) => !PAYROLL_TABS.includes(t.key));
  }, [canViewPayroll]);

  useEffect(() => {
    if (!tabs.some((option) => option.key === tab)) {
      setTab(canViewPayroll ? 'payroll' : 'leave');
    }
  }, [canViewPayroll, tab, tabs]);

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">Reports</h1>
          <p className="page-subtitle">
            {canViewPayroll ? 'Company payroll and HR reports' : 'Company HR reports'}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select value={year} onChange={(e) => setYear(Number(e.target.value))} className="form-input !w-auto">
            {yearOptions.map((optionYear) => <option key={optionYear} value={optionYear}>{optionYear}</option>)}
          </select>
          {(tab === 'department' || tab === 'statutory') && (
            <select value={month} onChange={(e) => setMonth(Number(e.target.value))} className="form-input !w-auto">
              {monthOptions.map((optionMonth) => <option key={optionMonth} value={optionMonth}>{months[optionMonth - 1]}</option>)}
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
          {/* Statutory Export Buttons */}
          <div className="bg-white rounded-xl border border-gray-200 p-4">
            <h3 className="text-sm font-medium text-gray-700 mb-3">Download Submission Files — {months[month - 1]} {year}</h3>
            <div className="flex flex-wrap gap-2">
              <button onClick={() => downloadStatutoryExport('epf', year, month)} className="btn-secondary !py-1.5 !px-3 !text-xs">
                <Download className="w-3.5 h-3.5" /> EPF (e-Caruman)
              </button>
              <button onClick={() => downloadStatutoryExport('socso', year, month)} className="btn-secondary !py-1.5 !px-3 !text-xs">
                <Download className="w-3.5 h-3.5" /> SOCSO
              </button>
              <button onClick={() => downloadStatutoryExport('eis', year, month)} className="btn-secondary !py-1.5 !px-3 !text-xs">
                <Download className="w-3.5 h-3.5" /> EIS
              </button>
              <button onClick={() => downloadStatutoryExport('pcb', year, month)} className="btn-secondary !py-1.5 !px-3 !text-xs">
                <Download className="w-3.5 h-3.5" /> PCB / CP39
              </button>
            </div>
          </div>
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

      {/* Leave Entitlement Report */}
      {tab === 'leave' && (
        <LeaveEntitlementReport
          data={leaveQuery.data}
          isLoading={leaveQuery.isLoading}
          year={year}
          onExport={() => {
            if (!leaveQuery.data) return;
            exportCSV(
              ['Employee', 'ID', 'Department', 'Leave Type', 'Status', 'Entitled', 'Taken', 'Pending', 'Balance'],
              leaveQuery.data.map((r) => {
                const elig = getLeaveEligibility(r.leave_type_name, r.gender, r.marital_status, r.num_children);
                return [
                  `"${r.employee_name}"`, r.employee_number, r.department || '',
                  r.leave_type_name, elig.eligible ? 'Eligible' : elig.reason || 'Not eligible',
                  String(r.entitled_days), String(r.taken_days), String(r.pending_days), String(r.balance),
                ];
              }),
              `leave-entitlement-${year}.csv`
            );
          }}
        />
      )}

      {/* EA Form */}
      {tab === 'ea_form' && (
        <EaFormTab data={eaQuery.data} isLoading={eaQuery.isLoading} year={year} />
      )}

      {/* Claims Report */}
      {tab === 'claims' && (
        <ClaimsSummaryReport
          data={claimsQuery.data}
          isLoading={claimsQuery.isLoading}
          year={year}
          onExport={() => {
            if (!claimsQuery.data) return;
            exportCSV(
              ['Employee', 'ID', 'Department', 'Total Claims', 'Total Amount', 'Approved', 'Approved Amount', 'Pending', 'Pending Amount', 'Rejected'],
              claimsQuery.data.map((r) => [
                `"${r.employee_name}"`, r.employee_number, r.department || '',
                String(r.total_claims), String(r.total_amount / 100),
                String(r.approved_count), String(r.approved_amount / 100),
                String(r.pending_count), String(r.pending_amount / 100),
                String(r.rejected_count),
              ]),
              `claims-report-${year}.csv`
            );
          }}
        />
      )}
    </div>
  );
}

// ─── Leave Entitlement Report ───

function LeaveEntitlementReport({
  data,
  isLoading,
  year,
  onExport,
}: {
  data: LeaveReportRow[] | undefined;
  isLoading: boolean;
  year: number;
  onExport: () => void;
}) {
  const [expandedEmployee, setExpandedEmployee] = useState<string | null>(null);
  const [search, setSearch] = useState('');

  const groups = useMemo(() => {
    if (!data) return [];
    const all = groupByEmployee(data);
    if (!search.trim()) return all;
    const q = search.toLowerCase();
    return all.filter(
      (g) =>
        g.employee_name.toLowerCase().includes(q) ||
        g.employee_number.toLowerCase().includes(q) ||
        (g.department || '').toLowerCase().includes(q)
    );
  }, [data, search]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-48">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-semibold text-gray-900">Leave Entitlement — {year}</h2>
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder="Search employee..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="form-input !w-auto min-w-[200px] !text-sm"
          />
          {data && data.length > 0 && (
            <button onClick={onExport} className="btn-secondary !py-1.5 !px-3 !text-xs">
              <Download className="w-3.5 h-3.5" /> Export
            </button>
          )}
        </div>
      </div>

      {groups.length === 0 ? (
        <div className="text-center py-16 text-gray-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" />
          <p className="text-sm">No leave data for {year}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {groups.map((g) => {
            const isOpen = expandedEmployee === g.employee_number;
            const totalBalance = g.leaves.reduce((sum, l) => {
              const elig = getLeaveEligibility(l.leave_type_name, g.gender, g.marital_status, g.num_children);
              return elig.eligible ? sum + Number(l.balance) : sum;
            }, 0);
            const totalTaken = g.leaves.reduce((sum, l) => {
              const elig = getLeaveEligibility(l.leave_type_name, g.gender, g.marital_status, g.num_children);
              return elig.eligible ? sum + Number(l.taken_days) : sum;
            }, 0);

            return (
              <div key={g.employee_number} className="bg-white rounded-xl border border-gray-200 overflow-hidden">
                {/* Employee header */}
                <button
                  onClick={() => setExpandedEmployee(isOpen ? null : g.employee_number)}
                  className="w-full flex items-center gap-4 px-5 py-4 hover:bg-gray-50 transition-colors text-left"
                >
                  <div className="text-gray-400">
                    {isOpen ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-semibold text-gray-900">{g.employee_name}</div>
                    <div className="text-xs text-gray-400">{g.employee_number} · {g.department || 'No department'}</div>
                  </div>
                  <div className="flex items-center gap-6 text-sm shrink-0">
                    <div className="text-center">
                      <div className="text-xs text-gray-400">Taken</div>
                      <div className="font-medium text-gray-700">{totalTaken}d</div>
                    </div>
                    <div className="text-center">
                      <div className="text-xs text-gray-400">Balance</div>
                      <div className={`font-bold ${totalBalance > 0 ? 'text-emerald-600' : 'text-gray-600'}`}>{totalBalance}d</div>
                    </div>
                  </div>
                </button>

                {/* Expanded leave table */}
                {isOpen && (
                  <div className="border-t border-gray-100">
                    <table className="w-full text-sm">
                      <thead>
                        <tr className="bg-gray-50 text-gray-500 text-xs uppercase tracking-wide">
                          <th className="text-left px-5 py-2.5 font-medium">Leave Type</th>
                          <th className="text-left px-5 py-2.5 font-medium">Status / Remarks</th>
                          <th className="text-center px-5 py-2.5 font-medium">Entitled</th>
                          <th className="text-center px-5 py-2.5 font-medium">Taken</th>
                          <th className="text-center px-5 py-2.5 font-medium">Pending</th>
                          <th className="text-center px-5 py-2.5 font-medium">Balance</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-gray-50">
                        {g.leaves.map((l, i) => {
                          const elig = getLeaveEligibility(l.leave_type_name, g.gender, g.marital_status, g.num_children);
                          return (
                            <tr key={i} className={!elig.eligible ? 'bg-gray-50/50' : ''}>
                              <td className="px-5 py-3 font-medium text-gray-900">{l.leave_type_name}</td>
                              <td className="px-5 py-3">
                                {elig.eligible ? (
                                  <span className="text-gray-600">
                                    Employee has <strong>{Number(l.balance)} day(s)</strong> of {l.leave_type_name}
                                  </span>
                                ) : (
                                  <span className="inline-flex items-center gap-1.5 text-amber-600 text-sm">
                                    <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                                    {elig.reason}
                                  </span>
                                )}
                              </td>
                              <td className="px-5 py-3 text-center text-gray-600">{elig.eligible ? Number(l.entitled_days) : '\u2014'}</td>
                              <td className="px-5 py-3 text-center text-gray-600">{elig.eligible ? Number(l.taken_days) : '\u2014'}</td>
                              <td className="px-5 py-3 text-center">
                                {elig.eligible ? (
                                  <span className={Number(l.pending_days) > 0 ? 'text-amber-600' : 'text-gray-600'}>{Number(l.pending_days)}</span>
                                ) : '\u2014'}
                              </td>
                              <td className="px-5 py-3 text-center">
                                {elig.eligible ? (
                                  <span className={`font-bold ${Number(l.balance) > 0 ? 'text-emerald-600' : Number(l.balance) < 0 ? 'text-red-600' : 'text-gray-600'}`}>
                                    {Number(l.balance)}
                                  </span>
                                ) : '\u2014'}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ─── Claims Summary Report ───

function ClaimsSummaryReport({
  data,
  isLoading,
  year,
  onExport,
}: {
  data: ClaimsReportRow[] | undefined;
  isLoading: boolean;
  year: number;
  onExport: () => void;
}) {
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!data) return [];
    if (!search.trim()) return data;
    const q = search.toLowerCase();
    return data.filter(
      (r) =>
        r.employee_name.toLowerCase().includes(q) ||
        r.employee_number.toLowerCase().includes(q) ||
        (r.department || '').toLowerCase().includes(q)
    );
  }, [data, search]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-48">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  // Summary totals
  const totals = (data ?? []).reduce(
    (acc, r) => ({
      claims: acc.claims + r.total_claims,
      amount: acc.amount + r.total_amount,
      approved: acc.approved + r.approved_count,
      approvedAmt: acc.approvedAmt + r.approved_amount,
      pending: acc.pending + r.pending_count,
      pendingAmt: acc.pendingAmt + r.pending_amount,
      rejected: acc.rejected + r.rejected_count,
    }),
    { claims: 0, amount: 0, approved: 0, approvedAmt: 0, pending: 0, pendingAmt: 0, rejected: 0 }
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-semibold text-gray-900">Claims Summary — {year}</h2>
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder="Search employee..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="form-input !w-auto min-w-[200px] !text-sm"
          />
          {data && data.length > 0 && (
            <button onClick={onExport} className="btn-secondary !py-1.5 !px-3 !text-xs">
              <Download className="w-3.5 h-3.5" /> Export
            </button>
          )}
        </div>
      </div>

      {/* Summary cards */}
      {data && data.length > 0 && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <SummaryCard label="Total Claims" value={String(totals.claims)} />
          <SummaryCard label="Total Amount" value={fmt(totals.amount)} />
          <SummaryCard label="Approved" value={`${totals.approved} (${fmt(totals.approvedAmt)})`} className="text-emerald-600" />
          <SummaryCard label="Pending" value={`${totals.pending} (${fmt(totals.pendingAmt)})`} className="text-amber-600" />
        </div>
      )}

      {filtered.length === 0 ? (
        <div className="text-center py-16 text-gray-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" />
          <p className="text-sm">No claims data for {year}</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 text-gray-500 text-xs uppercase tracking-wide">
                <th className="text-left px-5 py-3 font-medium">Employee</th>
                <th className="text-left px-5 py-3 font-medium">Department</th>
                <th className="text-center px-5 py-3 font-medium">Total</th>
                <th className="text-right px-5 py-3 font-medium">Amount</th>
                <th className="text-center px-5 py-3 font-medium">Approved</th>
                <th className="text-center px-5 py-3 font-medium">Pending</th>
                <th className="text-center px-5 py-3 font-medium">Rejected</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-50">
              {filtered.map((r) => (
                <tr key={r.employee_number} className="hover:bg-gray-50/50">
                  <td className="px-5 py-3">
                    <div className="font-semibold text-gray-900">{r.employee_name}</div>
                    <div className="text-xs text-gray-400">{r.employee_number}</div>
                  </td>
                  <td className="px-5 py-3 text-gray-500">{r.department || '\u2014'}</td>
                  <td className="px-5 py-3 text-center font-medium text-gray-900">{r.total_claims}</td>
                  <td className="px-5 py-3 text-right font-semibold text-gray-900">{fmt(r.total_amount)}</td>
                  <td className="px-5 py-3 text-center">
                    <span className="text-emerald-600 font-medium">{r.approved_count}</span>
                    {r.approved_amount > 0 && (
                      <div className="text-xs text-emerald-500">{fmt(r.approved_amount)}</div>
                    )}
                  </td>
                  <td className="px-5 py-3 text-center">
                    <span className="text-amber-600 font-medium">{r.pending_count}</span>
                    {r.pending_amount > 0 && (
                      <div className="text-xs text-amber-500">{fmt(r.pending_amount)}</div>
                    )}
                  </td>
                  <td className="px-5 py-3 text-center">
                    <span className={`font-medium ${r.rejected_count > 0 ? 'text-red-600' : 'text-gray-400'}`}>{r.rejected_count}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ─── EA Form Tab ───

function EaFormTab({
  data,
  isLoading,
  year,
}: {
  data: import('@/types').EaEmployeeSummary[] | undefined;
  isLoading: boolean;
  year: number;
}) {
  const [downloading, setDownloading] = useState<string | null>(null);

  const handleDownload = async (employeeId: string) => {
    setDownloading(employeeId);
    try {
      await downloadEaForm(year, employeeId);
    } finally {
      setDownloading(null);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-48">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <h2 className="font-semibold text-gray-900">EA Form (Borang EA) — {year}</h2>
      {!data || data.length === 0 ? (
        <div className="text-center py-16 text-gray-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" />
          <p className="text-sm">No payroll data for {year}</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 text-gray-500 text-xs uppercase tracking-wide">
                <th className="text-left px-5 py-3 font-medium">Employee</th>
                <th className="text-left px-5 py-3 font-medium">IC Number</th>
                <th className="text-center px-5 py-3 font-medium">Months</th>
                <th className="text-right px-5 py-3 font-medium">YTD Gross</th>
                <th className="text-right px-5 py-3 font-medium">Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-50">
              {data.map((emp) => (
                <tr key={emp.employee_id} className="hover:bg-gray-50/50">
                  <td className="px-5 py-3">
                    <div className="font-semibold text-gray-900">{emp.employee_name}</div>
                    <div className="text-xs text-gray-400">{emp.employee_number}</div>
                  </td>
                  <td className="px-5 py-3 text-gray-500">{emp.ic_number || '\u2014'}</td>
                  <td className="px-5 py-3 text-center text-gray-600">{emp.months_worked}</td>
                  <td className="px-5 py-3 text-right font-medium text-gray-900">{fmt(emp.ytd_gross)}</td>
                  <td className="px-5 py-3 text-right">
                    <button
                      onClick={() => handleDownload(emp.employee_id)}
                      disabled={downloading === emp.employee_id}
                      className="btn-secondary !py-1 !px-2.5 !text-xs disabled:opacity-50"
                    >
                      <Download className="w-3 h-3" />
                      {downloading === emp.employee_id ? 'Generating...' : 'Download PDF'}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ─── Shared Components ───

function SummaryField({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 mt-0.5">{value}</p>
    </div>
  );
}

function SummaryCard({ label, value, className }: { label: string; value: string; className?: string }) {
  return (
    <div className="bg-white rounded-xl border border-gray-200 px-4 py-3">
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className={`text-sm font-semibold mt-1 ${className || 'text-gray-900'}`}>{value}</p>
    </div>
  );
}
