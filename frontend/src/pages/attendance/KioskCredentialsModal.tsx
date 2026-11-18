import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertCircle, Copy, Plus, ShieldCheck, Trash2 } from 'lucide-react';
import {
  createKioskCredential,
  listKioskCredentials,
  revokeKioskCredential,
  type CreateKioskCredentialResponse,
} from '@/api/kiosk';
import { Modal } from '@/components/ui/Modal';
import { useConfirm } from '@/lib/confirm';
import { getErrorMessage } from '@/lib/utils';
import { formatDateTime } from './attendanceHelpers';

export function KioskCredentialsModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const confirmDialog = useConfirm();
  const queryClient = useQueryClient();
  const [label, setLabel] = useState('');
  const [created, setCreated] = useState<CreateKioskCredentialResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState('');

  const { data: credentials = [], isLoading, isError: listError } = useQuery({
    queryKey: ['kiosk-credentials'],
    queryFn: listKioskCredentials,
  });

  const createMut = useMutation({
    mutationFn: (l: string) => createKioskCredential(l),
    onSuccess: (data) => {
      setCreated(data);
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
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['kiosk-credentials'] }),
    onError: (e) => setError(getErrorMessage(e, t('attendance.kioskModal.revokeFailed'))),
  });

  const copyUrl = async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard write may fail in non-HTTPS contexts; user can copy manually.
    }
  };

  return (
    <Modal open onClose={onClose} title={t('attendance.kioskModal.title')} maxWidth="max-w-2xl">
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
                <div className="mt-3 flex items-center gap-2">
                  <input
                    readOnly
                    value={created.public_url}
                    aria-label={t('attendance.kioskModal.copyNow')}
                    onFocus={(e) => e.currentTarget.select()}
                    className="flex-1 px-3 py-2 bg-white dark:bg-slate-900 border border-emerald-300 dark:border-emerald-500/40 rounded-lg text-xs font-mono text-gray-800 dark:text-slate-100 outline-none truncate"
                  />
                  <button
                    onClick={() => copyUrl(created.public_url)}
                    className="flex items-center gap-1.5 px-3 py-2 bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-medium rounded-lg transition-colors"
                  >
                    <Copy className="w-3.5 h-3.5" />
                    {copied ? t('common.copied') : t('common.copy')}
                  </button>
                </div>
                <button
                  onClick={() => setCreated(null)}
                  className="mt-3 text-xs text-emerald-800 dark:text-emerald-300 hover:underline"
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
            className="mt-5 flex gap-2"
          >
            <input
              type="text"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder={t('attendance.kioskModal.namePlaceholder')}
              aria-label={t('attendance.kioskModal.namePlaceholder')}
              maxLength={100}
              className="form-input flex-1"
            />
            <button
              type="submit"
              disabled={createMut.isPending}
              className="btn-primary !py-2 !min-h-9"
            >
              <Plus className="w-4 h-4" />
              {createMut.isPending ? t('common.creating') : t('attendance.kioskModal.createLink')}
            </button>
          </form>
        )}
        {error && (
          <p className="mt-2 text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
            <AlertCircle className="w-3.5 h-3.5" /> {error}
          </p>
        )}

        {/* List */}
        <div className="mt-6">
          <h4 className="text-xs font-semibold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-2">
            {t('attendance.kioskModal.existing')}
          </h4>
          {isLoading ? (
            <div className="flex items-center justify-center h-24">
              <div className="spinner" />
            </div>
          ) : listError ? (
            // Must not read as "no links yet" — a 403 or network failure is a
            // different situation entirely.
            <p className="text-sm text-red-600 flex items-center gap-1.5">
              <AlertCircle className="w-3.5 h-3.5" />
              {t('attendance.kioskModal.loadFailed')}
            </p>
          ) : credentials.length === 0 ? (
            <p className="text-sm text-gray-400 italic">{t('attendance.kioskModal.empty')}</p>
          ) : (
            <div className="border border-gray-200 rounded-xl overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-gray-50">
                  <tr>
                    {[
                      t('attendance.kioskModal.cols.label'),
                      t('attendance.kioskModal.cols.prefix'),
                      t('attendance.kioskModal.cols.created'),
                      t('attendance.kioskModal.cols.lastUsed'),
                      t('common.status'),
                      '',
                    ].map((h) => (
                      <th
                        key={h}
                        className="px-3 py-2 text-left text-xs font-semibold text-gray-500 uppercase tracking-wider"
                      >
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-100">
                  {credentials.map((c) => {
                    const revoked = c.revoked_at != null;
                    return (
                      <tr key={c.id} className={revoked ? 'opacity-60' : ''}>
                        <td className="px-3 py-2.5 text-gray-900 font-medium">{c.label}</td>
                        <td className="px-3 py-2.5 text-xs font-mono text-gray-500">
                          {c.token_prefix}…
                        </td>
                        <td className="px-3 py-2.5 text-xs text-gray-500">
                          {formatDateTime(c.created_at)}
                        </td>
                        <td className="px-3 py-2.5 text-xs text-gray-500">
                          <div>{formatDateTime(c.last_used_at)}</div>
                          {c.last_used_ip && (
                            <div className="text-gray-400 font-mono">{c.last_used_ip}</div>
                          )}
                        </td>
                        <td className="px-3 py-2.5">
                          {revoked ? (
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-500">
                              {t('attendance.kioskModal.revoked')}
                            </span>
                          ) : (
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 text-emerald-700">
                              {t('common.active')}
                            </span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 text-right">
                          {!revoked && (
                            <button
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
                              disabled={revokeMut.isPending}
                              className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 transition-colors disabled:opacity-50"
                              title={t('attendance.kioskModal.revoke')}
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}
