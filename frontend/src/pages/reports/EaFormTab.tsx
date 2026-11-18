import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, FileText } from 'lucide-react';
import { downloadEaForm } from '@/api/reports';
import { formatMYR } from '@/lib/format';
import type { EaEmployeeSummary } from '@/types';

const fmt = formatMYR;

export function EaFormTab({
  data,
  isLoading,
  year,
}: {
  data: EaEmployeeSummary[] | undefined;
  isLoading: boolean;
  year: number;
}) {
  const { t } = useTranslation();
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
      <h2 className="font-semibold text-gray-900">{t('reports.eaTitle', { year })}</h2>
      {!data || data.length === 0 ? (
        <div className="text-center py-16 text-gray-400">
          <FileText className="w-8 h-8 mx-auto mb-3 opacity-40" />
          <p className="text-sm">{t('reports.noPayrollData', { year })}</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 text-gray-500 text-xs uppercase tracking-wide">
                <th className="text-left px-5 py-3 font-medium">{t('common.employee')}</th>
                <th className="text-left px-5 py-3 font-medium">{t('reports.cols.icNumber')}</th>
                <th className="text-center px-5 py-3 font-medium">{t('reports.cols.months')}</th>
                <th className="text-right px-5 py-3 font-medium">{t('reports.cols.ytdGross')}</th>
                <th className="text-right px-5 py-3 font-medium">{t('common.action')}</th>
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
                  <td className="px-5 py-3 text-right font-medium text-gray-900">
                    {fmt(emp.ytd_gross)}
                  </td>
                  <td className="px-5 py-3 text-right">
                    <button
                      onClick={() => handleDownload(emp.employee_id)}
                      disabled={downloading === emp.employee_id}
                      className="btn-secondary !py-1 !px-2.5 !text-xs disabled:opacity-50"
                    >
                      <Download className="w-3 h-3" />
                      {downloading === emp.employee_id
                        ? t('common.generating')
                        : t('reports.downloadPdf')}
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
