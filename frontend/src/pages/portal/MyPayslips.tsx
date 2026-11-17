import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, Printer, Download, FileText } from 'lucide-react';
import { getMyPayslips, getMyProfile, downloadPayslipPdf } from '@/api/portal';
import { formatMYR } from '@/lib/utils';
import { formatDateLong, formatPeriod, monthName } from '@/lib/format';
import { KpiSkeletonGrid, Skeleton } from '@/components/ui/Skeleton';
import { EmptyState } from '@/components/ui/EmptyState';
import type { MyPayslip, Employee } from '@/types';

const MONTH_NUMBERS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

export function MyPayslips() {
  const { t } = useTranslation();
  const [selectedPeriod, setSelectedPeriod] = useState<{ year: number; month: number } | null>(null);

  const { data: payslips, isLoading } = useQuery({
    queryKey: ['my-payslips'],
    queryFn: getMyPayslips,
  });

  const { data: profile } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  const sortedPayslips = useMemo(() => {
    return [...(payslips ?? [])].sort((a, b) =>
      b.period_year !== a.period_year ? b.period_year - a.period_year : b.period_month - a.period_month,
    );
  }, [payslips]);

  // Default to the latest available payslip so employees opening the page
  // mid-month before payroll runs see their most recent slip instead of a blank box.
  const activePeriod = useMemo(() => {
    if (selectedPeriod) return selectedPeriod;
    const now = new Date();
    const currentYear = now.getFullYear();
    const currentMonth = now.getMonth() + 1;
    const hasCurrent = sortedPayslips.some(
      (p) => p.period_year === currentYear && p.period_month === currentMonth,
    );
    if (hasCurrent || sortedPayslips.length === 0) {
      return { year: currentYear, month: currentMonth };
    }
    return {
      year: sortedPayslips[0].period_year,
      month: sortedPayslips[0].period_month,
    };
  }, [selectedPeriod, sortedPayslips]);

  const { year, month } = activePeriod;

  const currentPayslip = sortedPayslips.find(
    (p) => p.period_year === year && p.period_month === month,
  );

  const goBack = () => {
    setSelectedPeriod(month === 1 ? { year: year - 1, month: 12 } : { year, month: month - 1 });
  };

  const goForward = () => {
    setSelectedPeriod(month === 12 ? { year: year + 1, month: 1 } : { year, month: month + 1 });
  };

  if (isLoading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-8 w-48" />
        <KpiSkeletonGrid count={3} />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <p className="page-subtitle">{t('portal.payslips.title')}</p>
          <h1 className="page-title">{profile?.full_name || t('common.employee')}</h1>
        </div>
        {currentPayslip && (
          <div className="flex items-center gap-2 no-print">
            <button onClick={() => downloadPayslipPdf(currentPayslip.id)} className="btn-secondary">
              <Download className="w-4 h-4" /> {t('portal.payslips.downloadPdf')}
            </button>
            <button onClick={() => window.print()} className="btn-primary">
              <Printer className="w-4 h-4" /> {t('portal.payslips.print')}
            </button>
          </div>
        )}
      </div>

      {/* Period Navigation + Recent Available Pills */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between no-print">
        <div className="flex items-center gap-2">
          <button
            onClick={goBack}
            aria-label={t('common.previous')}
            className="p-2 rounded-xl hover:bg-white/70 dark:hover:bg-slate-800 transition-all-fast"
          >
            <ChevronLeft className="w-5 h-5 text-gray-500 dark:text-slate-400" />
          </button>
          <select
            value={month}
            aria-label={t('payroll.columns.period')}
            onChange={(e) => setSelectedPeriod({ year, month: Number(e.target.value) })}
            className="form-input !w-auto"
          >
            {MONTH_NUMBERS.map((m) => <option key={m} value={m}>{monthName(m)}</option>)}
          </select>
          <select
            value={year}
            aria-label={t('common.year')}
            onChange={(e) => setSelectedPeriod({ year: Number(e.target.value), month })}
            className="form-input !w-auto"
          >
            {Array.from({ length: 6 }, (_, i) => new Date().getFullYear() - 3 + i)
              .map((y) => <option key={y} value={y}>{y}</option>)}
          </select>
          <button
            onClick={goForward}
            aria-label={t('common.next')}
            className="p-2 rounded-xl hover:bg-white/70 dark:hover:bg-slate-800 transition-all-fast"
          >
            <ChevronRight className="w-5 h-5 text-gray-500 dark:text-slate-400" />
          </button>
        </div>

        {sortedPayslips.length > 0 && (
          <div className="flex items-center gap-1.5 overflow-x-auto pb-1 sm:pb-0">
            <span className="text-xs text-gray-400 dark:text-slate-500 mr-1 shrink-0">
              {t('ux.payslips.recentPeriods')}:
            </span>
            {sortedPayslips.slice(0, 5).map((p) => {
              const active = p.period_year === year && p.period_month === month;
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => setSelectedPeriod({ year: p.period_year, month: p.period_month })}
                  className={`shrink-0 rounded-full px-3 py-1 text-xs font-semibold transition-all ${
                    active
                      ? 'bg-gradient-to-r from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-xs'
                      : 'bg-white/80 dark:bg-slate-900/80 text-gray-600 dark:text-slate-300 ring-1 ring-gray-200/70 dark:ring-slate-800 hover:text-gray-900 dark:hover:text-white'
                  }`}
                >
                  {formatPeriod(p.period_year, p.period_month)}
                </button>
              );
            })}
          </div>
        )}
      </div>

      {!currentPayslip ? (
        <div className="card">
          <EmptyState icon={FileText} title={t('portal.payslips.empty')} />
        </div>
      ) : (
        <PayslipDetail payslip={currentPayslip} profile={profile} />
      )}
    </div>
  );
}

