import { useState, useCallback, useEffect } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import { Upload, Download, CheckCircle, XCircle, AlertTriangle, FileSpreadsheet, Loader2 } from 'lucide-react';
import { downloadImportTemplate, validateImport, confirmImport } from '@/api/employees';
import { getJob } from '@/api/jobs';
import { triggerBlobDownload } from '@/lib/download';
import { getErrorMessage, translateServerMessage } from '@/lib/utils';
import { PageHeader } from '@/components/ui/PageHeader';
import { useToast } from '@/lib/toast';
import type { ImportValidationResponse, ImportConfirmResponse } from '@/types';

type Step = 'upload' | 'preview' | 'confirm';
const MAX_IMPORT_BYTES = 20 * 1024 * 1024;

export function EmployeeImport() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [step, setStep] = useState<Step>('upload');
  const [file, setFile] = useState<File | null>(null);
  const [validation, setValidation] = useState<ImportValidationResponse | null>(null);
  const [skipInvalid, setSkipInvalid] = useState(true);
  const [result, setResult] = useState<ImportConfirmResponse | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [downloadingFormat, setDownloadingFormat] = useState<'csv' | 'xlsx' | null>(null);

  const validateMutation = useMutation({
    mutationFn: validateImport,
    onSuccess: (data) => {
      setValidation(data);
      setStep('preview');
    },
  });

  // The confirm runs detached — whole-headcount imports cannot fit in a
  // request. Submit returns the job; this poll follows it to a terminal state.
  const [jobId, setJobId] = useState<string | null>(null);
  const [jobError, setJobError] = useState('');
  const confirmMutation = useMutation({
    mutationFn: confirmImport,
    onSuccess: (job) => {
      setJobError('');
      setJobId(job.id);
    },
  });

  const {
    data: importJob,
    isError: isJobPollError,
    error: jobPollError,
    refetch: refetchJob,
  } = useQuery({
    queryKey: ['importJob', jobId],
    queryFn: () => getJob(jobId as string),
    enabled: jobId !== null,
    refetchInterval: (query) =>
      query.state.status !== 'error' &&
      (query.state.data?.status === 'pending' || query.state.data?.status === 'running')
        ? 1_000
        : false,
  });

  const importing =
    confirmMutation.isPending ||
    (jobId !== null &&
      importJob?.status !== 'succeeded' &&
      importJob?.status !== 'failed');

  useEffect(() => {
    if (!importJob) return;
    if (importJob.status === 'succeeded') {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employees-picker'] });
      setResult(importJob.result as ImportConfirmResponse);
      setStep('confirm');
      setJobId(null);
    } else if (importJob.status === 'failed') {
      setJobError(
        importJob.error
          ? translateServerMessage(importJob.error)
          : t('employees.import.importFailed'),
      );
      setJobId(null);
    }
  }, [importJob, queryClient, t]);

  const handleDownloadTemplate = async (format: 'csv' | 'xlsx') => {
    setDownloadingFormat(format);
    try {
      const blob = await downloadImportTemplate(format);
      triggerBlobDownload(blob, `employee_import_template.${format}`);
    } catch (err) {
      toast.error(getErrorMessage(err, t('employees.import.templateDownloadFailed')));
    } finally {
      setDownloadingFormat(null);
    }
  };

  const handleFileSelect = useCallback((selectedFile: File) => {
    const validExts = ['.csv', '.xlsx', '.xls'];
    const dotIndex = selectedFile.name.lastIndexOf('.');
    const ext = dotIndex >= 0 ? selectedFile.name.substring(dotIndex).toLowerCase() : '';
    if (!validExts.includes(ext)) {
      toast.error(t('employees.import.invalidFile'));
      return;
    }
    if (selectedFile.size > MAX_IMPORT_BYTES) {
      toast.error(t('employees.import.fileTooLarge'));
      return;
    }
    setFile(selectedFile);
  }, [t, toast]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleFileSelect(e.dataTransfer.files[0]);
    }
  }, [handleFileSelect]);

  const handleUpload = () => {
    if (file) {
      validateMutation.mutate(file);
    }
  };

  const handleConfirm = () => {
    if (validation) {
      confirmMutation.mutate({
        session_id: validation.session_id,
        skip_invalid: skipInvalid,
      });
    }
  };

  return (
    <div>
      <PageHeader
        title={t('employees.import.title')}
        breadcrumbs={[
          { label: t('nav.employees'), href: '/employees' },
          { label: t('employees.import.title') },
        ]}
      />

      {/* Steps indicator */}
      <ol className="flex flex-wrap items-center gap-2 mb-8">
        {(['upload', 'preview', 'confirm'] as Step[]).map((s, i) => {
          const currentIndex = ['upload', 'preview', 'confirm'].indexOf(step);
          const isCurrent = step === s;
          const isDone = i < currentIndex;
          return (
            <li key={s} className="flex items-center gap-2">
              <div
                aria-current={isCurrent ? 'step' : undefined}
                className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold tabular-nums shrink-0 ${
                  isCurrent
                    ? 'bg-gradient-to-br from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-sm'
                    : isDone
                    ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300'
                    : 'bg-gray-100 text-gray-500 dark:bg-slate-800 dark:text-slate-400'
                }`}
              >
                {i + 1}
              </div>
              <span
                className={`text-xs sm:text-sm ${
                  isCurrent
                    ? 'font-medium text-gray-900 dark:text-slate-100'
                    : 'hidden sm:inline text-gray-500 dark:text-slate-400'
                }`}
              >
                {t(`employees.import.steps.${s}`)}
              </span>
              {i < 2 && <div className="w-5 sm:w-8 h-px bg-gray-200 dark:bg-slate-700" aria-hidden="true" />}
            </li>
          );
        })}
      </ol>

      {/* Step 1: Upload */}
      {step === 'upload' && (
        <div className="card p-5 sm:p-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.import.uploadTitle')}</h2>

          {/* Template download */}
          <div className="bg-blue-50 dark:bg-blue-500/10 border border-blue-200 dark:border-blue-500/30 rounded-lg p-4 mb-6">
            <p className="text-sm text-blue-800 dark:text-blue-200 mb-3">
              {t('employees.import.templateHint')}
            </p>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                disabled={downloadingFormat !== null}
                onClick={() => handleDownloadTemplate('xlsx')}
                className="min-h-11 sm:min-h-9 flex items-center justify-center gap-2 bg-blue-600 text-white px-3.5 py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] w-full sm:w-auto"
              >
                {downloadingFormat === 'xlsx' ? (
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Download className="w-4 h-4" aria-hidden="true" />
                )}
                {t('employees.import.excelTemplate')}
              </button>
              <button
                type="button"
                disabled={downloadingFormat !== null}
                onClick={() => handleDownloadTemplate('csv')}
                className="min-h-11 sm:min-h-9 flex items-center justify-center gap-2 border border-blue-300 dark:border-blue-500/40 text-blue-700 dark:text-blue-300 px-3.5 py-2 rounded-lg hover:bg-blue-100 dark:hover:bg-blue-500/20 disabled:opacity-50 transition-colors text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] w-full sm:w-auto"
              >
                {downloadingFormat === 'csv' ? (
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Download className="w-4 h-4" aria-hidden="true" />
                )}
                {t('employees.import.csvTemplate')}
              </button>
            </div>
          </div>

          {/* Drop zone */}
          <div
            onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
            onDragLeave={() => setDragOver(false)}
            onDrop={handleDrop}
            className={`border-2 border-dashed rounded-xl p-8 text-center transition-colors ${
              dragOver
                ? 'border-[var(--accent-1)] bg-[var(--accent-soft)]'
                : 'border-gray-300 dark:border-slate-700'
            }`}
          >
            {file ? (
              <div className="flex flex-col items-center gap-3">
                <FileSpreadsheet className="w-12 h-12 text-green-600 dark:text-emerald-400" aria-hidden="true" />
                <div>
                  <p className="font-medium text-gray-900 dark:text-slate-100 break-all">{file.name}</p>
                  <p className="text-sm text-gray-500 dark:text-slate-400 tabular-nums">{(file.size / 1024).toFixed(1)} KB</p>
                </div>
                <button
                  type="button"
                  onClick={() => setFile(null)}
                  className="min-h-11 sm:min-h-9 text-sm font-medium text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 py-1.5 px-3 rounded-lg hover:bg-red-50 dark:hover:bg-red-500/10"
                >
                  {t('common.remove')}
                </button>
              </div>
            ) : (
              <div className="flex flex-col items-center gap-3">
                <Upload className="w-12 h-12 text-gray-400 dark:text-slate-500" aria-hidden="true" />
                <div>
                  <p className="text-gray-600 dark:text-slate-300">{t('employees.import.dropPrompt')}</p>
                  <label className="min-h-11 sm:min-h-0 inline-flex items-center mt-1 text-[var(--accent-1)] font-medium cursor-pointer hover:underline focus-within:outline-none focus-within:ring-2 focus-within:ring-[var(--ring)] rounded px-2 py-0.5">
                    {t('employees.import.browse')}
                    <input
                      type="file"
                      accept=".csv,.xlsx,.xls"
                      className="sr-only"
                      onChange={(e) => {
                        const picked = e.target.files?.[0];
                        e.currentTarget.value = '';
                        if (picked) handleFileSelect(picked);
                      }}
                    />
                  </label>
                </div>
                <p className="text-xs text-gray-500 dark:text-slate-400">{t('employees.import.fileTypes')}</p>
              </div>
            )}
          </div>

          {validateMutation.isError && (
            <div role="alert" className="mt-4 p-3 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 rounded-lg text-sm text-red-700 dark:text-red-300">
              {getErrorMessage(validateMutation.error, t('employees.import.validateFailed'))}
            </div>
          )}

          <div className="mt-6 flex justify-end">
            <button
              type="button"
              onClick={handleUpload}
              disabled={!file || validateMutation.isPending}
              className="btn-primary w-full sm:w-auto"
            >
              {validateMutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {t('employees.import.validating')}
                </>
              ) : (
                t('employees.import.validateButton')
              )}
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Preview */}
      {step === 'preview' && validation && (
        <div className="card p-5 sm:p-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.import.resultsTitle')}</h2>

          {/* Summary */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-6">
            <div className="bg-gray-50 dark:bg-slate-800/70 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-gray-900 dark:text-slate-100 tabular-nums">{validation.total_rows}</p>
              <p className="text-xs text-gray-500 dark:text-slate-400">{t('employees.import.totalRows')}</p>
            </div>
            <div className="bg-green-50 dark:bg-emerald-500/15 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-green-700 dark:text-emerald-300 tabular-nums">{validation.valid_rows}</p>
              <p className="text-xs text-green-600 dark:text-emerald-400">{t('employees.import.valid')}</p>
            </div>
            <div className="bg-red-50 dark:bg-red-500/15 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-red-700 dark:text-red-300 tabular-nums">{validation.error_rows}</p>
              <p className="text-xs text-red-600 dark:text-red-400">{t('employees.import.errorsLabel')}</p>
            </div>
            <div className="bg-yellow-50 dark:bg-amber-500/15 rounded-lg p-3 text-center">
              <p className="text-2xl font-bold text-yellow-700 dark:text-amber-300 tabular-nums">{validation.duplicate_rows}</p>
              <p className="text-xs text-yellow-700 dark:text-amber-400">{t('employees.import.duplicates')}</p>
            </div>
          </div>

          {/* Row details */}
          <div className="border border-gray-200 dark:border-slate-800 rounded-lg overflow-hidden mb-6">
            <div className="max-h-96 overflow-auto">
              <table className="w-full text-sm">
                <thead className="bg-gray-50 dark:bg-slate-800 sticky top-0 z-10 border-b border-gray-200 dark:border-slate-700">
                  <tr>
                    <th className="px-3 py-2 text-left font-medium text-gray-600 dark:text-slate-300">{t('employees.import.colRow')}</th>
                    <th className="px-3 py-2 text-left font-medium text-gray-600 dark:text-slate-300">{t('common.status')}</th>
                    <th className="px-3 py-2 text-left font-medium text-gray-600 dark:text-slate-300">{t('employees.columns.employee')}</th>
                    <th className="px-3 py-2 text-left font-medium text-gray-600 dark:text-slate-300">{t('common.details')}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                  {validation.rows.map((row) => (
                    <tr key={row.row_number} className={
                      row.status === 'valid'
                        ? 'bg-green-50/50 dark:bg-emerald-500/5'
                        : row.status === 'duplicate'
                        ? 'bg-yellow-50/50 dark:bg-amber-500/5'
                        : 'bg-red-50/50 dark:bg-red-500/5'
                    }>
                      <td className="px-3 py-2 text-gray-600 dark:text-slate-400 tabular-nums">{row.row_number}</td>
                      <td className="px-3 py-2">
                        {row.status === 'valid' && (
                          <>
                            <CheckCircle className="w-4 h-4 text-green-600 dark:text-emerald-400" aria-hidden="true" />
                            <span className="sr-only">{t('employees.import.valid')}</span>
                          </>
                        )}
                        {row.status === 'error' && (
                          <>
                            <XCircle className="w-4 h-4 text-red-600 dark:text-red-400" aria-hidden="true" />
                            <span className="sr-only">{t('employees.import.errorsLabel')}</span>
                          </>
                        )}
                        {row.status === 'duplicate' && (
                          <>
                            <AlertTriangle className="w-4 h-4 text-yellow-600 dark:text-amber-400" aria-hidden="true" />
                            <span className="sr-only">{t('employees.import.duplicates')}</span>
                          </>
                        )}
                      </td>
                      <td className="px-3 py-2">
                        <div className="font-medium text-gray-900 dark:text-slate-100">{row.data?.full_name || '\u2014'}</div>
                        <div className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">{row.data?.employee_number || '\u2014'}</div>
                      </td>
                      <td className="px-3 py-2">
                        {row.errors.length > 0 ? (
                          <ul className="text-xs space-y-0.5">
                            {row.errors.map((err, i) => (
                              <li key={i} className="text-red-600 dark:text-red-400">
                                <span className="font-medium">{err.field}</span>: {err.message}
                              </li>
                            ))}
                          </ul>
                        ) : (
                          <span className="text-xs text-green-600 dark:text-emerald-400">{t('employees.import.readyToImport')}</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Options */}
          {(validation.error_rows > 0 || validation.duplicate_rows > 0) && (
            <label className="min-h-11 sm:min-h-9 flex items-center gap-2 mb-4 text-sm text-gray-700 dark:text-slate-200 cursor-pointer">
              <input
                type="checkbox"
                checked={skipInvalid}
                onChange={(e) => setSkipInvalid(e.target.checked)}
                className="h-4 w-4 rounded border-gray-300 dark:border-slate-700 accent-[var(--accent-1)]"
              />
              <span>{t('employees.import.skipInvalid', { count: validation.valid_rows })}</span>
            </label>
          )}

          {isJobPollError && (
            <div role="alert" className="mb-4 p-3 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 rounded-lg text-sm text-red-700 dark:text-red-300 flex flex-wrap items-center justify-between gap-2">
              <span>{getErrorMessage(jobPollError, t('employees.import.importFailed'))}</span>
              <div className="flex items-center gap-2 shrink-0">
                <button
                  type="button"
                  onClick={() => setJobId(null)}
                  className="min-h-11 sm:min-h-9 px-2 text-xs font-medium underline hover:text-red-900 dark:hover:text-red-200"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="button"
                  onClick={() => refetchJob()}
                  className="btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 sm:!text-xs"
                >
                  {t('common.retry')}
                </button>
              </div>
            </div>
          )}

          {(confirmMutation.isError || jobError) && (
            <div role="alert" className="mb-4 p-3 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 rounded-lg text-sm text-red-700 dark:text-red-300">
              {jobError || getErrorMessage(confirmMutation.error, t('employees.import.importFailed'))}
            </div>
          )}

          <div className="flex flex-col-reverse sm:flex-row justify-between gap-3">
            <button
              type="button"
              disabled={importing || jobId !== null}
              onClick={() => {
                setStep('upload');
                setValidation(null);
                setFile(null);
                setJobId(null);
                setJobError('');
                confirmMutation.reset();
                validateMutation.reset();
              }}
              className="btn-secondary w-full sm:w-auto disabled:opacity-50"
            >
              {t('employees.import.backToUpload')}
            </button>
            <button
              type="button"
              onClick={handleConfirm}
              disabled={validation.valid_rows === 0 || importing}
              className="btn-primary w-full sm:w-auto"
            >
              {importing ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {t('employees.import.importing')}
                </>
              ) : (
                t('employees.import.importButton', { count: skipInvalid ? validation.valid_rows : validation.total_rows })
              )}
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Results */}
      {step === 'confirm' && result && (
        <div className="card p-6 text-center">
          <CheckCircle className="w-16 h-16 text-green-600 dark:text-emerald-400 mx-auto mb-4" aria-hidden="true" />
          <h2 className="text-xl font-semibold text-gray-900 dark:text-slate-100 mb-2">{t('employees.import.completeTitle')}</h2>
          <p className="text-gray-600 dark:text-slate-300 mb-6 tabular-nums">
            <Trans
              i18nKey="employees.import.completeBody"
              values={{ count: result.imported_count }}
              components={{ b: <span className="font-bold text-green-700 dark:text-emerald-300" /> }}
            />
            {result.skipped_count > 0 && (
              <> <span className="text-yellow-700 dark:text-amber-300">{t('employees.import.skippedRows', { count: result.skipped_count })}</span></>
            )}
          </p>

          {result.errors.length > 0 && (
            <div className="mb-6 text-left">
              <h3 className="text-sm font-medium text-red-700 dark:text-red-300 mb-2">{t('employees.import.failedRows')}</h3>
              <div className="bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 rounded-lg p-3 text-sm text-red-700 dark:text-red-300 max-h-40 overflow-auto">
                {result.errors.map((row) => (
                  <div key={row.row_number} className="mb-1 tabular-nums">
                    {t('employees.import.rowError', { row: row.row_number, errors: row.errors.map(e => `${e.field}: ${e.message}`).join(', ') })}
                  </div>
                ))}
              </div>
            </div>
          )}

          <button
            type="button"
            onClick={() => navigate('/employees')}
            className="btn-primary w-full sm:w-auto"
          >
            {t('employees.import.viewEmployees')}
          </button>
        </div>
      )}
    </div>
  );
}
