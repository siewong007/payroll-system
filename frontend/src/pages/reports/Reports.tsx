import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useQuery } from '@tanstack/react-query';
import { FileText, Download } from 'lucide-react';
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
  type PayrollSummaryRow,
  type DepartmentPayrollRow,
  type StatutoryReportRow,
} from '@/api/reports';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { downloadCsv, senToPlainRm } from '@/lib/csv';
import { canAccessPayrollData } from '@/lib/roles';
import { formatMYR, monthName } from '@/lib/format';
import { SummaryField } from './ReportShared';
import { LeaveEntitlementReport } from './LeaveEntitlementReport';
import { ClaimsSummaryReport } from './ClaimsSummaryReport';
import { EaFormTab } from './EaFormTab';
import { getLeaveEligibility } from './leaveReportHelpers';

const fmt = formatMYR;
const monthShort = (month: number) => monthName(month, 'short');

type ReportTab = 'payroll' | 'department' | 'statutory' | 'leave' | 'claims' | 'ea_form';

const payrollColumns = (t: TFunction): Column<PayrollSummaryRow>[] => [
  { key: 'period', header: t('reports.cols.period'), render: (r) => <span className="font-semibold text-gray-900">{r.period}</span> },
  { key: 'employees', header: t('reports.cols.employees'), align: 'right', render: (r) => r.employee_count },
  { key: 'gross', header: t('reports.cols.gross'), align: 'right', render: (r) => fmt(r.total_gross) },
  { key: 'net', header: t('reports.cols.net'), align: 'right', render: (r) => fmt(r.total_net) },
  // i18n-ok: EPF/SOCSO/EIS/PCB are fixed Malaysian statutory acronyms.
  { key: 'epf_ee', header: t('reports.cols.epfEe'), align: 'right', render: (r) => fmt(r.total_epf_employee) },
  { key: 'epf_er', header: t('reports.cols.epfEr'), align: 'right', render: (r) => fmt(r.total_epf_employer) },
  { key: 'socso', header: 'SOCSO', align: 'right', render: (r) => fmt(r.total_socso_employee + r.total_socso_employer) },
  { key: 'pcb', header: 'PCB', align: 'right', render: (r) => fmt(r.total_pcb) },
  { key: 'cost', header: t('reports.cols.totalCost'), align: 'right', render: (r) => <span className="font-semibold">{fmt(r.total_employer_cost)}</span> },
];

const deptColumns = (t: TFunction): Column<DepartmentPayrollRow>[] => [
  { key: 'department', header: t('common.department'), render: (r) => <span className="font-semibold">{r.department || t('common.unassigned')}</span> },
  { key: 'employees', header: t('reports.cols.employees'), align: 'right', render: (r) => r.employee_count },
  { key: 'gross', header: t('reports.cols.totalGross'), align: 'right', render: (r) => fmt(r.total_gross) },
  { key: 'net', header: t('reports.cols.totalNet'), align: 'right', render: (r) => fmt(r.total_net) },
  { key: 'cost', header: t('reports.cols.employerCost'), align: 'right', render: (r) => <span className="font-semibold">{fmt(r.total_employer_cost)}</span> },
];

const statutoryColumns = (t: TFunction): Column<StatutoryReportRow>[] => [
  {
    key: 'employee', header: t('common.employee'), render: (r) => (
      <div>
        <div className="font-semibold text-gray-900">{r.employee_name}</div>
        <div className="text-xs text-gray-400">{r.employee_number}</div>
      </div>
    ),
  },
  { key: 'epf_no', header: t('reports.cols.epfNo'), render: (r) => <span className="text-gray-500">{r.epf_number || '\u2014'}</span> },
  { key: 'gross', header: t('reports.cols.gross'), align: 'right', render: (r) => fmt(r.gross_salary) },
  { key: 'epf_ee', header: t('reports.cols.epfEe'), align: 'right', render: (r) => fmt(r.epf_employee) },
  { key: 'epf_er', header: t('reports.cols.epfEr'), align: 'right', render: (r) => fmt(r.epf_employer) },
  { key: 'socso_ee', header: t('reports.cols.socsoEe'), align: 'right', render: (r) => fmt(r.socso_employee) },
  { key: 'socso_er', header: t('reports.cols.socsoEr'), align: 'right', render: (r) => fmt(r.socso_employer) },
  { key: 'eis', header: 'EIS', align: 'right', render: (r) => fmt(r.eis_employee + r.eis_employer) },
  { key: 'pcb', header: 'PCB', align: 'right', render: (r) => fmt(r.pcb_amount) },
  { key: 'zakat', header: t('reports.cols.zakat'), align: 'right', render: (r) => fmt(r.zakat_amount) },
];

