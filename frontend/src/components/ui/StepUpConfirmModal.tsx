import { useEffect, useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import type { StepUpCredentials } from '@/types';

interface StepUpConfirmModalProps {
  open: boolean;
  title: string;
  description?: ReactNode;
  confirmLabel: string;
  pendingLabel?: string;
  variant?: 'primary' | 'danger';
  pending?: boolean;
  error?: string;
  onClose: () => void;
  onConfirm: (credentials: StepUpCredentials) => void;
}

export function StepUpConfirmModal({
  open,
  title,
  description,
  confirmLabel,
  pendingLabel,
  variant = 'primary',
  pending = false,
  error,
  onClose,
  onConfirm,
}: StepUpConfirmModalProps) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const twoFactorEnabled = Boolean(user?.two_factor_enabled);
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const formId = useId();

  useEffect(() => {
    if (!open) {
      setPassword('');
      setTotpCode('');
    }
  }, [open]);

  const canSubmit = Boolean(password) && (!twoFactorEnabled || Boolean(totpCode.trim())) && !pending;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;
    onConfirm({
      actor_password: password,
      ...(twoFactorEnabled && totpCode.trim()
        ? { actor_totp_code: totpCode.trim() }
        : {}),
    });
  };

  return (
    <Modal
      open={open}
      onClose={() => {
        if (!pending) onClose();
      }}
      title={title}
      maxWidth="max-w-md"
      footer={
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button type="button" disabled={pending} onClick={onClose} className="btn-secondary">
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={!canSubmit}
            className={
              variant === 'danger'
                ? 'inline-flex items-center justify-center gap-2 px-4 py-2 min-h-[44px] text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-500 transition-colors disabled:opacity-50'
                : 'btn-primary inline-flex items-center justify-center gap-2 disabled:opacity-50'
            }
          >
            {pending ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                {pendingLabel ?? confirmLabel}
              </>
            ) : (
              confirmLabel
            )}
          </button>
        </div>
      }
    >
      <form id={formId} onSubmit={handleSubmit} className="space-y-4">
        {error && (
          <div
            role="alert"
            className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25"
          >
            {error}
          </div>
        )}
        {description && <div className="text-sm text-gray-600 dark:text-slate-300">{description}</div>}
        <FormField label={t('users.confirmWithYourPassword')} required>
          {(aria) => (
            <input
              {...aria}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
              className="form-input"
              autoComplete="current-password"
              required
            />
          )}
        </FormField>
        {twoFactorEnabled && (
          <FormField label={t('auth.twoFactor.title')} required>
            {(aria) => (
              <input
                {...aria}
                type="text"
                inputMode="text"
                autoComplete="one-time-code"
                value={totpCode}
                onChange={(e) => setTotpCode(e.target.value)}
                placeholder="123456"
                className="form-input text-center tracking-widest"
                required
              />
            )}
          </FormField>
        )}
      </form>
    </Modal>
  );
}
