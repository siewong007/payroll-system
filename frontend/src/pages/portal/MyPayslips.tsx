import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, Printer, Download } from 'lucide-react';
import { getMyPayslips, getMyProfile, downloadPayslipPdf } from '@/api/portal';
import { formatMYR } from '@/lib/utils';
import type { MyPayslip } from '@/types';

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December'];

export function MyPayslips() {
  const [year, setYear] = useState(2026);
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
          <p className="page-subtitle">Payslip</p>
          <h1 className="page-title">{profile?.full_name || 'Employee'}</h1>
        </div>
        {currentPayslip && (
          <div className="flex items-center gap-2">
            <button onClick={() => downloadPayslipPdf(currentPayslip.id)} className="btn-secondary">
              <Download className="w-4 h-4" /> Download PDF
            </button>
            <button onClick={() => window.print()} className="btn-primary">
              <Printer className="w-4 h-4" /> Print Payslip
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
          {MONTHS.map((m, i) => <option key={m} value={i + 1}>{m}</option>)}
        </select>
        <select
          value={year}
          onChange={(e) => setYear(Number(e.target.value))}
          className="form-input !w-auto"
        >
          {[2024, 2025, 2026, 2027].map((y) => <option key={y} value={y}>{y}</option>)}
        </select>
        <button onClick={goForward} className="p-2 rounded-lg hover:bg-gray-100 transition-all-fast">
          <ChevronRight className="w-5 h-5 text-gray-500" />
        </button>
      </div>

      {!currentPayslip ? (
        <div className="card text-center py-16">
          <p className="text-gray-400">You have no payment this month</p>
        </div>
      ) : (
        <PayslipDetail payslip={currentPayslip} profile={profile} />
      )}
    </div>
  );
}

function PayslipDetail({ payslip, profile }: { payslip: MyPayslip; profile: any }) {
  const periodStart = new Date(payslip.period_start).toLocaleDateString('en-MY', { day: '2-digit', month: 'long', year: 'numeric' });
  const periodEnd = new Date(payslip.period_end).toLocaleDateString('en-MY', { day: '2-digit', month: 'long', year: 'numeric' });
  const payDate = new Date(payslip.pay_date).toLocaleDateString('en-MY', { day: '2-digit', month: 'long', year: 'numeric' });

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

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Left Column */}
      <div className="space-y-6">
        <div className="card">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">Personal Details</span>
          </div>
          <p className="text-sm font-semibold text-gray-900">{profile?.full_name}</p>
          <p className="text-sm text-gray-400">{profile?.designation}</p>
        </div>

        <div className="card">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">Payslip Summary</span>
          </div>
          <p className="text-xs text-gray-400 mb-3">{MONTHS[payslip.period_month - 1]} {payslip.period_year}</p>
          <div className="divide-y divide-gray-50">
            <SummaryRow label="Pay Period from" value={periodStart} />
            <SummaryRow label="Pay Period to" value={periodEnd} />
            <SummaryRow label="Pay Date" value={payDate} />
          </div>
          <div className="border-t border-gray-200 mt-3 pt-3 space-y-1">
            <SummaryRow label="Basic Salary" value={formatMYR(payslip.basic_salary)} />
            <SummaryRow label="Gross Salary" value={formatMYR(payslip.gross_salary)} highlight="text-gray-600 font-semibold" />
            <SummaryRow label="Total Deductions" value={formatMYR(payslip.total_deductions)} />
            <div className="flex justify-between py-2 border-t border-gray-200 pt-3 mt-2">
              <span className="text-sm font-bold text-gray-900">Net Salary (A - C)</span>
              <span className="text-sm font-bold text-gray-900">{formatMYR(payslip.net_salary)}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Middle Column */}
      <div className="card">
        <div className="section-header">
          <span className="section-number">3</span>
          <span className="section-title">Payslip Details</span>
        </div>

        <p className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">Earnings (A)</p>
        <Row label="Basic Pay (TOTAL)" amount={payslip.basic_salary} />
        {payslip.total_allowances > 0 && <Row label="Allowances (TOTAL)" amount={payslip.total_allowances} />}
        {payslip.total_overtime > 0 && <Row label="Overtime (TOTAL)" amount={payslip.total_overtime} />}
        {payslip.total_commission > 0 && <Row label="Commissions (TOTAL)" amount={payslip.total_commission} />}
        {payslip.total_bonus > 0 && <Row label="Bonus (TOTAL)" amount={payslip.total_bonus} />}
        <Row label="Total Earnings" amount={payslip.gross_salary} bold />

        {payslip.total_claims > 0 && (
          <>
            <p className="text-xs font-bold text-gray-500 uppercase tracking-wider mt-5 mb-2">Payments (B)</p>
            <Row label="Reimbursements (TOTAL)" amount={payslip.total_claims} />
            <Row label="Total Payments" amount={payslip.total_claims} bold />
          </>
        )}

        <p className="text-xs font-bold text-gray-500 uppercase tracking-wider mt-5 mb-2">Deductions (C)</p>
        {payslip.epf_employee > 0 && <Row label="EPF (Employee)" amount={payslip.epf_employee} negative />}
        {payslip.eis_employee > 0 && <Row label="EIS (Employee)" amount={payslip.eis_employee} negative />}
        {payslip.socso_employee > 0 && <Row label="SOCSO (Employee)" amount={payslip.socso_employee} negative />}
        {payslip.pcb_amount > 0 && <Row label="PCB MTD" amount={payslip.pcb_amount} negative />}
        {payslip.zakat_amount > 0 && <Row label="Zakat" amount={payslip.zakat_amount} negative />}
        {payslip.ptptn_amount > 0 && <Row label="PTPTN" amount={payslip.ptptn_amount} negative />}
        {payslip.unpaid_leave_deduction > 0 && <Row label="Unpaid Leave" amount={payslip.unpaid_leave_deduction} negative />}
        <Row label="Total Deductions" amount={payslip.total_deductions} bold negative />

        <div className="mt-5 pt-4 border-t-2 border-gray-300">
          <Row label="Net Salary (A + B - C)" amount={payslip.net_salary} bold />
        </div>

        {profile?.bank_name && (
          <p className="text-xs text-gray-400 mt-4 italic">
            *Paid by Bank Transfer – {profile.bank_name}, account no. {profile.bank_account_number}
          </p>
        )}
      </div>

      {/* Right Column */}
      <div className="space-y-6">
        <div className="card">
          <div className="section-header">
            <span className="section-number">4</span>
            <span className="section-title">Statutory Funds</span>
          </div>
          <div className="divide-y divide-gray-50">
            <SummaryRow label="EPF (Employer)" value={formatMYR(payslip.epf_employer)} />
            <SummaryRow label="Total EPF" value={formatMYR(payslip.epf_employee + payslip.epf_employer)} />
            <SummaryRow label="EIS (Employer)" value={formatMYR(payslip.eis_employer)} />
            <SummaryRow label="SOCSO (Employer)" value={formatMYR(payslip.socso_employer)} />
          </div>
        </div>

        <div className="card">
          <div className="section-header">
            <span className="section-number">5</span>
            <span className="section-title">Year-To-Date</span>
          </div>
          <p className="text-xs text-gray-400 mb-3">Jan-{payslip.period_year} to Dec-{payslip.period_year}</p>
          <div className="divide-y divide-gray-50">
            <SummaryRow label="YTD Gross Salary" value={formatMYR(payslip.ytd_gross)} />
            <SummaryRow label="YTD EPF (Employee)" value={formatMYR(payslip.ytd_epf_employee)} />
            <SummaryRow label="YTD EIS (Employee)" value={formatMYR(payslip.ytd_eis_employee)} />
            <SummaryRow label="YTD PCB MTD" value={formatMYR(payslip.ytd_pcb)} />
            <SummaryRow label="YTD SOCSO (Employee)" value={formatMYR(payslip.ytd_socso_employee)} />
            {payslip.ytd_zakat > 0 && (
              <SummaryRow label="YTD Zakat" value={formatMYR(payslip.ytd_zakat)} />
            )}
            <div className="flex justify-between py-2 border-t border-gray-200 pt-3 mt-2">
              <span className="text-sm font-bold text-gray-900">YTD Net Salary</span>
              <span className="text-sm font-bold">{formatMYR(payslip.ytd_net)}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