const Row = ({ label, amount, indent, bold, negative }: {
  label: string; amount: number; indent?: boolean; bold?: boolean; negative?: boolean;
}) => (
  <div className={`flex justify-between py-2 ${indent ? 'pl-4' : ''} ${bold ? 'font-semibold border-t border-gray-200 dark:border-slate-800 pt-3 mt-1' : ''}`}>
    <span className={`text-sm ${indent ? 'text-gray-400 italic' : 'text-gray-600 dark:text-slate-300'}`}>{label}</span>
    <span className={`text-sm tabular-nums ${bold ? 'font-bold text-gray-900 dark:text-slate-100' : 'font-medium text-gray-800 dark:text-slate-200'} ${negative ? '!text-red-600 dark:!text-red-400' : ''}`}>
      {negative ? '-' : ''}{formatMYR(Math.abs(amount))}
    </span>
  </div>
);

const SummaryRow = ({ label, value, highlight }: { label: string; value: string; highlight?: string }) => (
  <div className="flex justify-between py-2">
    <span className="text-sm text-gray-400 dark:text-slate-400">{label}</span>
    <span className={`text-sm font-medium tabular-nums text-gray-900 dark:text-slate-100 ${highlight || ''}`}>{value}</span>
  </div>
);

function PayslipDetail({ payslip, profile }: { payslip: MyPayslip; profile: Employee | undefined }) {
  const { t } = useTranslation();
  const otherEarnings = payslip.gross_salary - payslip.basic_salary
    - payslip.total_allowances - payslip.total_overtime
    - payslip.total_bonus - payslip.total_commission;

  const periodStart = formatDateLong(payslip.period_start);
  const periodEnd = formatDateLong(payslip.period_end);
  const payDate = formatDateLong(payslip.pay_date);

  const grossBase = Math.max(1, payslip.gross_salary);
  const netPct = Math.min(100, Math.max(0, Math.round((payslip.net_salary / grossBase) * 100)));
  const dedPct = Math.min(100 - netPct, Math.max(0, Math.round((payslip.total_deductions / grossBase) * 100)));
  const employerStatutoryTotal = payslip.epf_employer + payslip.socso_employer + payslip.eis_employer;

  return (
    <div className="space-y-6">
      {/* Visual Pay Composition Strip */}
      <div className="card space-y-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <div>
            <p className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('ux.payslips.netTakeHome')}</p>
            <p className="mt-1 text-2xl font-bold tabular-nums text-emerald-600 dark:text-emerald-400">
              {formatMYR(payslip.net_salary)}
            </p>
            <p className="text-xs text-gray-400 dark:text-slate-500">
              {t('ux.payslips.ofGross', { pct: netPct })}
            </p>
          </div>
          <div>
            <p className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('ux.payslips.deductionsShare')}</p>
            <p className="mt-1 text-xl font-bold tabular-nums text-rose-600 dark:text-rose-400">
              {formatMYR(payslip.total_deductions)}
            </p>
            <p className="text-xs text-gray-400 dark:text-slate-500">
              {t('ux.payslips.ofGross', { pct: dedPct })}
            </p>
          </div>
          <div>
            <p className="text-xs font-medium text-gray-500 dark:text-slate-400">{t('ux.payslips.employerContributions')}</p>
            <p className="mt-1 text-xl font-bold tabular-nums text-teal-700 dark:text-teal-300">
              {formatMYR(employerStatutoryTotal)}
            </p>
            <p className="text-xs text-gray-400 dark:text-slate-500">
              {formatPeriod(payslip.period_year, payslip.period_month)}
            </p>
          </div>
        </div>

        <div className="h-3 w-full overflow-hidden rounded-full bg-gray-100 dark:bg-slate-800 flex no-print">
          <div
            className="h-full bg-gradient-to-r from-emerald-500 to-teal-500 transition-all duration-500"
            style={{ width: `${netPct}%` }}
          />
          <div
            className="h-full bg-rose-400/85 transition-all duration-500"
            style={{ width: `${dedPct}%` }}
          />
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left Column */}
        <div className="space-y-6">
          <div className="card">
            <div className="section-header">
              <span className="section-number">1</span>
              <span className="section-title">{t('portal.payslips.personalDetails')}</span>
            </div>
            <p className="text-sm font-semibold text-gray-900 dark:text-slate-100">{profile?.full_name}</p>
            <p className="text-sm text-gray-400 dark:text-slate-400">{profile?.designation}</p>
          </div>

          <div className="card">
            <div className="section-header">
              <span className="section-number">2</span>
              <span className="section-title">{t('portal.payslips.summary')}</span>
            </div>
            <p className="text-xs text-gray-400 dark:text-slate-500 mb-3">{formatPeriod(payslip.period_year, payslip.period_month)}</p>
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              <SummaryRow label={t('portal.payslips.periodFrom')} value={periodStart} />
              <SummaryRow label={t('portal.payslips.periodTo')} value={periodEnd} />
              <SummaryRow label={t('portal.payslips.payDate')} value={payDate} />
            </div>
            <div className="border-t border-gray-200 dark:border-slate-800 mt-3 pt-3 space-y-1">
              <SummaryRow label={t('portal.payslips.basicSalary')} value={formatMYR(payslip.basic_salary)} />
              <SummaryRow label={t('portal.payslips.grossSalary')} value={formatMYR(payslip.gross_salary)} highlight="font-semibold" />
              <SummaryRow label={t('portal.payslips.totalDeductions')} value={formatMYR(payslip.total_deductions)} />
              <div className="flex justify-between py-2 border-t border-gray-200 dark:border-slate-800 pt-3 mt-2">
                <span className="text-sm font-bold text-gray-900 dark:text-slate-100">{t('portal.payslips.netSalaryShort')}</span>
                <span className="text-sm font-bold tabular-nums text-emerald-600 dark:text-emerald-400">{formatMYR(payslip.net_salary)}</span>
              </div>
            </div>
          </div>
        </div>

        {/* Middle Column */}
        <div className="card">
          <div className="section-header">
            <span className="section-number">3</span>
            <span className="section-title">{t('portal.payslips.details')}</span>
          </div>

          <p className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-2">{t('portal.payslips.earnings')}</p>
          <Row label={t('portal.payslips.basicPayTotal')} amount={payslip.basic_salary} />
          {payslip.total_allowances > 0 && <Row label={t('portal.payslips.allowancesTotal')} amount={payslip.total_allowances} />}
          {payslip.total_overtime > 0 && <Row label={t('portal.payslips.overtimeTotal')} amount={payslip.total_overtime} />}
          {payslip.total_commission > 0 && <Row label={t('portal.payslips.commissionsTotal')} amount={payslip.total_commission} />}
          {payslip.total_bonus > 0 && <Row label={t('portal.payslips.bonusTotal')} amount={payslip.total_bonus} />}
          {otherEarnings !== 0 && <Row label={t('portal.payslips.otherEarningsTotal')} amount={otherEarnings} negative={otherEarnings < 0} />}
          <Row label={t('portal.payslips.totalEarnings')} amount={payslip.gross_salary} bold />

          {payslip.total_claims > 0 && (
            <>
              <p className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mt-5 mb-2">{t('portal.payslips.payments')}</p>
              <Row label={t('portal.payslips.reimbursementsTotal')} amount={payslip.total_claims} />
              <Row label={t('portal.payslips.totalPayments')} amount={payslip.total_claims} bold />
            </>
          )}

          <p className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mt-5 mb-2">{t('portal.payslips.deductions')}</p>
          {payslip.epf_employee > 0 && <Row label={t('payroll.fields.epfEmployee')} amount={payslip.epf_employee} negative />}
          {payslip.eis_employee > 0 && <Row label={t('payroll.fields.eisEmployee')} amount={payslip.eis_employee} negative />}
          {payslip.socso_employee > 0 && <Row label={t('payroll.fields.socsoEmployee')} amount={payslip.socso_employee} negative />}
          {payslip.pcb_amount > 0 && <Row label={t('portal.payslips.pcbMtd')} amount={payslip.pcb_amount} negative />}
          {payslip.zakat_amount > 0 && <Row label={t('payroll.fields.zakat')} amount={payslip.zakat_amount} negative />}
          {payslip.ptptn_amount > 0 && <Row label="PTPTN" amount={payslip.ptptn_amount} negative />}
          {payslip.tabung_haji_amount > 0 && <Row label={t('portal.payslips.tabungHaji')} amount={payslip.tabung_haji_amount} negative />}
          {payslip.total_loan_deductions > 0 && <Row label={t('portal.payslips.loanDeductions')} amount={payslip.total_loan_deductions} negative />}
          {payslip.total_other_deductions > 0 && <Row label={t('portal.payslips.otherDeductions')} amount={payslip.total_other_deductions} negative />}
          {payslip.unpaid_leave_deduction > 0 && <Row label={t('portal.payslips.unpaidLeave')} amount={payslip.unpaid_leave_deduction} negative />}
          <Row label={t('portal.payslips.totalDeductions')} amount={payslip.total_deductions} bold negative />

          <div className="mt-5 pt-4 border-t-2 border-gray-300 dark:border-slate-700">
            <Row label={t('portal.payslips.netSalaryFull')} amount={payslip.net_salary} bold />
          </div>

          {profile?.bank_name && (
            <p className="text-xs text-gray-400 dark:text-slate-500 mt-4 italic">
              {t('portal.payslips.paidByBank', { bank: profile.bank_name, account: profile.bank_account_number })}
            </p>
          )}
        </div>

        {/* Right Column */}
        <div className="space-y-6">
          <div className="card">
            <div className="section-header">
              <span className="section-number">4</span>
              <span className="section-title">{t('portal.payslips.statutoryFunds')}</span>
            </div>
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              <SummaryRow label={t('payroll.fields.epfEmployer')} value={formatMYR(payslip.epf_employer)} />
              <SummaryRow label={t('portal.payslips.totalEpf')} value={formatMYR(payslip.epf_employee + payslip.epf_employer)} />
              <SummaryRow label={t('payroll.fields.eisEmployer')} value={formatMYR(payslip.eis_employer)} />
              <SummaryRow label={t('payroll.fields.socsoEmployer')} value={formatMYR(payslip.socso_employer)} />
            </div>
          </div>

          <div className="card">
            <div className="section-header">
              <span className="section-number">5</span>
              <span className="section-title">{t('portal.payslips.ytd')}</span>
            </div>
            <p className="text-xs text-gray-400 dark:text-slate-500 mb-3">{t('portal.payslips.ytdRange', { from: monthName(1), to: monthName(12), year: payslip.period_year })}</p>
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              <SummaryRow label={t('portal.payslips.ytdGross')} value={formatMYR(payslip.ytd_gross)} />
              <SummaryRow label={t('portal.payslips.ytdEpf')} value={formatMYR(payslip.ytd_epf_employee)} />
              <SummaryRow label={t('portal.payslips.ytdEis')} value={formatMYR(payslip.ytd_eis_employee)} />
              <SummaryRow label={t('portal.payslips.ytdPcb')} value={formatMYR(payslip.ytd_pcb)} />
              <SummaryRow label={t('portal.payslips.ytdSocso')} value={formatMYR(payslip.ytd_socso_employee)} />
              {payslip.ytd_zakat > 0 && (
                <SummaryRow label={t('portal.payslips.ytdZakat')} value={formatMYR(payslip.ytd_zakat)} />
              )}
              <div className="flex justify-between py-2 border-t border-gray-200 dark:border-slate-800 pt-3 mt-2">
                <span className="text-sm font-bold text-gray-900 dark:text-slate-100">{t('portal.payslips.ytdNet')}</span>
                <span className="text-sm font-bold tabular-nums text-gray-900 dark:text-slate-100">{formatMYR(payslip.ytd_net)}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