const PAYROLL_TABS: ReportTab[] = ['payroll', 'department', 'statutory', 'ea_form'];

export function Reports() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user);
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

  // `@/lib/csv` is the single escaping authority — do not pre-quote a value on
  // the way in, or the helper doubles it. It mirrors the backend's
  // `csv_helpers`, so a report downloaded here and the same data pulled from the
  // API escape identically.
  const exportCSV = (headers: string[], rows: unknown[][], filename: string) =>
    downloadCsv(filename, headers, rows);

  const tabs = useMemo(() => {
    const all: { key: ReportTab; label: string }[] = [
      { key: 'payroll', label: t('reports.tabs.payroll') },
      { key: 'department', label: t('reports.tabs.department') },
      { key: 'statutory', label: t('reports.tabs.statutory') },
      { key: 'ea_form', label: t('reports.tabs.eaForm') },
      { key: 'leave', label: t('reports.tabs.leave') },
      { key: 'claims', label: t('reports.tabs.claims') },
    ];
    return canViewPayroll ? all : all.filter((tabDef) => !PAYROLL_TABS.includes(tabDef.key));
  }, [canViewPayroll, t]);

  useEffect(() => {
    if (!tabs.some((option) => option.key === tab)) {
      setTab(canViewPayroll ? 'payroll' : 'leave');
    }
  }, [canViewPayroll, tab, tabs]);

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">{t('reports.title')}</h1>
          <p className="page-subtitle">
            {canViewPayroll ? t('reports.subtitlePayroll') : t('reports.subtitleHr')}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select value={year} onChange={(e) => setYear(Number(e.target.value))} className="form-input !w-auto">
            {yearOptions.map((optionYear) => <option key={optionYear} value={optionYear}>{optionYear}</option>)}
          </select>
          {(tab === 'department' || tab === 'statutory') && (
            <select value={month} onChange={(e) => setMonth(Number(e.target.value))} className="form-input !w-auto">
              {monthOptions.map((optionMonth) => <option key={optionMonth} value={optionMonth}>{monthShort(optionMonth)}</option>)}
            </select>
          )}
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-gray-200 overflow-x-auto">
        {tabs.map((tabDef) => (
          <button
            key={tabDef.key}
            onClick={() => setTab(tabDef.key)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 whitespace-nowrap transition-all-fast ${
              tab === tabDef.key ? 'border-black text-gray-900' : 'border-transparent text-gray-400 hover:text-gray-700'
            }`}
          >
            {tabDef.label}
          </button>
        ))}
      </div>

      {/* Payroll Summary */}
      {tab === 'payroll' && (
        <div className="space-y-3">
          {payrollQuery.data && payrollQuery.data.length > 0 && (
            <div className="flex justify-between items-center">
              <h2 className="font-semibold text-gray-900">{t('reports.payrollSummaryTitle', { year })}</h2>
              <button
                onClick={() =>
                  exportCSV(
                    // i18n-ok: CSV file content for downstream tooling, not UI copy.
                    ['Period', 'Employees', 'Gross', 'Net', 'EPF(EE)', 'EPF(ER)', 'SOCSO(EE)', 'SOCSO(ER)', 'EIS(EE)', 'EIS(ER)', 'PCB', 'Zakat', 'Employer Cost'],
                    payrollQuery.data!.map((r) => [
                      r.period, r.employee_count, senToPlainRm(r.total_gross), senToPlainRm(r.total_net),
                      senToPlainRm(r.total_epf_employee), senToPlainRm(r.total_epf_employer),
                      senToPlainRm(r.total_socso_employee), senToPlainRm(r.total_socso_employer),
                      senToPlainRm(r.total_eis_employee), senToPlainRm(r.total_eis_employer),
                      senToPlainRm(r.total_pcb), senToPlainRm(r.total_zakat), senToPlainRm(r.total_employer_cost),
                    ]),
                    `payroll-summary-${year}.csv`
                  )
                }
                className="btn-secondary !py-1.5 !px-3 !text-xs"
              >
                <Download className="w-3.5 h-3.5" /> {t('reports.exportCsv')}
              </button>
            </div>
          )}
          <DataTable
            columns={payrollColumns(t)}
            data={payrollQuery.data ?? []}
            perPage={12}
            isLoading={payrollQuery.isLoading}
            emptyMessage={t('reports.noPayrollData', { year })}
            emptyIcon={<FileText className="w-8 h-8 text-gray-200" />}
            rowKey={(r) => r.period}
            summaryTitle={(r) => t('reports.payrollForPeriod', { period: r.period })}
            renderSummary={(r) => (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label={t('reports.cols.period')} value={r.period} />
                <SummaryField label={t('reports.cols.employees')} value={String(r.employee_count)} />
                <SummaryField label={t('reports.cols.totalGross')} value={fmt(r.total_gross)} />
                <SummaryField label={t('reports.cols.totalNet')} value={fmt(r.total_net)} />
                <SummaryField label={t('reports.cols.epfEmployee')} value={fmt(r.total_epf_employee)} />
                <SummaryField label={t('reports.cols.epfEmployer')} value={fmt(r.total_epf_employer)} />
                <SummaryField label={t('reports.cols.socsoEmployee')} value={fmt(r.total_socso_employee)} />
                <SummaryField label={t('reports.cols.socsoEmployer')} value={fmt(r.total_socso_employer)} />
                <SummaryField label={t('reports.cols.eisEmployee')} value={fmt(r.total_eis_employee)} />
                <SummaryField label={t('reports.cols.eisEmployer')} value={fmt(r.total_eis_employer)} />
                <SummaryField label="PCB" value={fmt(r.total_pcb)} />
                <SummaryField label={t('reports.cols.zakat')} value={fmt(r.total_zakat)} />
                <div className="col-span-2 pt-2 border-t border-gray-100">
                  <SummaryField label={t('reports.cols.totalEmployerCost')} value={fmt(r.total_employer_cost)} />
                </div>
              </div>
            )}
          />
        </div>
      )}

      {/* Department Breakdown */}
      {tab === 'department' && (
        <div className="space-y-3">
          <h2 className="font-semibold text-gray-900">{t('reports.deptTitle', { month: monthShort(month), year })}</h2>
          <DataTable
            columns={deptColumns(t)}
            data={deptQuery.data ?? []}
            perPage={10}
            isLoading={deptQuery.isLoading}
            emptyMessage={t('common.noData')}
            rowKey={(r) => r.department || 'unassigned'}  // i18n-ok: key, not display
            summaryTitle={(r) => t('reports.deptSummaryTitle', { department: r.department || t('common.unassigned') })}
          />
        </div>
      )}

      {/* Statutory Report */}
      {tab === 'statutory' && (
        <div className="space-y-3">
          {/* Statutory Export Buttons */}
          <div className="bg-white rounded-xl border border-gray-200 p-4">
            <h3 className="text-sm font-medium text-gray-700 mb-3">{t('reports.downloadSubmission', { month: monthShort(month), year })}</h3>
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
            <h2 className="font-semibold text-gray-900">{t('reports.statutoryTitle', { month: monthShort(month), year })}</h2>
            {statutoryQuery.data && statutoryQuery.data.length > 0 && (
              <button
                onClick={() =>
                  exportCSV(
                    // i18n-ok: CSV file content for downstream tooling, not UI copy.
                    ['Employee', 'ID', 'IC', 'EPF No', 'Gross', 'EPF(EE)', 'EPF(ER)', 'SOCSO(EE)', 'SOCSO(ER)', 'EIS(EE)', 'EIS(ER)', 'PCB', 'Zakat'],
                    statutoryQuery.data!.map((r) => [
                      r.employee_name, r.employee_number, r.ic_number, r.epf_number,
                      senToPlainRm(r.gross_salary), senToPlainRm(r.epf_employee), senToPlainRm(r.epf_employer),
                      senToPlainRm(r.socso_employee), senToPlainRm(r.socso_employer),
                      senToPlainRm(r.eis_employee), senToPlainRm(r.eis_employer),
                      senToPlainRm(r.pcb_amount), senToPlainRm(r.zakat_amount),
                    ]),
                    `statutory-${year}-${String(month).padStart(2, '0')}.csv`
                  )
                }
                className="btn-secondary !py-1.5 !px-3 !text-xs"
              >
                <Download className="w-3.5 h-3.5" /> {t('reports.exportCsv')}
              </button>
            )}
          </div>
          <DataTable
            columns={statutoryColumns(t)}
            data={statutoryQuery.data ?? []}
            perPage={10}
            isLoading={statutoryQuery.isLoading}
            emptyMessage={t('common.noData')}
            rowKey={(r) => r.employee_number}
            summaryTitle={(r) => t('reports.statutorySummaryTitle', { name: r.employee_name })}
            renderSummary={(r) => (
              <div className="grid grid-cols-2 gap-4">
                <SummaryField label={t('common.employee')} value={r.employee_name} />
                <SummaryField label={t('common.id')} value={r.employee_number} />
                <SummaryField label={t('reports.cols.icNumber')} value={r.ic_number || '\u2014'} />
                <SummaryField label={t('reports.cols.epfNumber')} value={r.epf_number || '\u2014'} />
                <SummaryField label={t('reports.cols.grossSalary')} value={fmt(r.gross_salary)} />
                <SummaryField label={t('reports.cols.basicSalary')} value={fmt(r.basic_salary)} />
                <SummaryField label={t('reports.cols.epfEmployee')} value={fmt(r.epf_employee)} />
                <SummaryField label={t('reports.cols.epfEmployer')} value={fmt(r.epf_employer)} />
                <SummaryField label={t('reports.cols.socsoEmployee')} value={fmt(r.socso_employee)} />
                <SummaryField label={t('reports.cols.socsoEmployer')} value={fmt(r.socso_employer)} />
                <SummaryField label={t('reports.cols.eisEmployee')} value={fmt(r.eis_employee)} />
                <SummaryField label={t('reports.cols.eisEmployer')} value={fmt(r.eis_employer)} />
                <SummaryField label="PCB" value={fmt(r.pcb_amount)} />
                <SummaryField label={t('reports.cols.zakat')} value={fmt(r.zakat_amount)} />
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
              // i18n-ok: CSV file content for downstream tooling, not UI copy.
              ['Employee', 'ID', 'Department', 'Leave Type', 'Status', 'Entitled', 'Taken', 'Pending', 'Balance'],
              leaveQuery.data.map((r) => {
                const elig = getLeaveEligibility(t, r.leave_type_name, r.gender, r.marital_status, r.num_children);
                return [
                  r.employee_name, r.employee_number, r.department,
                  // i18n-ok: CSV cell values, not UI copy.
                  r.leave_type_name, elig.eligible ? 'Eligible' : elig.reason || 'Not eligible',
                  r.entitled_days, r.taken_days, r.pending_days, r.balance,
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
              // i18n-ok: CSV file content for downstream tooling, not UI copy.
              ['Employee', 'ID', 'Department', 'Total Claims', 'Total Amount', 'Approved', 'Approved Amount', 'Pending', 'Pending Amount', 'Rejected'],
              claimsQuery.data.map((r) => [
                r.employee_name, r.employee_number, r.department,
                r.total_claims, senToPlainRm(r.total_amount),
                r.approved_count, senToPlainRm(r.approved_amount),
                r.pending_count, senToPlainRm(r.pending_amount),
                r.rejected_count,
              ]),
              `claims-report-${year}.csv`
            );
          }}
        />
      )}
    </div>
  );
}
