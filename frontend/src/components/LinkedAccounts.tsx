import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link2 } from 'lucide-react';
import {
  getGoogleAuthorizeUrl,
  getLinkedAccounts,
  getOAuth2Providers,
  unlinkOAuth2Provider,
} from '@/api/oauth2';
import { getErrorMessage } from '@/lib/utils';
import { GoogleIcon } from '@/components/ui/GoogleIcon';
import { Modal } from '@/components/ui/Modal';
import { TurnstileWidget, type TurnstileWidgetRef } from '@/components/TurnstileWidget';

export function LinkedAccounts() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [error, setError] = useState('');
  const [confirmingUnlink, setConfirmingUnlink] = useState(false);
  const turnstileRef = useRef<TurnstileWidgetRef>(null);
  const turnstileToken = useRef<string | undefined>(undefined);

  const { data: accounts = [], isLoading } = useQuery({
    queryKey: ['oauth2-accounts'],
    queryFn: getLinkedAccounts,
  });
  const { data: providers = [] } = useQuery({
    queryKey: ['oauth2-providers'],
    queryFn: getOAuth2Providers,
    staleTime: 300_000,
  });

  const google = accounts.find((a) => a.provider === 'google');
  const googleEnabled = providers.some((p) => p.provider === 'google' && p.enabled);

  const unlink = useMutation({
    mutationFn: unlinkOAuth2Provider,
    onSuccess: () => {
      setError('');
      setConfirmingUnlink(false);
      queryClient.invalidateQueries({ queryKey: ['oauth2-accounts'] });
    },
    onError: (err) => setError(getErrorMessage(err, t('auth.linkedAccounts.unlinkFailed'))),
  });

  const startLink = async () => {
    setError('');
    try {
      // The /oauth2/link route navigates back to this page on success.
      sessionStorage.setItem('oauth2_link_return', window.location.pathname);
      const token = turnstileToken.current;
      turnstileToken.current = undefined;
      window.location.href = await getGoogleAuthorizeUrl('link', token);
    } catch (err) {
      sessionStorage.removeItem('oauth2_link_return');
      turnstileRef.current?.reset();
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
          <Link2 className="w-4 h-4 text-gray-400" />
          <span className="section-title">{t('auth.linkedAccounts.title')}</span>
        </div>
        {google && <span className="badge badge-approved ml-auto">{t('auth.linkedAccounts.linkedBadge')}</span>}
      </div>

      <p className="text-sm text-gray-500 mb-4">
        {t('auth.linkedAccounts.description')}
      </p>

      {error && (
        <div className="mb-4 bg-red-50 border border-red-100 text-red-600 text-sm px-4 py-3 rounded-xl">
          {error}
        </div>
      )}

      <div className="py-4 flex items-center gap-3">
        {google?.avatar_url ? (
          <img src={google.avatar_url} alt="" className="w-8 h-8 rounded-full" />
        ) : (
          <GoogleIcon />
        )}
        <div className="min-w-0 flex-1">
          <span className="text-sm font-medium text-gray-900">Google</span>{/* i18n-ok: brand name */}
          <p className="text-xs text-gray-500 mt-1">
            {isLoading
              ? t('common.loading')
              : google
                ? google.provider_email
                  ? t('auth.linkedAccounts.linkedAs', { email: google.provider_email })
                  : t('auth.linkedAccounts.linked')
                : t('auth.linkedAccounts.notLinked')}
          </p>
        </div>
        {google ? (
          <button
            type="button"
            onClick={() => setConfirmingUnlink(true)}
            disabled={unlink.isPending}
            className="text-sm font-medium text-red-600 hover:text-red-700 disabled:opacity-50 whitespace-nowrap"
          >
            {unlink.isPending ? t('auth.linkedAccounts.unlinking') : t('auth.linkedAccounts.unlink')}
          </button>
        ) : (
          googleEnabled && (
            <div className="flex flex-col items-end gap-2">
              <TurnstileWidget
                ref={turnstileRef}
                onVerify={(t) => {
                  turnstileToken.current = t;
                }}
                onExpire={() => {
                  turnstileToken.current = undefined;
                }}
                onError={() => {
                  turnstileToken.current = undefined;
                }}
              />
              <button
                type="button"
                onClick={startLink}
                className="text-sm font-medium text-black hover:underline whitespace-nowrap"
              >
                {t('auth.linkedAccounts.linkAccount')}
              </button>
            </div>
          )
        )}
      </div>

      <Modal
        open={confirmingUnlink}
        onClose={() => setConfirmingUnlink(false)}
        title={t('auth.linkedAccounts.unlinkTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end gap-2">
            <button type="button" className="btn-secondary !min-h-0 !py-2" onClick={() => setConfirmingUnlink(false)}>
              {t('common.cancel')}
            </button>
            <button
              type="button"
              onClick={() => unlink.mutate('google')}
              disabled={unlink.isPending}
              className="btn-primary !min-h-0 !py-2 !bg-none !bg-red-600 hover:!bg-red-700"
            >
              {unlink.isPending ? t('auth.linkedAccounts.unlinking') : t('auth.linkedAccounts.unlinkSubmit')}
            </button>
          </div>
        }
      >
        <p className="text-sm text-gray-600">
          {t('auth.linkedAccounts.unlinkBody')}
        </p>
      </Modal>
    </section>
  );
}
