import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link2 } from 'lucide-react';
import {
  getGoogleAuthorizeUrl,
  getLinkedAccounts,
  getOAuth2Providers,
  unlinkOAuth2Provider,
} from '@/api/oauth2';
import { totpStatus } from '@/api/totp';
import { FormField } from '@/components/ui/FormField';
import { getErrorMessage } from '@/lib/utils';
import { turnstileEnabled } from '@/lib/turnstile';
import { useTurnstileWaiter } from '@/lib/useTurnstileWaiter';
import { GoogleIcon } from '@/components/ui/GoogleIcon';
import { Modal } from '@/components/ui/Modal';
import { TurnstileWidget } from '@/components/TurnstileWidget';

export function LinkedAccounts() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [error, setError] = useState('');
  const [linking, setLinking] = useState(false);
  const [confirmingUnlink, setConfirmingUnlink] = useState(false);
  const [unlinkPassword, setUnlinkPassword] = useState('');
  const [unlinkTotpCode, setUnlinkTotpCode] = useState('');
  const {
    widgetRef: turnstileRef,
    waitForToken: waitForTurnstileToken,
    handleVerify: handleTurnstileVerify,
    handleExpire: handleTurnstileExpire,
    handleError: handleTurnstileError,
    resetWidget: resetTurnstileWidget,
  } = useTurnstileWaiter();

  const { data: accounts = [], isLoading } = useQuery({
    queryKey: ['oauth2-accounts'],
    queryFn: getLinkedAccounts,
  });
  const { data: providers = [] } = useQuery({
    queryKey: ['oauth2-providers'],
    queryFn: getOAuth2Providers,
    staleTime: 300_000,
  });
  const totpQuery = useQuery({
    queryKey: ['totp-status'],
    queryFn: async () => (await totpStatus()) ?? { enabled: false },
    retry: false,
  });
  const twoFactorEnabled = Boolean(totpQuery.data?.enabled);

  const google = accounts.find((a) => a.provider === 'google');
  const googleEnabled = providers.some((p) => p.provider === 'google' && p.enabled);

  const closeUnlinkModal = () => {
    setConfirmingUnlink(false);
    setUnlinkPassword('');
    setUnlinkTotpCode('');
  };

  const unlink = useMutation({
    mutationFn: ({ provider, password, totpCode }: { provider: string; password: string; totpCode?: string }) =>
      unlinkOAuth2Provider(provider, password, totpCode),
    onSuccess: () => {
      setError('');
      closeUnlinkModal();
      queryClient.invalidateQueries({ queryKey: ['oauth2-accounts'] });
    },
    onError: (err) => setError(getErrorMessage(err, t('auth.linkedAccounts.unlinkFailed'))),
  });

  const startLink = async () => {
    if (linking) return;
    setError('');
    setLinking(true);
    try {
      // The /oauth2/link route navigates back to this page on success.
      try {
        sessionStorage.setItem('oauth2_link_return', window.location.pathname + window.location.search);
      } catch {
        // Ignore restricted storage.
      }
      const token = await waitForTurnstileToken();
      if (turnstileEnabled() && !token) {
        try {
          sessionStorage.removeItem('oauth2_link_return');
        } catch {
          // Ignore restricted storage.
        }
        resetTurnstileWidget();
        setLinking(false);
        setError(t('auth.verificationSlow'));
        return;
      }
      window.location.href = await getGoogleAuthorizeUrl('link', token);
    } catch (err) {
      try {
        sessionStorage.removeItem('oauth2_link_return');
      } catch {
        // Ignore restricted storage.
      }
      resetTurnstileWidget();
      setLinking(false);
      setError(getErrorMessage(err, t('auth.googleUnavailable')));
    }
  };

  // Hidden only when there is nothing to do: no provider configured AND
  // nothing linked to manage. A linked row stays visible so it can be unlinked
  // even if the provider is later switched off.
  if (!googleEnabled && !google) {
    return null;
  }

  return (
    <section className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <Link2 className="w-4 h-4 text-gray-400 dark:text-slate-400" />
          <span className="section-title">{t('auth.linkedAccounts.title')}</span>
        </div>
        {google && <span className="badge badge-approved ml-auto">{t('auth.linkedAccounts.linkedBadge')}</span>}
      </div>

      <p className="text-sm text-gray-500 dark:text-slate-400 mb-4">
        {t('auth.linkedAccounts.description')}
      </p>

      {error && (
        <div role="alert" className="mb-4 bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl">
          {error}
        </div>
      )}

      <div className="py-4 flex flex-col sm:flex-row sm:items-center gap-3">
        <div className="flex items-center gap-3 min-w-0 flex-1">
          {google?.avatar_url && /^https:\/\//i.test(google.avatar_url) ? (
            <img
              src={google.avatar_url}
              alt=""
              referrerPolicy="no-referrer"
              className="w-8 h-8 rounded-full shrink-0"
            />
          ) : (
            <GoogleIcon />
          )}
          <div className="min-w-0 flex-1">
            <span className="text-sm font-medium text-gray-900 dark:text-slate-100">Google</span>{/* i18n-ok: brand name */}
            <p className="text-xs text-gray-500 dark:text-slate-400 mt-1 truncate">
              {isLoading
                ? t('common.loading')
                : google
                  ? google.provider_email
                    ? t('auth.linkedAccounts.linkedAs', { email: google.provider_email })
                    : t('auth.linkedAccounts.linked')
                  : t('auth.linkedAccounts.notLinked')}
            </p>
          </div>
        </div>
        {google ? (
          <button
            type="button"
            onClick={() => setConfirmingUnlink(true)}
            disabled={unlink.isPending}
            className="self-start sm:self-center min-h-10 px-2 text-sm font-medium text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 disabled:opacity-50 whitespace-nowrap"
          >
            {unlink.isPending ? t('auth.linkedAccounts.unlinking') : t('auth.linkedAccounts.unlink')}
          </button>
        ) : (
          googleEnabled && (
            <div className="flex flex-col items-start sm:items-end gap-2 max-w-full overflow-x-auto">
              <TurnstileWidget
                ref={turnstileRef}
                action="oauth2_google"
                onVerify={handleTurnstileVerify}
                onExpire={handleTurnstileExpire}
                onError={handleTurnstileError}
              />
              <button
                type="button"
                onClick={startLink}
                disabled={linking}
                className="min-h-10 text-sm font-semibold text-[var(--accent-1)] hover:underline disabled:opacity-50 whitespace-nowrap"
              >
                {t('auth.linkedAccounts.linkAccount')}
              </button>
            </div>
          )
        )}
      </div>

      <Modal
        open={confirmingUnlink}
        onClose={closeUnlinkModal}
        title={t('auth.linkedAccounts.unlinkTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end gap-2">
            <button type="button" className="btn-secondary !min-h-0 !py-2" onClick={closeUnlinkModal}>
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              form="unlink-google-form"
              disabled={unlink.isPending || !unlinkPassword || (twoFactorEnabled && !unlinkTotpCode.trim())}
              className="btn-primary !min-h-0 !py-2 !bg-none !bg-red-600 hover:!bg-red-700"
            >
              {unlink.isPending ? t('auth.linkedAccounts.unlinking') : t('auth.linkedAccounts.unlinkSubmit')}
            </button>
          </div>
        }
      >
        <form
          id="unlink-google-form"
          onSubmit={(e) => {
            e.preventDefault();
            if (!unlinkPassword || (twoFactorEnabled && !unlinkTotpCode.trim())) return;
            unlink.mutate({
              provider: 'google',
              password: unlinkPassword,
              ...(twoFactorEnabled && unlinkTotpCode.trim() ? { totpCode: unlinkTotpCode.trim() } : {}),
            });
          }}
          className="space-y-4"
        >
          <p className="text-sm text-gray-600 dark:text-slate-300">
            {t('auth.linkedAccounts.unlinkBody')}
          </p>
          <FormField label={t('auth.twoFactor.confirmPasswordPrompt')} required>
            {(aria) => (
              <input
                {...aria}
                type="password"
                autoComplete="current-password"
                value={unlinkPassword}
                onChange={(e) => setUnlinkPassword(e.target.value)}
                placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
                className="form-input !p-2.5"
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
                  value={unlinkTotpCode}
                  onChange={(e) => setUnlinkTotpCode(e.target.value)}
                  placeholder="123456"
                  className="form-input !p-2.5 text-center tracking-widest"
                  required
                />
              )}
            </FormField>
          )}
        </form>
      </Modal>
    </section>
  );
}
