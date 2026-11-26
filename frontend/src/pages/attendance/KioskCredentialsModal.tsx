import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertCircle, Copy, Loader2, Plus, ShieldCheck, Trash2 } from 'lucide-react';
import {
  createKioskCredential,
  listKioskCredentials,
  revokeKioskCredential,
  type CreateKioskCredentialResponse,
} from '@/api/kiosk';
import { EmptyState } from '@/components/ui/EmptyState';
import { Modal } from '@/components/ui/Modal';
import { Skeleton } from '@/components/ui/Skeleton';
import { useConfirm } from '@/lib/confirm';
import { getErrorMessage } from '@/lib/utils';
import { formatDateTime } from './attendanceHelpers';

export function KioskCredentialsModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const confirmDialog = useConfirm();
  const queryClient = useQueryClient();
  const labelInputId = useId();
  const [label, setLabel] = useState('');
  const [created, setCreated] = useState<CreateKioskCredentialResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [hasCopied, setHasCopied] = useState(false);
  const [revokingId, setRevokingId] = useState<string | null>(null);
  const [error, setError] = useState('');
  const copyTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
    };
  }, []);

  const { data: credentials = [], isLoading, isError: listError, refetch } = useQuery({
    queryKey: ['kiosk-credentials'],
    queryFn: listKioskCredentials,
  });

  const createMut = useMutation({
    mutationFn: (l: string) => createKioskCredential(l),
    onSuccess: (data) => {
      setCreated(data);
      setCopied(false);
      setHasCopied(false);
      setLabel('');
      setError('');
      queryClient.invalidateQueries({ queryKey: ['kiosk-credentials'] });
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      setError(getErrorMessage(e, t('attendance.kioskModal.createFailed')));
    },
  });

  const revokeMut = useMutation({
    mutationFn: (id: string) => revokeKioskCredential(id),
    onMutate: (id) => setRevokingId(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['kiosk-credentials'] }),
    onError: (e) => setError(getErrorMessage(e, t('attendance.kioskModal.revokeFailed'))),
    onSettled: () => setRevokingId(null),
  });

  const copyUrl = async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setHasCopied(true);
      setError('');
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
      copyTimerRef.current = window.setTimeout(() => {
        copyTimerRef.current = null;
        setCopied(false);
      }, 2000);
    } catch {
      setError(t('attendance.kioskModal.copyFailed'));
    }
  };

  const confirmDiscardUncopied = async (): Promise<boolean> => {
    if (!created || hasCopied) return true;
    return confirmDialog({
      description: t('attendance.kioskModal.hideUncopiedConfirm'),
      confirmLabel: t('attendance.kioskModal.doneHide'),
      variant: 'warning',
    });
  };

  const handleHideCreated = async () => {
    if (!(await confirmDiscardUncopied())) return;
    setCreated(null);
  };

  const handleRequestClose = async () => {
    if (!(await confirmDiscardUncopied())) return;
    onClose();
  };

  return (
    <Modal open onClose={() => void handleRequestClose()} title={t('attendance.kioskModal.title')} maxWidth="max-w-2xl">
      <div>
        <p className="text-sm text-gray-500 dark:text-slate-400">
          {t('attendance.kioskModal.body')}
        </p>

        {/* One-time URL display after creation */}
        {created && (
          <div className="mt-5 border border-emerald-200 dark:border-emerald-500/30 bg-emerald-50 dark:bg-emerald-500/10 rounded-xl p-4">
            <div className="flex items-start gap-2">
              <ShieldCheck className="w-5 h-5 text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-emerald-900 dark:text-emerald-100">
                  {t('attendance.kioskModal.copyNow')}
                </p>
                <p className="text-xs text-emerald-700 dark:text-emerald-300 mt-0.5">
                  {t('attendance.kioskModal.treatLikePassword')}
                </p>
                <div className="mt-3 flex flex-col sm:flex-row sm:items-center gap-2">
                  <input
                    readOnly
                    value={created.public_url}
                    aria-label={t('attendance.kioskModal.copyNow')}
                    onFocus={(e) => e.currentTarget.select()}
                    className="flex-1 min-w-0 px-3 py-2 bg-white dark:bg-slate-900 border border-emerald-300 dark:border-emerald-500/40 rounded-lg text-xs font-mono text-gray-800 dark:text-slate-100 outline-none truncate"
                  />
                  <button
                    type="button"
                    onClick={() => void copyUrl(created.public_url)}
                    className="min-h-11 sm:min-h-9 flex items-center justify-center gap-1.5 px-3 py-2 bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-medium rounded-lg transition-colors shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                  >
                    <Copy className="w-3.5 h-3.5" aria-hidden="true" />
                    {copied ? t('common.copied') : t('common.copy')}
                  </button>
                </div>
                <button
                  type="button"
                  onClick={() => void handleHideCreated()}
                  className="mt-3 min-h-11 sm:min-h-0 inline-flex items-center text-xs text-emerald-800 dark:text-emerald-300 hover:underline rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  {t('attendance.kioskModal.doneHide')}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Create form */}
        {!created && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!label.trim()) {
                setError(t('attendance.kioskModal.nameRequired'));
                return;
              }
              createMut.mutate(label.trim());
            }}
            className="mt-5 space-y-1.5"
          >
            <label htmlFor={labelInputId} className="form-label">
              {t('attendance.kioskModal.cols.label')}
            </label>
            <div className="flex flex-col sm:flex-row gap-2">
              <input
                id={labelInputId}
                type="text"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder={t('attendance.kioskModal.namePlaceholder')}
                maxLength={100}
                className="form-input flex-1 min-w-0"
              />
              <button
                type="submit"
                disabled={createMut.isPending}
                className="btn-primary !py-2 min-h-11 sm:!min-h-10 shrink-0"
              >
                {createMut.isPending ? (
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Plus className="w-4 h-4" aria-hidden="true" />
                )}
                {createMut.isPending ? t('common.creating') : t('attendance.kioskModal.createLink')}
              </button>
            </div>
          </form>
        )}
        {error && (
          <p role="alert" className="mt-2 text-sm text-red-600 dark:text-red-400 flex items-center gap-1.5">
            <AlertCircle className="w-4 h-4 shrink-0" aria-hidden="true" /> {error}
          </p>
        )}

        {/* List */}
        <div className="mt-6">
          <h4 className="text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-2">
            {t('attendance.kioskModal.existing')}
          </h4>
          {isLoading ? (
            <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-2">
              {[0, 1].map((i) => (
                <Skeleton key={i} className="h-14 w-full rounded-xl" />
              ))}
            </div>
          ) : listError ? (
            // Must not read as "no links yet" — a 403 or network failure is a
            // different situation entirely.
            <div
              role="alert"
              className="rounded-xl border border-red-200 dark:border-red-500/30 bg-red-50/70 dark:bg-red-500/10 p-3 text-sm text-red-600 dark:text-red-400 flex flex-wrap items-center justify-between gap-2"
            >
              <span className="flex items-center gap-1.5">
                <AlertCircle className="w-4 h-4 shrink-0" aria-hidden="true" />
                {t('attendance.kioskModal.loadFailed')}
              </span>
              <button
                type="button"
                onClick={() => void refetch()}
                className="btn-secondary min-h-11 sm:min-h-9 !py-1.5 !px-3 text-xs shrink-0"
              >
                {t('common.retry')}
              </button>
            </div>
          ) : credentials.length === 0 ? (
            <EmptyState compact title={t('attendance.kioskModal.empty')} />
          ) : (
            <>
              {/* Mobile cards */}
              <div className="md:hidden border border-gray-200 dark:border-slate-800 rounded-xl divide-y divide-gray-100 dark:divide-slate-800 overflow-hidden">
                {credentials.map((c) => {
                  const revoked = c.revoked_at != null;
                  const isRevoking = revokingId === c.id && revokeMut.isPending;
                  const revokeLabel = `${t('attendance.kioskModal.revoke')} \u2014 ${c.label}`;
                  return (
                    <div key={c.id} className={`p-3.5 space-y-2 ${revoked ? 'opacity-75' : ''}`}>
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{c.label}</p>
                          <p className="text-xs font-mono text-gray-500 dark:text-slate-400">{c.token_prefix}…</p>
                        </div>
                        <div className="flex items-center gap-1.5 shrink-0">
                          {revoked ? (
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 dark:bg-slate-800 text-gray-600 dark:text-slate-400">
                              {t('attendance.kioskModal.revoked')}
                            </span>
                          ) : (
                            <>
                              <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-300">
                                {t('common.active')}
                              </span>
                              <button
                                type="button"
                                onClick={async () => {
                                  const ok = await confirmDialog({
                                    description: t('attendance.kioskModal.revokeConfirm', { label: c.label }),
                                    variant: 'danger',
                                  });
                                  if (ok) revokeMut.mutate(c.id);
                                }}
                                disabled={isRevoking}
                                className="min-h-11 min-w-11 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/10 transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                                title={revokeLabel}
                                aria-label={revokeLabel}
                              >
                                {isRevoking ? (
                                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                                ) : (
                                  <Trash2 className="w-4 h-4" aria-hidden="true" />
                                )}
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                      <div className="grid grid-cols-2 gap-2 text-xs text-gray-500 dark:text-slate-400">
                        <div>
                          <span className="block text-[10px] uppercase tracking-wider text-gray-500 dark:text-slate-400">
                            {t('attendance.kioskModal.cols.created')}
                          </span>
                          <span className="tabular-nums">{formatDateTime(c.created_at)}</span>
                        </div>
                        <div>
                          <span className="block text-[10px] uppercase tracking-wider text-gray-500 dark:text-slate-400">
                            {t('attendance.kioskModal.cols.lastUsed')}
                          </span>
                          <span className="tabular-nums">{formatDateTime(c.last_used_at)}</span>
                          {c.last_used_ip && (
                            <span className="block font-mono text-gray-500 dark:text-slate-400">{c.last_used_ip}</span>
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Desktop table */}
              <div className="hidden md:block border border-gray-200 dark:border-slate-800 rounded-xl overflow-x-auto">
                <table className="w-full text-sm">
                  <thead className="bg-gray-50 dark:bg-slate-800/60">
                    <tr>
                      <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        {t('attendance.kioskModal.cols.label')}
                      </th>
                      <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        {t('attendance.kioskModal.cols.prefix')}
                      </th>
                      <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        {t('attendance.kioskModal.cols.created')}
                      </th>
                      <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        {t('attendance.kioskModal.cols.lastUsed')}
                      </th>
                      <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        {t('common.status')}
                      </th>
                      <th className="px-3 py-2 text-right text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider whitespace-nowrap">
                        <span className="sr-only">{t('common.actions')}</span>
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                    {credentials.map((c) => {
                      const revoked = c.revoked_at != null;
                      const isRevoking = revokingId === c.id && revokeMut.isPending;
                      const revokeLabel = `${t('attendance.kioskModal.revoke')} \u2014 ${c.label}`;
                      return (
                        <tr key={c.id} className={revoked ? 'opacity-75' : ''}>
                          <td className="px-3 py-2.5 text-gray-900 dark:text-slate-100 font-medium">{c.label}</td>
                          <td className="px-3 py-2.5 text-xs font-mono text-gray-500 dark:text-slate-400 whitespace-nowrap">
                            {c.token_prefix}…
                          </td>
                          <td className="px-3 py-2.5 text-xs text-gray-500 dark:text-slate-400 whitespace-nowrap tabular-nums">
                            {formatDateTime(c.created_at)}
                          </td>
                          <td className="px-3 py-2.5 text-xs text-gray-500 dark:text-slate-400 tabular-nums">
                            <div>{formatDateTime(c.last_used_at)}</div>
                            {c.last_used_ip && (
                              <div className="text-gray-500 dark:text-slate-400 font-mono">{c.last_used_ip}</div>
                            )}
                          </td>
                          <td className="px-3 py-2.5">
                            {revoked ? (
                              <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 dark:bg-slate-800 text-gray-500 dark:text-slate-400">
                                {t('attendance.kioskModal.revoked')}
                              </span>
                            ) : (
                              <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-300">
                                {t('common.active')}
                              </span>
                            )}
                          </td>
                          <td className="px-3 py-2.5 text-right">
                            {!revoked && (
                              <button
                                type="button"
                                onClick={async () => {
                                  const ok = await confirmDialog({
                                    description: t('attendance.kioskModal.revokeConfirm', {
                                      label: c.label,
                                    }),
                                    variant: 'danger',
                                  });
                                  if (ok) {
                                    revokeMut.mutate(c.id);
                                  }
                                }}
                                disabled={isRevoking}
                                className="min-h-11 min-w-11 md:min-h-9 md:min-w-9 inline-flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/10 transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                                title={revokeLabel}
                                aria-label={revokeLabel}
                              >
                                {isRevoking ? (
                                  <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                                ) : (
                                  <Trash2 className="w-3.5 h-3.5" aria-hidden="true" />
                                )}
                              </button>
                            )}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}
