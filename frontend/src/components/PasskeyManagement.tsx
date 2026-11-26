import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Fingerprint, Plus, Trash2, Pencil, Check, X } from 'lucide-react';
import { listPasskeys, deletePasskey, renamePasskey, passkeyRegisterBegin, passkeyRegisterComplete } from '@/api/passkey';
import type { PasskeyInfo } from '@/api/passkey';
import { createPasskeyCredential, isWebAuthnSupported } from '@/lib/webauthn';
import { formatDate, getErrorMessage } from '@/lib/utils';
import { Modal } from '@/components/ui/Modal';

export function PasskeyManagement() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [registering, setRegistering] = useState(false);
  const [newName, setNewName] = useState('');
  const [password, setPassword] = useState('');
  const [showNameInput, setShowNameInput] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  const { data: passkeys, isLoading } = useQuery({
    queryKey: ['passkeys'],
    queryFn: listPasskeys,
  });

  const deleteMutation = useMutation({
    mutationFn: deletePasskey,
    onSuccess: () => {
      setError('');
      queryClient.invalidateQueries({ queryKey: ['passkeys'] });
      setDeletingId(null);
      setSuccess(t('auth.passkeys.deleted'));
      setTimeout(() => setSuccess(''), 3000);
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('common.error'))),
  });

  const renameMutation = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) => renamePasskey(id, name),
    onSuccess: () => {
      setError('');
      queryClient.invalidateQueries({ queryKey: ['passkeys'] });
      setEditingId(null);
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('common.error'))),
  });

  if (!isWebAuthnSupported()) {
    return null;
  }

  const handleRegister = async () => {
    setError('');
    setRegistering(true);
    try {
      // Step 1: Get challenge from server
      const { challenge_id, options } = await passkeyRegisterBegin(password);

      // Step 2: Browser WebAuthn ceremony
      const credential = await createPasskeyCredential(options.publicKey);

      // Step 3: Send credential to server
      const name = newName.trim() || t('auth.passkeys.defaultName');
      await passkeyRegisterComplete(challenge_id, credential, name);

      queryClient.invalidateQueries({ queryKey: ['passkeys'] });
      setShowNameInput(false);
      setNewName('');
      setPassword('');
      setSuccess(t('auth.passkeys.registered'));
      setTimeout(() => setSuccess(''), 3000);
    } catch (err: unknown) {
      console.error('Passkey registration error:', err);
      setError(getErrorMessage(err, t('auth.passkeys.registerFailed')));
    } finally {
      setRegistering(false);
    }
  };

  return (
    <div className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <Fingerprint className="w-4 h-4 text-gray-400 dark:text-slate-400" />
          <span className="section-title">{t('auth.passkeys.title')}</span>
        </div>
        {passkeys && passkeys.length > 0 && (
          <span className="badge badge-cancelled ml-auto">{passkeys.length}</span>
        )}
      </div>

      <p className="text-sm text-gray-500 dark:text-slate-400 mb-4">
        {t('auth.passkeys.description')}
      </p>

      {error && (
        <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl mb-4">
          {error}
        </div>
      )}
      {success && (
        <div role="status" className="bg-green-50 dark:bg-emerald-500/10 border border-green-100 dark:border-emerald-500/30 text-green-600 dark:text-emerald-300 text-sm px-4 py-3 rounded-xl mb-4">
          {success}
        </div>
      )}

      {isLoading ? (
        <div className="flex justify-center py-8">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900 dark:border-slate-200" />
        </div>
      ) : (
        <div className="space-y-2 mb-4">
          {(!passkeys || passkeys.length === 0) ? (
            <p className="text-sm text-gray-500 dark:text-slate-400 py-4 text-center">{t('auth.passkeys.empty')}</p>
          ) : (
            passkeys.map((pk: PasskeyInfo) => (
              <div
                key={pk.id}
                className="flex items-center gap-3 p-3 rounded-xl border border-gray-100 dark:border-slate-800 hover:bg-gray-50 dark:hover:bg-slate-800/50 transition-colors"
              >
                <Fingerprint className="w-5 h-5 text-gray-400 dark:text-slate-400 shrink-0" />
                <div className="flex-1 min-w-0">
                  {editingId === pk.id ? (
                    <div className="flex items-center gap-1.5">
                      <input
                        type="text"
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && editName.trim()) {
                            renameMutation.mutate({ id: pk.id, name: editName.trim() });
                          } else if (e.key === 'Escape') {
                            setEditingId(null);
                          }
                        }}
                        aria-label={t('auth.passkeys.rename')}
                        className="form-input !py-1.5 flex-1 min-w-0"
                        autoFocus
                      />
                      <button
                        type="button"
                        onClick={() => renameMutation.mutate({ id: pk.id, name: editName.trim() })}
                        disabled={!editName.trim() || renameMutation.isPending}
                        aria-label={t('common.save')}
                        className="min-h-10 min-w-10 inline-flex items-center justify-center text-green-600 dark:text-emerald-400 hover:bg-green-50 dark:hover:bg-emerald-500/15 rounded-lg disabled:opacity-50"
                      >
                        <Check className="w-4 h-4" />
                      </button>
                      <button
                        type="button"
                        onClick={() => setEditingId(null)}
                        aria-label={t('common.cancel')}
                        className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  ) : (
                    <>
                      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{pk.credential_name}</p>
                      <p className="text-xs text-gray-500 dark:text-slate-400">
                        {t('auth.passkeys.added', { date: formatDate(pk.created_at) })}
                        {pk.last_used_at && ` · ${t('auth.passkeys.lastUsed', { date: formatDate(pk.last_used_at) })}`}
                      </p>
                    </>
                  )}
                </div>
                {editingId !== pk.id && (
                  <div className="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      onClick={() => { setEditingId(pk.id); setEditName(pk.credential_name); }}
                      aria-label={t('auth.passkeys.rename')}
                      title={t('auth.passkeys.rename')}
                      className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg"
                    >
                      <Pencil className="w-3.5 h-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => setDeletingId(pk.id)}
                      aria-label={t('auth.passkeys.delete')}
                      title={t('auth.passkeys.delete')}
                      className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-lg"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                )}
              </div>
            ))
          )}
        </div>
      )}

      {/* Register new passkey */}
      {showNameInput ? (
        <div className="flex flex-wrap items-center gap-2">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder={t('auth.passkeys.namePlaceholder')}
            aria-label={t('auth.passkeys.namePlaceholder')}
            className="form-input flex-1 min-w-[160px]"
            autoFocus
          />
          <input
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
            aria-label={t('auth.twoFactor.currentPasswordPlaceholder')}
            className="form-input flex-1 min-w-[160px]"
          />
          <button
            type="button"
            onClick={handleRegister}
            disabled={registering || !password}
            className="btn-primary !py-2 !min-h-10"
          >
            {registering ? t('auth.passkeys.registering') : t('auth.passkeys.register')}
          </button>
          <button
            type="button"
            onClick={() => { setShowNameInput(false); setNewName(''); setPassword(''); }}
            aria-label={t('common.cancel')}
            className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 rounded-lg"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setShowNameInput(true)}
          className="min-h-10 flex items-center gap-2 text-sm font-semibold text-[var(--accent-1)] hover:opacity-80 transition-opacity"
        >
          <Plus className="w-4 h-4" />
          {t('auth.passkeys.add')}
        </button>
      )}

      <Modal
        open={deletingId !== null}
        onClose={() => setDeletingId(null)}
        title={t('auth.passkeys.deleteTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end gap-2">
            <button type="button" className="btn-secondary !min-h-0 !py-2" onClick={() => setDeletingId(null)}>
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={() => deletingId && deleteMutation.mutate(deletingId)}
              disabled={deleteMutation.isPending}
              className="btn-primary !min-h-0 !py-2 !bg-none !bg-red-600 hover:!bg-red-700"
            >
              {deleteMutation.isPending ? t('auth.passkeys.deleting') : t('auth.passkeys.deleteSubmit')}
            </button>
          </div>
        }
      >
        <p className="text-sm text-gray-600 dark:text-slate-300">
          {t('auth.passkeys.deleteBody')}
        </p>
      </Modal>
    </div>
  );
}
