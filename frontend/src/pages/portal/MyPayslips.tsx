import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, Printer, Download } from 'lucide-react';
import { getMyPayslips, getMyProfile, downloadPayslipPdf } from '@/api/portal';
import { formatMYR } from '@/lib/utils';
import { formatDateLong, formatPeriod, monthName } from '@/lib/format';
import type { MyPayslip, Employee } from '@/types';

const MONTH_NUMBERS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

export function MyPayslips() {
  const { t } = useTranslation();
  const [year, setYear] = useState(() => new Date().getFullYear());
  const [month, setMonth] = useState(new Date().getMonth() + 1);

  const { data: payslips, isLoading } = useQuery({
    queryKey: ['my-payslips'],
    queryFn: getMyPayslips,
  });

  const { data: profile } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  const currentPayslip = payslips?.find(
    (p) => p.period_year === year && p.period_month === month
  );

  const goBack = () => {
    if (month === 1) { setMonth(12); setYear(year - 1); }
    else setMonth(month - 1);
  };

  const goForward = () => {
    if (month === 12) { setMonth(1); setYear(year + 1); }
    else setMonth(month + 1);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="page-header">
          <p className="page-subtitle">{t('portal.payslips.title')}</p>
          <h1 className="page-title">{profile?.full_name || t('common.employee')}</h1>
        </div>
        {currentPayslip && (
          <div className="flex items-center gap-2">
            <button onClick={() => downloadPayslipPdf(currentPayslip.id)} className="btn-secondary">
              <Download className="w-4 h-4" /> {t('portal.payslips.downloadPdf')}
            </button>
            <button onClick={() => window.print()} className="btn-primary">
              <Printer className="w-4 h-4" /> {t('portal.payslips.print')}
            </button>
          </div>
        )}
      </div>

      {/* Month Selector */}
      <div className="flex items-center justify-center gap-3">
        <button onClick={goBack} className="p-2 rounded-lg hover:bg-gray-100 transition-all-fast">
          <ChevronLeft className="w-5 h-5 text-gray-500" />
        </button>
        <select
          value={month}
          onChange={(e) => setMonth(Number(e.target.value))}
          className="form-input !w-auto"
        >
          {MONTH_NUMBERS.map((m) => <option key={m} value={m}>{monthName(m)}</option>)}
        </select>
        <select
          value={year}
          onChange={(e) => setYear(Number(e.target.value))}
          className="form-input !w-auto"
        >
          {Array.from({ length: 6 }, (_, i) => new Date().getFullYear() - 3 + i)
            .map((y) => <option key={y} value={y}>{y}</option>)}
        </select>
        <button onClick={goForward} className="p-2 rounded-lg hover:bg-gray-100 transition-all-fast">
          <ChevronRight className="w-5 h-5 text-gray-500" />
        </button>
      </div>

      {!currentPayslip ? (
        <div className="card text-center py-16">
          <p className="text-gray-400">{t('portal.payslips.empty')}</p>
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
  <div className={`flex justify-between py-2 ${indent ? 'pl-4' : ''} ${bold ? 'font-semibold border-t border-gray-200 pt-3 mt-1' : ''}`}>
    <span className={`text-sm ${indent ? 'text-gray-400 italic' : 'text-gray-600'}`}>{label}</span>
    <span className={`text-sm ${bold ? 'font-bold' : 'font-medium'} ${negative ? 'text-red-600' : ''}`}>
      {negative ? '-' : ''}{formatMYR(Math.abs(amount))}
    </span>
  </div>
);

const SummaryRow = ({ label, value, highlight }: { label: string; value: string; highlight?: string }) => (
  <div className="flex justify-between py-2">
    <span className="text-sm text-gray-400">{label}</span>
    <span className={`text-sm font-medium ${highlight || ''}`}>{value}</span>
  </div>
);

function PayslipDetail({ payslip, profile }: { payslip: MyPayslip; profile: Employee | undefined }) {
  const { t } = useTranslation();
  // The four named earning rows are narrow allow-lists over the entry item_type,
  // but Total Earnings is gross — every earning that reached the payslip. An
  // entry staged under any other item type landed in the total without a row of
  // its own, so the listed rows did not add up to the total printed under them.
  const otherEarnings = payslip.gross_salary - payslip.basic_salary
    - payslip.total_allowances - payslip.total_overtime
    - payslip.total_bonus - payslip.total_commission;

  const periodStart = formatDateLong(payslip.period_start);
  const periodEnd = formatDateLong(payslip.period_end);
  const payDate = formatDateLong(payslip.pay_date);

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Left Column */}
      <div className="space-y-6">
        <div className="card">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">{t('portal.payslips.personalDetails')}</span>
          </div>
          <p className="text-sm font-semibold text-gray-900">{profile?.full_name}</p>
          <p className="text-sm text-gray-400">{profile?.designation}</p>
        </div>

        <div className="card">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">{t('portal.payslips.summary')}</span>
          </div>
          <p className="text-xs text-gray-400 mb-3">{formatPeriod(payslip.period_year, payslip.period_month)}</p>
          <div className="divide-y divide-gray-50">
            <SummaryRow label={t('portal.payslips.periodFrom')} value={periodStart} />
            <SummaryRow label={t('portal.payslips.periodTo')} value={periodEnd} />
            <SummaryRow label={t('portal.payslips.payDate')} value={payDate} />
          </div>
          <div className="border-t border-gray-200 mt-3 pt-3 space-y-1">
            <SummaryRow label={t('portal.payslips.basicSalary')} value={formatMYR(payslip.basic_salary)} />
            <SummaryRow label={t('portal.payslips.grossSalary')} value={formatMYR(payslip.gross_salary)} highlight="text-gray-600 font-semibold" />
            <SummaryRow label={t('portal.payslips.totalDeductions')} value={formatMYR(payslip.total_deductions)} />
            <div className="flex justify-between py-2 border-t border-gray-200 pt-3 mt-2">
              <span className="text-sm font-bold text-gray-900">{t('portal.payslips.netSalaryShort')}</span>
              <span className="text-sm font-bold text-gray-900">{formatMYR(payslip.net_salary)}</span>
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

        <p className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">{t('portal.payslips.earnings')}</p>
        <Row label={t('portal.payslips.basicPayTotal')} amount={payslip.basic_salary} />
        {payslip.total_allowances > 0 && <Row label={t('portal.payslips.allowancesTotal')} amount={payslip.total_allowances} />}
        {payslip.total_overtime > 0 && <Row label={t('portal.payslips.overtimeTotal')} amount={payslip.total_overtime} />}
        {payslip.total_commission > 0 && <Row label={t('portal.payslips.commissionsTotal')} amount={payslip.total_commission} />}
        {payslip.total_bonus > 0 && <Row label={t('portal.payslips.bonusTotal')} amount={payslip.total_bonus} />}
        {otherEarnings !== 0 && <Row label={t('portal.payslips.otherEarningsTotal')} amount={otherEarnings} negative={otherEarnings < 0} />}
        <Row label={t('portal.payslips.totalEarnings')} amount={payslip.gross_salary} bold />

        {payslip.total_claims > 0 && (
          <>
            <p className="text-xs font-bold text-gray-500 uppercase tracking-wider mt-5 mb-2">{t('portal.payslips.payments')}</p>
            <Row label={t('portal.payslips.reimbursementsTotal')} amount={payslip.total_claims} />
            <Row label={t('portal.payslips.totalPayments')} amount={payslip.total_claims} bold />
          </>
        )}

        <p className="text-xs font-bold text-gray-500 uppercase tracking-wider mt-5 mb-2">{t('portal.payslips.deductions')}</p>
        {payslip.epf_employee > 0 && <Row label={t('payroll.fields.epfEmployee')} amount={payslip.epf_employee} negative />}
        {payslip.eis_employee > 0 && <Row label={t('payroll.fields.eisEmployee')} amount={payslip.eis_employee} negative />}
        {payslip.socso_employee > 0 && <Row label={t('payroll.fields.socsoEmployee')} amount={payslip.socso_employee} negative />}
        {payslip.pcb_amount > 0 && <Row label={t('portal.payslips.pcbMtd')} amount={payslip.pcb_amount} negative />}
        {payslip.zakat_amount > 0 && <Row label={t('payroll.fields.zakat')} amount={payslip.zakat_amount} negative />}
        {payslip.ptptn_amount > 0 && <Row label="PTPTN" amount={payslip.ptptn_amount} negative />}
        {/* Already on the wire and already inside Total Deductions; omitting the
            rows just made the deduction side disagree with its own total too. */}
        {payslip.tabung_haji_amount > 0 && <Row label={t('portal.payslips.tabungHaji')} amount={payslip.tabung_haji_amount} negative />}
        {payslip.total_loan_deductions > 0 && <Row label={t('portal.payslips.loanDeductions')} amount={payslip.total_loan_deductions} negative />}
        {payslip.total_other_deductions > 0 && <Row label={t('portal.payslips.otherDeductions')} amount={payslip.total_other_deductions} negative />}
        {payslip.unpaid_leave_deduction > 0 && <Row label={t('portal.payslips.unpaidLeave')} amount={payslip.unpaid_leave_deduction} negative />}
        <Row label={t('portal.payslips.totalDeductions')} amount={payslip.total_deductions} bold negative />

        <div className="mt-5 pt-4 border-t-2 border-gray-300">
          <Row label={t('portal.payslips.netSalaryFull')} amount={payslip.net_salary} bold />
        </div>

        {profile?.bank_name && (
          <p className="text-xs text-gray-400 mt-4 italic">
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
          <div className="divide-y divide-gray-50">
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
          <p className="text-xs text-gray-400 mb-3">{t('portal.payslips.ytdRange', { from: monthName(1), to: monthName(12), year: payslip.period_year })}</p>
          <div className="divide-y divide-gray-50">
            <SummaryRow label={t('portal.payslips.ytdGross')} value={formatMYR(payslip.ytd_gross)} />
            <SummaryRow label={t('portal.payslips.ytdEpf')} value={formatMYR(payslip.ytd_epf_employee)} />
            <SummaryRow label={t('portal.payslips.ytdEis')} value={formatMYR(payslip.ytd_eis_employee)} />
            <SummaryRow label={t('portal.payslips.ytdPcb')} value={formatMYR(payslip.ytd_pcb)} />
            <SummaryRow label={t('portal.payslips.ytdSocso')} value={formatMYR(payslip.ytd_socso_employee)} />
            {payslip.ytd_zakat > 0 && (
              <SummaryRow label={t('portal.payslips.ytdZakat')} value={formatMYR(payslip.ytd_zakat)} />
            )}
            <div className="flex justify-between py-2 border-t border-gray-200 pt-3 mt-2">
              <span className="text-sm font-bold text-gray-900">{t('portal.payslips.ytdNet')}</span>
              <span className="text-sm font-bold">{formatMYR(payslip.ytd_net)}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
