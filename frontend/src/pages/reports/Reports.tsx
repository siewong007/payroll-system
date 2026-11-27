import { type KeyboardEvent, lazy, Suspense, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, Download, FileText, Loader2 } from 'lucide-react';
import { usePermissions } from '@/lib/usePermissions';
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
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { downloadCsv, senToPlainRm } from '@/lib/csv';
import { formatMYR, formatNumber, monthName } from '@/lib/format';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import { SummaryField } from './ReportShared';
const LeaveEntitlementReport = lazy(() =>
  import('./LeaveEntitlementReport').then((m) => ({ default: m.LeaveEntitlementReport }))
);
const ClaimsSummaryReport = lazy(() =>
  import('./ClaimsSummaryReport').then((m) => ({ default: m.ClaimsSummaryReport }))
);
const EaFormTab = lazy(() =>
  import('./EaFormTab').then((m) => ({ default: m.EaFormTab }))
);
import { getLeaveEligibility } from './leaveReportHelpers';

const fmt = formatMYR;
const monthShort = (month: number) => monthName(month, 'short');

type ReportTab = 'payroll' | 'department' | 'statutory' | 'leave' | 'claims' | 'ea_form';
type StatutoryFileType = 'epf' | 'socso' | 'eis' | 'pcb';

const payrollColumns = (t: TFunction): Column<PayrollSummaryRow>[] => [
  {
    key: 'period',
    header: t('reports.cols.period'),
    primary: true,
    sortValue: (r) => r.period,
    render: (r) => (
      <span className="font-semibold text-gray-900 dark:text-slate-100 whitespace-nowrap">{r.period}</span>
    ),
  },
  {
    key: 'employees',
    header: t('reports.cols.employees'),
    align: 'right',
    sortValue: (r) => r.employee_count,
    render: (r) => <span className="tabular-nums">{formatNumber(r.employee_count)}</span>,
  },
  {
    key: 'gross',
    header: t('reports.cols.gross'),
    align: 'right',
    sortValue: (r) => r.total_gross,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_gross)}</span>,
  },
  {
    key: 'net',
    header: t('reports.cols.net'),
    align: 'right',
    sortValue: (r) => r.total_net,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_net)}</span>,
  },
  {
    key: 'epf_ee',
    header: t('reports.cols.epfEe'),
    align: 'right',
    sortValue: (r) => r.total_epf_employee,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_epf_employee)}</span>,
  },
  {
    key: 'epf_er',
    header: t('reports.cols.epfEr'),
    align: 'right',
    sortValue: (r) => r.total_epf_employer,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_epf_employer)}</span>,
  },
  {
    key: 'socso',
    header: t('reports.cols.socso'),
    align: 'right',
    sortValue: (r) => r.total_socso_employee + r.total_socso_employer,
    render: (r) => (
      <span className="tabular-nums whitespace-nowrap">
        {fmt(r.total_socso_employee + r.total_socso_employer)}
      </span>
    ),
  },
  {
    key: 'pcb',
    header: t('reports.cols.pcb'),
    align: 'right',
    sortValue: (r) => r.total_pcb,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_pcb)}</span>,
  },
  {
    key: 'cost',
    header: t('reports.cols.totalCost'),
    align: 'right',
    sortValue: (r) => r.total_employer_cost,
    render: (r) => (
      <span className="font-semibold tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
        {fmt(r.total_employer_cost)}
      </span>
    ),
  },
];

const deptColumns = (t: TFunction): Column<DepartmentPayrollRow>[] => [
  {
    key: 'department',
    header: t('common.department'),
    primary: true,
    sortValue: (r) => r.department || t('common.unassigned'),
    render: (r) => (
      <span className="font-semibold text-gray-900 dark:text-slate-100">
        {r.department || t('common.unassigned')}
      </span>
    ),
  },
  {
    key: 'employees',
    header: t('reports.cols.employees'),
    align: 'right',
    sortValue: (r) => r.employee_count,
    render: (r) => <span className="tabular-nums">{formatNumber(r.employee_count)}</span>,
  },
  {
    key: 'gross',
    header: t('reports.cols.totalGross'),
    align: 'right',
    sortValue: (r) => r.total_gross,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_gross)}</span>,
  },
  {
    key: 'net',
    header: t('reports.cols.totalNet'),
    align: 'right',
    sortValue: (r) => r.total_net,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.total_net)}</span>,
  },
  {
    key: 'cost',
    header: t('reports.cols.employerCost'),
    align: 'right',
    sortValue: (r) => r.total_employer_cost,
    render: (r) => (
      <span className="font-semibold tabular-nums whitespace-nowrap text-gray-900 dark:text-slate-100">
        {fmt(r.total_employer_cost)}
      </span>
    ),
  },
];

