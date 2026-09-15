import { useState, useRef } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Download, Upload, FileJson, CheckCircle2, AlertTriangle, Loader2 } from 'lucide-react';
import { exportCompanyBackup, importCompanyBackup } from '@/api/backup';
import { listCompanies } from '@/api/admin';
import { getErrorMessage } from '@/lib/utils';
import { formatNumber } from '@/lib/format';
import { useAuth } from '@/context/AuthContext';
import { hasAnyRole } from '@/lib/roles';
import type { ImportResult } from '@/types';

export function BackupPage() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const isSuperAdmin = hasAnyRole(user, ['super_admin']);

  const [selectedCompanyId, setSelectedCompanyId] = useState('');
  const [exporting, setExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState(false);
  const [exportError, setExportError] = useState('');

  const [importing, setImporting] = useState(false);
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [importError, setImportError] = useState('');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [createNewCompany, setCreateNewCompany] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data: companies } = useQuery({
    queryKey: ['admin-companies'],
    queryFn: listCompanies,
    enabled: isSuperAdmin,
  });

  const handleExport = async () => {
    if (isSuperAdmin && !selectedCompanyId) return;
    setExporting(true);
    setExportSuccess(false);
    setExportError('');
    try {
      await exportCompanyBackup(isSuperAdmin ? selectedCompanyId : undefined);
      setExportSuccess(true);
    } catch (err: unknown) {
      const msg = err instanceof Error ? (err as { response?: { data?: { error?: string } } }).response?.data?.error || err.message : t('backup.exportFailed');
      setExportError(msg);
    } finally {
      setExporting(false);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setSelectedFile(file);
      setImportResult(null);
      setImportError('');
    }
  };

  const handleImport = async () => {
    if (!selectedFile) return;
    setImporting(true);
    setImportResult(null);
    setImportError('');
    try {
      const result = await importCompanyBackup(
        selectedFile,
        isSuperAdmin && !createNewCompany ? selectedCompanyId : undefined,
        isSuperAdmin && createNewCompany,
      );
      setImportResult(result);
      setSelectedFile(null);
      if (fileInputRef.current) fileInputRef.current.value = '';
    } catch (err: unknown) {
      setImportError(getErrorMessage(err, t('backup.importFailed')));
    } finally {
      setImporting(false);
    }
  };

  const totalRecords = importResult
    ? Object.values(importResult.records_imported).reduce((a, b) => a + b, 0)
    : 0;

  return (
    <div className="max-w-4xl mx-auto space-y-8">
      <div>
        <h1 className="text-2xl font-bold text-gray-900">{t('backup.title')}</h1>
        <p className="text-sm text-gray-500 mt-1">
          {t('backup.subtitle')}
        </p>
      </div>

      {/* Export Section */}
      <div className="bg-white rounded-2xl border border-gray-200 p-6">
        <div className="flex items-start gap-4">
          <div className="w-10 h-10 bg-blue-50 rounded-xl flex items-center justify-center shrink-0">
            <Download className="w-5 h-5 text-blue-600" />
          </div>
          <div className="flex-1">
            <h2 className="text-lg font-semibold text-gray-900">{t('backup.exportTitle')}</h2>
            <p className="text-sm text-gray-500 mt-1">
              {t('backup.exportDescription')}
            </p>
            <p className="text-xs text-gray-400 mt-2">
              {t('backup.exportExcludes')}
            </p>

            {isSuperAdmin && (
              <div className="mt-4 max-w-xs">
                <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.selectCompany')}</label>
                <select
                  value={selectedCompanyId}
                  onChange={(e) => { setSelectedCompanyId(e.target.value); setExportSuccess(false); setExportError(''); }}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
                >
                  <option value="">{t('backup.chooseCompany')}</option>
                  {companies?.map((c) => (
                    <option key={c.id} value={c.id}>{c.name}</option>
                  ))}
                </select>
              </div>
            )}

            <div className="mt-4 flex items-center gap-3">
              <button
                onClick={handleExport}
                disabled={exporting || (isSuperAdmin && !selectedCompanyId)}
                className="inline-flex items-center gap-2 px-4 py-2.5 bg-gray-900 text-white text-sm font-medium rounded-xl hover:bg-gray-800 disabled:opacity-50 transition-all"
              >
                {exporting ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Download className="w-4 h-4" />
                )}
                {exporting ? t('common.exporting') : t('backup.exportBackup')}
              </button>
              {exportSuccess && (
                <span className="inline-flex items-center gap-1.5 text-sm text-green-600">
                  <CheckCircle2 className="w-4 h-4" />
                  {t('backup.downloaded')}
                </span>
              )}
            </div>
            {exportError && (
              <div className="mt-3 p-3 bg-red-50 text-red-700 text-sm rounded-xl flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
                {exportError}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Import Section */}
      <div className="bg-white rounded-2xl border border-gray-200 p-6">
        <div className="flex items-start gap-4">
          <div className="w-10 h-10 bg-amber-50 rounded-xl flex items-center justify-center shrink-0">
            <Upload className="w-5 h-5 text-amber-600" />
          </div>
          <div className="flex-1">
            <h2 className="text-lg font-semibold text-gray-900">{t('backup.importTitle')}</h2>
            <p className="text-sm text-gray-500 mt-1">
              {t('backup.importDescription')}
            </p>
            <p className="text-xs text-amber-600 mt-2">
              {t('backup.importNote')}
            </p>

            {isSuperAdmin && (
              <div className="mt-4 max-w-md space-y-3">
                <label className="flex items-center gap-2 text-sm text-gray-700 cursor-pointer">
                  <input
                    type="radio"
                    checked={!createNewCompany}
                    onChange={() => { setCreateNewCompany(false); setImportResult(null); setImportError(''); }}
                  />
                  {t('backup.overwriteExisting')}
                </label>
                {!createNewCompany && (
                  <select
                    value={selectedCompanyId}
                    onChange={(e) => { setSelectedCompanyId(e.target.value); setImportResult(null); setImportError(''); }}
                    className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm focus:ring-1 focus:ring-black outline-none"
                  >
                    <option value="">{t('backup.chooseTarget')}</option>
                    {companies?.map((c) => (
                      <option key={c.id} value={c.id}>{c.name}</option>
                    ))}
                  </select>
                )}
                <label className="flex items-center gap-2 text-sm text-gray-700 cursor-pointer">
                  <input
                    type="radio"
                    checked={createNewCompany}
                    onChange={() => { setCreateNewCompany(true); setImportResult(null); setImportError(''); }}
                  />
                  {t('backup.createNew')}
                </label>
                {createNewCompany && (
                  <p className="text-xs text-gray-500">
                    {t('backup.createNewNote')}
                  </p>
                )}
              </div>
            )}

            <div className="mt-4 space-y-3">
              <div className="flex items-center gap-3">
                <label
                  className="inline-flex items-center gap-2 px-4 py-2.5 bg-gray-100 text-gray-700 text-sm font-medium rounded-xl hover:bg-gray-200 cursor-pointer transition-all"
                >
                  <FileJson className="w-4 h-4" />
                  {t('common.chooseFile')}
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept=".json"
                    onChange={handleFileSelect}
                    className="hidden"
                  />
                </label>
                {selectedFile && (
                  <span className="text-sm text-gray-600">
                    {selectedFile.name} ({(selectedFile.size / 1024 / 1024).toFixed(2)} MB)
                  </span>
                )}
              </div>

              <button
                onClick={handleImport}
                disabled={!selectedFile || importing || (isSuperAdmin && !createNewCompany && !selectedCompanyId)}
                className="inline-flex items-center gap-2 px-4 py-2.5 bg-amber-600 text-white text-sm font-medium rounded-xl hover:bg-amber-700 disabled:opacity-50 transition-all"
              >
                {importing ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Upload className="w-4 h-4" />
                )}
                {importing ? t('common.importing') : t('backup.importBackup')}
              </button>
            </div>

            {importError && (
              <div className="mt-4 p-3 bg-red-50 text-red-700 text-sm rounded-xl flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
                {importError}
              </div>
            )}

            {importResult && (
              <div className="mt-4 space-y-3">
                <div className="p-4 bg-green-50 rounded-xl">
                  <div className="flex items-center gap-2 text-green-700 font-medium">
                    <CheckCircle2 className="w-5 h-5" />
                    {importResult.is_overwrite ? t('backup.overwritten') : t('backup.importSuccess')}
                  </div>
                  <p className="text-sm text-green-600 mt-1">
                    {importResult.is_overwrite
                      ? <Trans i18nKey="backup.overwrittenDetail" values={{ name: importResult.new_company_name, count: totalRecords }} components={{ b: <strong /> }} />
                      : <Trans i18nKey="backup.createdDetail" values={{ name: importResult.new_company_name, count: totalRecords }} components={{ b: <strong /> }} />
                    }
                  </p>
                </div>

                {importResult.warnings.length > 0 && (
                  <div className="p-3 bg-amber-50 rounded-xl">
                    <p className="text-sm font-medium text-amber-700">{t('backup.warnings')}</p>
                    <ul className="mt-1 text-sm text-amber-600 list-disc list-inside">
                      {importResult.warnings.map((w, i) => (
                        <li key={i}>{w}</li>
                      ))}
                    </ul>
                  </div>
                )}

                <div className="border border-gray-200 rounded-xl overflow-hidden">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="bg-gray-50 border-b border-gray-200">
                        <th className="text-left px-4 py-2.5 font-medium text-gray-600">{t('backup.table')}</th>
                        <th className="text-right px-4 py-2.5 font-medium text-gray-600">{t('backup.records')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {Object.entries(importResult.records_imported)
                        .sort(([, a], [, b]) => b - a)
                        .map(([table, count]) => (
                          <tr key={table} className="border-b border-gray-100 last:border-0">
                            <td className="px-4 py-2 text-gray-700">{table.replace(/_/g, ' ')}</td>
                            <td className="px-4 py-2 text-right text-gray-900 font-medium tabular-nums">
                              {formatNumber(count)}
                            </td>
                          </tr>
                        ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
