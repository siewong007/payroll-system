import { useState, useRef } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Download, Upload, FileJson, CheckCircle2, AlertTriangle, Loader2, X } from 'lucide-react';
import { exportCompanyBackup, importCompanyBackup } from '@/api/backup';
import { listCompanies } from '@/api/admin';
import { PageHeader } from '@/components/ui/PageHeader';
import { StepUpConfirmModal } from '@/components/ui/StepUpConfirmModal';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import { formatDateTime, formatNumber } from '@/lib/format';
import { useAuth } from '@/context/AuthContext';
import { hasAnyRole } from '@/lib/roles';
import type { ImportResult, StepUpCredentials } from '@/types';

const BACKUP_FILE_MAX_BYTES = 100 * 1024 * 1024;

interface BackupPreviewMetadata {
  format_version: string;
  exported_at: string;
  source_company_name: string;
  total_records: number;
}

export function BackupPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  const { user } = useAuth();
  const isSuperAdmin = hasAnyRole(user, ['super_admin']);

  const [exportCompanyId, setExportCompanyId] = useState('');
  const [importCompanyId, setImportCompanyId] = useState('');
  const [confirmTargetName, setConfirmTargetName] = useState('');
  const [exporting, setExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState(false);
  const [exportError, setExportError] = useState('');

  const [importing, setImporting] = useState(false);
  const [importResult, setImportResult] = useState<ImportResult | null>(null);
  const [importError, setImportError] = useState('');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [preview, setPreview] = useState<BackupPreviewMetadata | null>(null);
  const [previewError, setPreviewError] = useState('');
  const [createNewCompany, setCreateNewCompany] = useState(false);
  const [pendingAction, setPendingAction] = useState<'export' | 'import' | null>(null);
  const [stepUpError, setStepUpError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);
  const fileReadSeqRef = useRef(0);

  const {
    data: companies,
    isError: companiesError,
    error: companiesQueryError,
    refetch: refetchCompanies,
  } = useQuery({
    queryKey: ['admin-companies'],
    queryFn: listCompanies,
    enabled: isSuperAdmin,
  });

  const targetCompany = companies?.find((c) => c.id === importCompanyId);

  const requestExport = () => {
    if (isSuperAdmin && !exportCompanyId) return;
    setExportSuccess(false);
    setExportError('');
    setStepUpError('');
    setPendingAction('export');
  };

  const handleExport = async (stepUp: StepUpCredentials) => {
    if (isSuperAdmin && !exportCompanyId) return;
    setExporting(true);
    setExportSuccess(false);
    setExportError('');
    setStepUpError('');
    try {
      await exportCompanyBackup(isSuperAdmin ? exportCompanyId : undefined, stepUp);
      setExportSuccess(true);
      setPendingAction(null);
      toast.success(t('backup.downloaded'));
    } catch (err: unknown) {
      const msg = getErrorMessage(err, t('backup.exportFailed'));
      setStepUpError(msg);
      setExportError(msg);
      toast.error(msg);
    } finally {
      setExporting(false);
    }
  };

  const clearSelectedFile = () => {
    fileReadSeqRef.current += 1;
    setSelectedFile(null);
    setPreview(null);
    setPreviewError('');
    setConfirmTargetName('');
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const seq = ++fileReadSeqRef.current;
    setSelectedFile(file);
    setImportResult(null);
    setImportError('');
    setPreview(null);
    setPreviewError('');
    setConfirmTargetName('');
    if (file.size > BACKUP_FILE_MAX_BYTES) {
      setPreviewError(t('backup.fileTooLarge'));
      return;
    }
    try {
      const text = typeof file.text === 'function' ? await file.text() : await new Response(file).text();
      if (fileReadSeqRef.current !== seq) return;
      const parsed = JSON.parse(text) as {
        metadata?: {
          format_version?: unknown;
          exported_at?: unknown;
          source_company_name?: unknown;
          record_counts?: Record<string, unknown>;
        };
      };
      const meta = parsed?.metadata;
      if (
        !meta ||
        typeof meta.source_company_name !== 'string' ||
        typeof meta.exported_at !== 'string' ||
        typeof meta.format_version !== 'string'
      ) {
        setPreviewError(t('backup.invalidFile'));
        return;
      }
      const counts = meta.record_counts && typeof meta.record_counts === 'object' ? Object.values(meta.record_counts) : [];
      const totalRecordsCount = counts.reduce<number>((sum, n) => sum + (Number.isFinite(Number(n)) ? Number(n) : 0), 0);
      setPreview({
        format_version: meta.format_version,
        exported_at: meta.exported_at,
        source_company_name: meta.source_company_name,
        total_records: totalRecordsCount,
      });
    } catch {
      if (fileReadSeqRef.current !== seq) return;
      setPreviewError(t('backup.invalidFile'));
    }
  };

  const overwriteBlocked =
    isSuperAdmin &&
    !createNewCompany &&
    (!importCompanyId || !targetCompany || confirmTargetName.trim() !== targetCompany.name);

  const requestImport = () => {
    if (!selectedFile || !preview || previewError || overwriteBlocked) return;
    setImportResult(null);
    setImportError('');
    setStepUpError('');
    setPendingAction('import');
  };

  const handleImport = async (stepUp: StepUpCredentials) => {
    if (!selectedFile || !preview || previewError || overwriteBlocked) return;
    setImporting(true);
    setImportResult(null);
    setImportError('');
    setStepUpError('');
    try {
      const result = await importCompanyBackup(
        selectedFile,
        isSuperAdmin && !createNewCompany ? importCompanyId : undefined,
        isSuperAdmin && createNewCompany,
        stepUp,
      );
      setImportResult(result);
      setPendingAction(null);
      clearSelectedFile();
      await queryClient.invalidateQueries();
      toast.success(result.is_overwrite ? t('backup.overwritten') : t('backup.importSuccess'));
    } catch (err: unknown) {
      const msg = getErrorMessage(err, t('backup.importFailed'));
      setStepUpError(msg);
      setImportError(msg);
      toast.error(msg);
    } finally {
      setImporting(false);
    }
  };

  const totalRecords = importResult
    ? Object.values(importResult.records_imported).reduce((a, b) => a + b, 0)
    : 0;

  return (
    <div className="max-w-5xl mx-auto space-y-6">
      <PageHeader title={t('backup.title')} subtitle={t('backup.subtitle')} />

      {isSuperAdmin && companiesError && (
        <div
          role="alert"
          className="p-4 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/25 rounded-xl flex flex-wrap items-center justify-between gap-3 text-sm text-red-700 dark:text-red-300"
        >
          <div className="flex items-start gap-2">
            <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" aria-hidden="true" />
            <span>{getErrorMessage(companiesQueryError, t('companies.loadFailed'))}</span>
          </div>
          <button type="button" onClick={() => void refetchCompanies()} className="btn-secondary min-h-11 sm:min-h-9 text-xs">
            {t('common.retry')}
          </button>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 items-start">
      {/* Export Section */}
      <div className="card p-6">
        <div className="flex flex-col sm:flex-row sm:items-start gap-4">
          <div className="w-10 h-10 bg-blue-50 dark:bg-blue-500/15 rounded-xl flex items-center justify-center shrink-0">
            <Download className="w-5 h-5 text-blue-600 dark:text-blue-400" aria-hidden="true" />
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100">{t('backup.exportTitle')}</h2>
            <p className="text-sm text-gray-600 dark:text-slate-300 mt-1">
              {t('backup.exportDescription')}
            </p>
            <p className="text-xs text-gray-600 dark:text-slate-400 mt-2">
              {t('backup.exportExcludes')}
            </p>

            {isSuperAdmin && (
              <div className="mt-4 max-w-xs">
                <label htmlFor="export-company-select" className="form-label">{t('common.selectCompany')}</label>
                <select
                  id="export-company-select"
                  value={exportCompanyId}
                  onChange={(e) => { setExportCompanyId(e.target.value); setExportSuccess(false); setExportError(''); }}
                  className="form-input min-h-11 sm:min-h-10"
                >
                  <option value="">{t('backup.chooseCompany')}</option>
                  {companies?.map((c) => (
                    <option key={c.id} value={c.id}>{c.name}</option>
                  ))}
                </select>
              </div>
            )}

            <div className="mt-4 flex flex-wrap items-center gap-3">
              <button
                type="button"
                onClick={requestExport}
                disabled={exporting || (isSuperAdmin && !exportCompanyId)}
                className="btn-primary w-full sm:w-auto min-h-11 inline-flex items-center justify-center gap-2"
              >
                {exporting ? (
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Download className="w-4 h-4" aria-hidden="true" />
                )}
                {exporting ? t('common.exporting') : t('backup.exportBackup')}
              </button>
              {exportSuccess && (
                <span role="status" className="inline-flex items-center gap-1.5 text-sm text-green-700 dark:text-emerald-400">
                  <CheckCircle2 className="w-4 h-4" aria-hidden="true" />
                  {t('backup.downloaded')}
                </span>
              )}
            </div>
            {exportError && (
              <div role="alert" className="mt-3 p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/25 flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" aria-hidden="true" />
                {exportError}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Import Section */}
      <div className="card p-6">
        <div className="flex flex-col sm:flex-row sm:items-start gap-4">
          <div className="w-10 h-10 bg-amber-50 dark:bg-amber-500/15 rounded-xl flex items-center justify-center shrink-0">
            <Upload className="w-5 h-5 text-amber-600 dark:text-amber-400" aria-hidden="true" />
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100">{t('backup.importTitle')}</h2>
            <p className="text-sm text-gray-600 dark:text-slate-300 mt-1">
              {t('backup.importDescription')}
            </p>
            <p className="text-xs text-amber-700 dark:text-amber-400 mt-2">
              {t('backup.importNote')}
            </p>

            {isSuperAdmin && (
              <fieldset className="mt-4 max-w-md space-y-2.5">
                <legend className="sr-only">{t('backup.importTitle')}</legend>
                <label className="flex items-center gap-2.5 min-h-11 px-3 py-2 rounded-xl border border-gray-200 dark:border-slate-700 text-sm text-gray-800 dark:text-slate-200 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-[var(--ring)] cursor-pointer hover:bg-gray-50 dark:hover:bg-slate-800/60 transition-colors">
                  <input
                    type="radio"
                    name="import-mode"
                    checked={!createNewCompany}
                    onChange={() => {
                      setCreateNewCompany(false);
                      setConfirmTargetName('');
                      setImportResult(null);
                      setImportError('');
                    }}
                    className="accent-black dark:accent-[var(--accent-1)]"
                  />
                  {t('backup.overwriteExisting')}
                </label>
                {!createNewCompany && (
                  <div className="space-y-2.5 pl-4">
                    <div>
                      <label htmlFor="import-target-select" className="sr-only">{t('backup.chooseTarget')}</label>
                      <select
                        id="import-target-select"
                        value={importCompanyId}
                        onChange={(e) => {
                          setImportCompanyId(e.target.value);
                          setConfirmTargetName('');
                          setImportResult(null);
                          setImportError('');
                        }}
                        className="form-input min-h-11 sm:min-h-10"
                      >
                        <option value="">{t('backup.chooseTarget')}</option>
                        {companies?.map((c) => (
                          <option key={c.id} value={c.id}>{c.name}</option>
                        ))}
                      </select>
                    </div>
                  </div>
                )}
                <label className="flex items-center gap-2.5 min-h-11 px-3 py-2 rounded-xl border border-gray-200 dark:border-slate-700 text-sm text-gray-800 dark:text-slate-200 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-[var(--ring)] cursor-pointer hover:bg-gray-50 dark:hover:bg-slate-800/60 transition-colors">
                  <input
                    type="radio"
                    name="import-mode"
                    checked={createNewCompany}
                    onChange={() => {
                      setCreateNewCompany(true);
                      setConfirmTargetName('');
                      setImportResult(null);
                      setImportError('');
                    }}
                    className="accent-black dark:accent-[var(--accent-1)]"
                  />
                  {t('backup.createNew')}
                </label>
                {createNewCompany && (
                  <p className="text-xs text-gray-600 dark:text-slate-400 pl-4">
                    {t('backup.createNewNote')}
                  </p>
                )}
              </fieldset>
            )}

            <div className="mt-4 space-y-3">
              <div className="flex flex-wrap items-center gap-3">
                <label
                  className="inline-flex w-full sm:w-auto items-center justify-center gap-2 px-4 py-2.5 min-h-[44px] bg-gray-100 dark:bg-slate-800 text-gray-800 dark:text-slate-200 text-sm font-medium rounded-xl hover:bg-gray-200 dark:hover:bg-slate-700 cursor-pointer transition-all focus-within:ring-2 focus-within:ring-[var(--ring)]"
                >
                  <FileJson className="w-4 h-4" aria-hidden="true" />
                  {t('common.chooseFile')}
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept=".json,application/json"
                    onChange={(e) => { void handleFileSelect(e); }}
                    className="sr-only"
                  />
                </label>
                {selectedFile && (
                  <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-slate-300">
                    <span className="break-all">
                      {selectedFile.name} ({(selectedFile.size / 1024 / 1024).toFixed(2)} MB)
                    </span>
                    <button
                      type="button"
                      onClick={clearSelectedFile}
                      aria-label={t('common.removeFile')}
                      className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                    >
                      <X className="w-4 h-4" aria-hidden="true" />
                    </button>
                  </div>
                )}
              </div>

              {previewError && (
                <div role="alert" className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/25 flex items-start gap-2">
                  <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" aria-hidden="true" />
                  {previewError}
                </div>
              )}

              {preview && (
                <div className="p-4 rounded-xl border border-gray-200 dark:border-slate-700 bg-gray-50/70 dark:bg-slate-800/50 space-y-2 text-sm">
                  <p className="text-xs font-semibold uppercase tracking-wider text-gray-600 dark:text-slate-300">
                    {t('backup.previewTitle')}
                  </p>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-xs">
                    <div>
                      <span className="text-gray-600 dark:text-slate-400">{t('backup.sourceCompany')}: </span>
                      <span className="font-medium text-gray-900 dark:text-slate-100">{preview.source_company_name}</span>
                    </div>
                    <div>
                      <span className="text-gray-600 dark:text-slate-400">{t('backup.exportedAt')}: </span>
                      <span className="font-medium text-gray-900 dark:text-slate-100">{formatDateTime(preview.exported_at)}</span>
                    </div>
                    <div>
                      <span className="text-gray-600 dark:text-slate-400">{t('backup.formatVersion')}: </span>
                      <span className="font-mono font-medium text-gray-900 dark:text-slate-100">{preview.format_version}</span>
                    </div>
                    <div>
                      <span className="text-gray-600 dark:text-slate-400">{t('backup.records')}: </span>
                      <span className="font-medium tabular-nums text-gray-900 dark:text-slate-100">{formatNumber(preview.total_records)}</span>
                    </div>
                  </div>
                </div>
              )}

              {isSuperAdmin && !createNewCompany && targetCompany && selectedFile && !previewError && (
                <div className="max-w-md p-3.5 rounded-xl bg-red-50/70 dark:bg-red-500/10 border border-red-200 dark:border-red-500/25 space-y-2">
                  <p className="text-xs text-red-700 dark:text-red-300">
                    <Trans
                      i18nKey="backup.overwriteWarning"
                      values={{ name: targetCompany.name }}
                      components={{ b: <strong /> }}
                    />
                  </p>
                  <label htmlFor="backup-overwrite-confirm" className="block text-xs font-medium text-red-800 dark:text-red-200">
                    <Trans
                      i18nKey="backup.overwriteConfirmLabel"
                      values={{ name: targetCompany.name }}
                      components={{ b: <strong /> }}
                    />
                  </label>
                  <input
                    id="backup-overwrite-confirm"
                    value={confirmTargetName}
                    onChange={(e) => setConfirmTargetName(e.target.value)}
                    placeholder={targetCompany.name}
                    className="form-input"
                  />
                </div>
              )}

              <button
                type="button"
                onClick={requestImport}
                disabled={!selectedFile || !preview || importing || Boolean(previewError) || overwriteBlocked}
                className="w-full sm:w-auto inline-flex items-center justify-center gap-2 px-4 py-2.5 min-h-[44px] bg-amber-600 text-white text-sm font-medium rounded-xl hover:bg-amber-700 disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 transition-all"
              >
                {importing ? (
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Upload className="w-4 h-4" aria-hidden="true" />
                )}
                {importing ? t('common.importing') : t('backup.importBackup')}
              </button>
            </div>

            {importError && (
              <div role="alert" className="mt-4 p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/25 flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" aria-hidden="true" />
                {importError}
              </div>
            )}

            {importResult && (
              <div className="mt-4 space-y-3">
                <div className="p-4 bg-green-50 dark:bg-emerald-500/10 rounded-xl border border-green-100 dark:border-emerald-500/25">
                  <div className="flex items-center gap-2 text-green-700 dark:text-emerald-300 font-medium">
                    <CheckCircle2 className="w-5 h-5" aria-hidden="true" />
                    {importResult.is_overwrite ? t('backup.overwritten') : t('backup.importSuccess')}
                  </div>
                  <p className="text-sm text-green-700 dark:text-emerald-400 mt-1">
                    {importResult.is_overwrite
                      ? <Trans i18nKey="backup.overwrittenDetail" values={{ name: importResult.new_company_name, count: totalRecords }} components={{ b: <strong /> }} />
                      : <Trans i18nKey="backup.createdDetail" values={{ name: importResult.new_company_name, count: totalRecords }} components={{ b: <strong /> }} />
                    }
                  </p>
                </div>

                {importResult.warnings.length > 0 && (
                  <div className="p-3 bg-amber-50 dark:bg-amber-500/10 rounded-xl border border-amber-100 dark:border-amber-500/25">
                    <p className="text-sm font-medium text-amber-800 dark:text-amber-300">{t('backup.warnings')}</p>
                    <ul className="mt-1 text-sm text-amber-700 dark:text-amber-300 list-disc list-inside">
                      {importResult.warnings.map((w, i) => (
                        <li key={i}>{w}</li>
                      ))}
                    </ul>
                  </div>
                )}

                <div className="border border-gray-200 dark:border-slate-800 rounded-xl overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="bg-gray-50 dark:bg-slate-800/60 border-b border-gray-200 dark:border-slate-800">
                        <th className="text-left px-4 py-2.5 font-medium text-gray-700 dark:text-slate-300">{t('backup.table')}</th>
                        <th className="text-right px-4 py-2.5 font-medium text-gray-700 dark:text-slate-300">{t('backup.records')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {Object.entries(importResult.records_imported)
                        .sort(([, a], [, b]) => b - a)
                        .map(([table, count]) => (
                          <tr key={table} className="border-b border-gray-100 dark:border-slate-800 last:border-0">
                            <td className="px-4 py-2 text-gray-700 dark:text-slate-300">{table.replace(/_/g, ' ')}</td>
                            <td className="px-4 py-2 text-right text-gray-900 dark:text-slate-100 font-medium tabular-nums">
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

      <StepUpConfirmModal
        open={pendingAction !== null}
        onClose={() => {
          if (!exporting && !importing) {
            setPendingAction(null);
            setStepUpError('');
          }
        }}
        title={
          pendingAction === 'import'
            ? t('backup.confirmImportTitle')
            : t('backup.confirmExportTitle')
        }
        description={
          pendingAction === 'import'
            ? isSuperAdmin && createNewCompany
              ? t('backup.confirmImportCreateBody', {
                  name: preview?.source_company_name ?? '',
                })
              : targetCompany
                ? (
                    <Trans
                      i18nKey="backup.overwriteWarning"
                      values={{ name: targetCompany.name }}
                      components={{ b: <strong /> }}
                    />
                  )
                : t('backup.importNote')
            : t('backup.confirmExportBody')
        }
        confirmLabel={
          pendingAction === 'import' ? t('backup.importBackup') : t('backup.exportBackup')
        }
        pendingLabel={
          pendingAction === 'import' ? t('common.importing') : t('common.exporting')
        }
        variant={pendingAction === 'import' ? 'danger' : 'primary'}
        pending={exporting || importing}
        error={stepUpError}
        onConfirm={(stepUp) => {
          if (pendingAction === 'export') {
            void handleExport(stepUp);
          } else if (pendingAction === 'import') {
            void handleImport(stepUp);
          }
        }}
      />
    </div>
  );
}