const statutoryColumns = (t: TFunction): Column<StatutoryReportRow>[] => [
  {
    key: 'employee',
    header: t('common.employee'),
    primary: true,
    sortValue: (r) => r.employee_name,
    render: (r) => (
      <div>
        <div className="font-semibold text-gray-900 dark:text-slate-100">{r.employee_name}</div>
        <div className="text-xs text-gray-500 dark:text-slate-400">{r.employee_number}</div>
      </div>
    ),
  },
  {
    key: 'epf_no',
    header: t('reports.cols.epfNo'),
    sortValue: (r) => r.epf_number ?? '',
    render: (r) => (
      <span className="text-gray-600 dark:text-slate-300 tabular-nums whitespace-nowrap">
        {r.epf_number || '\u2014'}
      </span>
    ),
  },
  {
    key: 'gross',
    header: t('reports.cols.gross'),
    align: 'right',
    sortValue: (r) => r.gross_salary,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.gross_salary)}</span>,
  },
  {
    key: 'epf_ee',
    header: t('reports.cols.epfEe'),
    align: 'right',
    sortValue: (r) => r.epf_employee,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.epf_employee)}</span>,
  },
  {
    key: 'epf_er',
    header: t('reports.cols.epfEr'),
    align: 'right',
    sortValue: (r) => r.epf_employer,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.epf_employer)}</span>,
  },
  {
    key: 'socso_ee',
    header: t('reports.cols.socsoEe'),
    align: 'right',
    sortValue: (r) => r.socso_employee,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.socso_employee)}</span>,
  },
  {
    key: 'socso_er',
    header: t('reports.cols.socsoEr'),
    align: 'right',
    sortValue: (r) => r.socso_employer,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.socso_employer)}</span>,
  },
  {
    key: 'eis',
    header: t('reports.cols.eis'),
    align: 'right',
    sortValue: (r) => r.eis_employee + r.eis_employer,
    render: (r) => (
      <span className="tabular-nums whitespace-nowrap">{fmt(r.eis_employee + r.eis_employer)}</span>
    ),
  },
  {
    key: 'pcb',
    header: t('reports.cols.pcb'),
    align: 'right',
    sortValue: (r) => r.pcb_amount,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.pcb_amount)}</span>,
  },
  {
    key: 'zakat',
    header: t('reports.cols.zakat'),
    align: 'right',
    sortValue: (r) => r.zakat_amount,
    render: (r) => <span className="tabular-nums whitespace-nowrap">{fmt(r.zakat_amount)}</span>,
  },
];

export function Reports() {
  const { t } = useTranslation();
  const toast = useToast();
  const tabsIdPrefix = useId();
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const { can } = usePermissions();
  const canViewPayroll = can('view_payroll');
  const canViewStatutory = can('view_statutory_exports');
  const [searchParams, setSearchParams] = useSearchParams();
  const [downloadingExport, setDownloadingExport] = useState<StatutoryFileType | null>(null);

  const tabs = useMemo(() => {
    const list: { key: ReportTab; label: string }[] = [];
    if (canViewPayroll) {
      list.push({ key: 'payroll', label: t('reports.tabs.payroll') });
      list.push({ key: 'department', label: t('reports.tabs.department') });
    }
    if (canViewStatutory) {
      list.push({ key: 'statutory', label: t('reports.tabs.statutory') });
      list.push({ key: 'ea_form', label: t('reports.tabs.eaForm') });
    }
    list.push({ key: 'leave', label: t('reports.tabs.leave') });
    list.push({ key: 'claims', label: t('reports.tabs.claims') });
    return list;
  }, [canViewPayroll, canViewStatutory, t]);

  const defaultTab = tabs[0]?.key ?? 'leave';
  const rawTab = searchParams.get('tab') as ReportTab | null;
  const tab: ReportTab = rawTab && tabs.some((item) => item.key === rawTab) ? rawTab : defaultTab;

  const today = new Date();
  const fallbackYear = today.getFullYear();
  const fallbackMonth = today.getMonth() + 1;

  const parsedUrlYear = Number(searchParams.get('year'));
  const hasValidUrlYear = Number.isInteger(parsedUrlYear) && parsedUrlYear >= 2000 && parsedUrlYear <= 2100;
  const parsedUrlMonth = Number(searchParams.get('month'));
  const hasValidUrlMonth = Number.isInteger(parsedUrlMonth) && parsedUrlMonth >= 1 && parsedUrlMonth <= 12;

  const [year, setYearState] = useState(() => (hasValidUrlYear ? parsedUrlYear : fallbackYear));
  const [month, setMonthState] = useState(() => (hasValidUrlMonth ? parsedUrlMonth : fallbackMonth));
  const [hasInitializedPeriods, setHasInitializedPeriods] = useState(hasValidUrlYear || hasValidUrlMonth);

  const updateParams = (patch: { tab?: ReportTab; year?: number; month?: number }) => {
    const next = new URLSearchParams(searchParams);
    if (patch.tab !== undefined) next.set('tab', patch.tab);
    if (patch.year !== undefined) next.set('year', String(patch.year));
    if (patch.month !== undefined) next.set('month', String(patch.month));
    setSearchParams(next, { replace: true });
  };

  const handleTabChange = (nextTab: ReportTab) => {
    updateParams({ tab: nextTab });
  };

  const handleTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>, currentIndex: number) => {
    let targetIndex: number | null = null;
    if (event.key === 'ArrowRight') {
      targetIndex = (currentIndex + 1) % tabs.length;
    } else if (event.key === 'ArrowLeft') {
      targetIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    } else if (event.key === 'Home') {
      targetIndex = 0;
    } else if (event.key === 'End') {
      targetIndex = tabs.length - 1;
    }
    if (targetIndex === null) return;
    event.preventDefault();
    const targetTab = tabs[targetIndex];
    if (!targetTab) return;
    handleTabChange(targetTab.key);
    tabRefs.current[targetIndex]?.focus();
  };

  const handleYearChange = (nextYear: number) => {
    setYearState(nextYear);
    updateParams({ year: nextYear });
  };

  const handleMonthChange = (nextMonth: number) => {
    setMonthState(nextMonth);
    updateParams({ month: nextMonth });
  };

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

    setYearState(periodsQuery.data.default_year);
    setMonthState(periodsQuery.data.default_month);
    setHasInitializedPeriods(true);
  }, [hasInitializedPeriods, periodsQuery.data]);

  useEffect(() => {
    if (!periodsQuery.data || yearOptions.length === 0 || yearOptions.includes(year)) {
      return;
    }

    const clampedYear = yearOptions[yearOptions.length - 1];
    setYearState(clampedYear);
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set('year', String(clampedYear));
        return next;
      },
      { replace: true },
    );
  }, [periodsQuery.data, setSearchParams, year, yearOptions]);

  useEffect(() => {
    if (
      !periodsQuery.data ||
      !['department', 'statutory'].includes(tab) ||
      monthOptions.length === 0 ||
      monthOptions.includes(month)
    ) {
      return;
    }

    const clampedMonth = monthOptions[monthOptions.length - 1];
    setMonthState(clampedMonth);
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set('month', String(clampedMonth));
        return next;
      },
      { replace: true },
    );
  }, [month, monthOptions, periodsQuery.data, setSearchParams, tab]);

  const payrollQuery = useQuery({
    queryKey: ['report-payroll', year],
    queryFn: () => getPayrollSummary(year),
    enabled: tab === 'payroll' && canViewPayroll,
  });

  const deptQuery = useQuery({
    queryKey: ['report-dept', year, month],
    queryFn: () => getPayrollByDepartment(year, month),
    enabled: tab === 'department' && canViewPayroll,
  });

  const statutoryQuery = useQuery({
    queryKey: ['report-statutory', year, month],
    queryFn: () => getStatutoryReport(year, month),
    enabled: tab === 'statutory' && canViewStatutory,
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
    enabled: tab === 'ea_form' && canViewStatutory,
  });

  // `@/lib/csv` is the single escaping authority — do not pre-quote a value on
  // the way in, or the helper doubles it. It mirrors the backend's
  // `csv_helpers`, so a report downloaded here and the same data pulled from the
  // API escape identically.
  const exportCSV = (headers: string[], rows: unknown[][], filename: string) =>
    downloadCsv(filename, headers, rows);

  const handleStatutoryExport = async (type: StatutoryFileType) => {
    setDownloadingExport(type);
    try {
      await downloadStatutoryExport(type, year, month);
    } catch (err) {
      toast.error(getErrorMessage(err, t('reports.downloadFailed')));
    } finally {
      setDownloadingExport(null);
    }
  };

  const subtitle = canViewPayroll || canViewStatutory ? t('reports.subtitlePayroll') : t('reports.subtitleHr');
  const activeTabId = `${tabsIdPrefix}-tab-${tab}`;
  const activePanelId = `${tabsIdPrefix}-panel-${tab}`;

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('reports.title')}
        subtitle={subtitle}
        actions={
          <div className="no-print flex items-center gap-2">
            <select
              value={year}
              onChange={(e) => handleYearChange(Number(e.target.value))}
              className="form-input !w-auto min-h-11 sm:min-h-10"
              aria-label={t('reports.filterYear')}
            >
              {yearOptions.map((optionYear) => (
                <option key={optionYear} value={optionYear}>{optionYear}</option>
              ))}
            </select>
            {(tab === 'department' || tab === 'statutory') && (
              <select
                value={month}
                onChange={(e) => handleMonthChange(Number(e.target.value))}
                className="form-input !w-auto min-h-11 sm:min-h-10"
                aria-label={t('reports.filterMonth')}
              >
                {monthOptions.map((optionMonth) => (
                  <option key={optionMonth} value={optionMonth}>{monthShort(optionMonth)}</option>
                ))}
              </select>
            )}
          </div>
        }
      />

      {periodsQuery.isError && (
        <div
          className="no-print rounded-xl border border-amber-200 dark:border-amber-900/50 bg-amber-50 dark:bg-amber-950/30 px-4 py-3 text-sm text-amber-800 dark:text-amber-200 flex flex-wrap items-center justify-between gap-2"
          role="alert"
        >
          <span>{getErrorMessage(periodsQuery.error, t('reports.periodsLoadFailed'))}</span>
          <button type="button" onClick={() => void periodsQuery.refetch()} className="btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 text-xs">
            {t('common.retry')}
          </button>
        </div>
      )}

      {/* Tabs */}
      <div
        role="tablist"
        aria-label={t('reports.tabsLabel')}
        className="no-print flex gap-1 border-b border-gray-200 dark:border-slate-800 overflow-x-auto"
      >
        {tabs.map((tabDef, idx) => {
          const isSelected = tab === tabDef.key;
          return (
            <button
              key={tabDef.key}
              ref={(el) => {
                tabRefs.current[idx] = el;
              }}
              id={`${tabsIdPrefix}-tab-${tabDef.key}`}
              type="button"
              role="tab"
              aria-selected={isSelected}
              tabIndex={isSelected ? 0 : -1}
              aria-controls={isSelected ? activePanelId : undefined}
              onClick={() => handleTabChange(tabDef.key)}
              onKeyDown={(e) => handleTabKeyDown(e, idx)}
              className={`min-h-11 px-4 py-2.5 text-sm font-medium border-b-2 whitespace-nowrap shrink-0 transition-all-fast focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                isSelected
                  ? 'border-[var(--accent-1)] text-gray-900 dark:text-slate-100'
                  : 'border-transparent text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
              }`}
            >
              {tabDef.label}
            </button>
          );
        })}
      </div>

      <div id={activePanelId} role="tabpanel" aria-labelledby={activeTabId}>
        {/* Payroll Summary */}
        {tab === 'payroll' && canViewPayroll && (
          <div className="space-y-3">
            {payrollQuery.isError ? (
              <div className="card">
                <EmptyState
                  icon={AlertTriangle}
                  title={t('reports.loadFailed')}
                  description={getErrorMessage(payrollQuery.error, t('reports.loadFailed'))}
                  action={
                    <button type="button" onClick={() => void payrollQuery.refetch()} className="btn-primary">
                      {t('common.retry')}
                    </button>
                  }
                />
              </div>
            ) : (
              <>
                {payrollQuery.data && payrollQuery.data.length > 0 && (
                  <div className="flex flex-wrap justify-between items-center gap-2">
                    <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('reports.payrollSummaryTitle', { year })}</h2>
                    <button
                      type="button"
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
                      className="no-print btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 !text-xs"
                    >
                      <Download className="w-3.5 h-3.5" aria-hidden="true" /> {t('reports.exportCsv')}
                    </button>
                  </div>
                )}
                <DataTable
                  columns={payrollColumns(t)}
                  data={payrollQuery.data ?? []}
                  perPage={12}
                  isLoading={payrollQuery.isLoading}
                  emptyMessage={t('reports.noPayrollData', { year })}
                  emptyIcon={<FileText className="w-8 h-8 text-gray-300 dark:text-slate-600" aria-hidden="true" />}
                  rowKey={(r, i) => `${r.period}-${i}`}
                  summaryTitle={(r) => t('reports.payrollForPeriod', { period: r.period })}
                  renderSummary={(r) => (
                    <div className="grid grid-cols-2 gap-4">
                      <SummaryField label={t('reports.cols.period')} value={r.period} />
                      <SummaryField label={t('reports.cols.employees')} value={formatNumber(r.employee_count)} />
                      <SummaryField label={t('reports.cols.totalGross')} value={fmt(r.total_gross)} />
                      <SummaryField label={t('reports.cols.totalNet')} value={fmt(r.total_net)} />
                      <SummaryField label={t('reports.cols.epfEmployee')} value={fmt(r.total_epf_employee)} />
                      <SummaryField label={t('reports.cols.epfEmployer')} value={fmt(r.total_epf_employer)} />
                      <SummaryField label={t('reports.cols.socsoEmployee')} value={fmt(r.total_socso_employee)} />
                      <SummaryField label={t('reports.cols.socsoEmployer')} value={fmt(r.total_socso_employer)} />
                      <SummaryField label={t('reports.cols.eisEmployee')} value={fmt(r.total_eis_employee)} />
                      <SummaryField label={t('reports.cols.eisEmployer')} value={fmt(r.total_eis_employer)} />
                      <SummaryField label={t('reports.cols.pcb')} value={fmt(r.total_pcb)} />
                      <SummaryField label={t('reports.cols.zakat')} value={fmt(r.total_zakat)} />
                      <div className="col-span-2 pt-2 border-t border-gray-100 dark:border-slate-800">
                        <SummaryField label={t('reports.cols.totalEmployerCost')} value={fmt(r.total_employer_cost)} />
                      </div>
                    </div>
                  )}
                />
              </>
            )}
          </div>
        )}

        {/* Department Breakdown */}
        {tab === 'department' && canViewPayroll && (
          <div className="space-y-3">
            {deptQuery.isError ? (
              <div className="card">
                <EmptyState
                  icon={AlertTriangle}
                  title={t('reports.loadFailed')}
                  description={getErrorMessage(deptQuery.error, t('reports.loadFailed'))}
                  action={
                    <button type="button" onClick={() => void deptQuery.refetch()} className="btn-primary">
                      {t('common.retry')}
                    </button>
                  }
                />
              </div>
            ) : (
              <>
                <div className="flex flex-wrap justify-between items-center gap-2">
                  <h2 className="font-semibold text-gray-900 dark:text-slate-100">
                    {t('reports.deptTitle', { month: monthShort(month), year })}
                  </h2>
                  {deptQuery.data && deptQuery.data.length > 0 && (
                    <button
                      type="button"
                      onClick={() =>
                        exportCSV(
                          // i18n-ok: CSV file content for downstream tooling, not UI copy.
                          ['Department', 'Employees', 'Gross', 'Net', 'Employer Cost'],
                          deptQuery.data!.map((r) => [
                            r.department || 'Unassigned',
                            r.employee_count,
                            senToPlainRm(r.total_gross),
                            senToPlainRm(r.total_net),
                            senToPlainRm(r.total_employer_cost),
                          ]),
                          `department-payroll-${year}-${String(month).padStart(2, '0')}.csv`
                        )
                      }
                      className="no-print btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 !text-xs"
                    >
                      <Download className="w-3.5 h-3.5" aria-hidden="true" /> {t('reports.exportCsv')}
                    </button>
                  )}
                </div>
                <DataTable
                  columns={deptColumns(t)}
                  data={deptQuery.data ?? []}
                  perPage={10}
                  isLoading={deptQuery.isLoading}
                  emptyMessage={t('common.noData')}
                  rowKey={(r) => r.department || 'unassigned'} // i18n-ok: key, not display
                  summaryTitle={(r) => t('reports.deptSummaryTitle', { department: r.department || t('common.unassigned') })}
                  renderSummary={(r) => (
                    <div className="grid grid-cols-2 gap-4">
                      <SummaryField label={t('common.department')} value={r.department || t('common.unassigned')} />
                      <SummaryField label={t('reports.cols.employees')} value={formatNumber(r.employee_count)} />
                      <SummaryField label={t('reports.cols.totalGross')} value={fmt(r.total_gross)} />
                      <SummaryField label={t('reports.cols.totalNet')} value={fmt(r.total_net)} />
                      <div className="col-span-2 pt-2 border-t border-gray-100 dark:border-slate-800">
                        <SummaryField label={t('reports.cols.employerCost')} value={fmt(r.total_employer_cost)} />
                      </div>
                    </div>
                  )}
                />
              </>
            )}
          </div>
        )}

        {/* Statutory Report */}
        {tab === 'statutory' && canViewStatutory && (
          <div className="space-y-3">
            {/* Statutory Export Buttons */}
            <div className="no-print bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 p-4">
              <h3 className="text-sm font-medium text-gray-700 dark:text-slate-200 mb-3">
                {t('reports.downloadSubmission', { month: monthShort(month), year })}
              </h3>
              <div className="flex flex-wrap gap-2">
                {(['epf', 'socso', 'eis', 'pcb'] as const).map((exportType) => (
                  <button
                    key={exportType}
                    type="button"
                    onClick={() => handleStatutoryExport(exportType)}
                    disabled={downloadingExport !== null}
                    className="btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 !text-xs disabled:opacity-50"
                  >
                    {downloadingExport === exportType ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <Download className="w-3.5 h-3.5" aria-hidden="true" />
                    )}
                    {downloadingExport === exportType
                      ? t('common.downloading')
                      : t(`reports.statutoryFiles.${exportType}`)}
                  </button>
                ))}
              </div>
            </div>

            {statutoryQuery.isError ? (
              <div className="card">
                <EmptyState
                  icon={AlertTriangle}
                  title={t('reports.loadFailed')}
                  description={getErrorMessage(statutoryQuery.error, t('reports.loadFailed'))}
                  action={
                    <button type="button" onClick={() => void statutoryQuery.refetch()} className="btn-primary">
                      {t('common.retry')}
                    </button>
                  }
                />
              </div>
            ) : (
              <>
                <div className="flex flex-wrap justify-between items-center gap-2">
                  <h2 className="font-semibold text-gray-900 dark:text-slate-100">
                    {t('reports.statutoryTitle', { month: monthShort(month), year })}
                  </h2>
                  {statutoryQuery.data && statutoryQuery.data.length > 0 && (
                    <button
                      type="button"
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
                      className="no-print btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 !text-xs"
                    >
                      <Download className="w-3.5 h-3.5" aria-hidden="true" /> {t('reports.exportCsv')}
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
                      <SummaryField label={t('reports.cols.pcb')} value={fmt(r.pcb_amount)} />
                      <SummaryField label={t('reports.cols.zakat')} value={fmt(r.zakat_amount)} />
                    </div>
                  )}
                />
              </>
            )}
          </div>
        )}

        {/* Leave Entitlement Report */}
        {tab === 'leave' && (
          <Suspense fallback={null}>
            <LeaveEntitlementReport
              data={leaveQuery.data}
              isLoading={leaveQuery.isLoading}
              isError={leaveQuery.isError}
              error={leaveQuery.error}
              onRetry={() => void leaveQuery.refetch()}
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
          </Suspense>
        )}

        {/* EA Form */}
        {tab === 'ea_form' && canViewStatutory && (
          <Suspense fallback={null}>
            <EaFormTab
              data={eaQuery.data}
              isLoading={eaQuery.isLoading}
              isError={eaQuery.isError}
              error={eaQuery.error}
              onRetry={() => void eaQuery.refetch()}
              year={year}
            />
          </Suspense>
        )}

        {/* Claims Report */}
        {tab === 'claims' && (
          <Suspense fallback={null}>
            <ClaimsSummaryReport
              data={claimsQuery.data}
              isLoading={claimsQuery.isLoading}
              isError={claimsQuery.isError}
              error={claimsQuery.error}
              onRetry={() => void claimsQuery.refetch()}
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
          </Suspense>
        )}
      </div>
    </div>
  );
}
