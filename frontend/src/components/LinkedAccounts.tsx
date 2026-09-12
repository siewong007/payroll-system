import { useRef, useState } from 'react';
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
import { TurnstileWidget, type TurnstileWidgetRef } from '@/components/TurnstileWidget';

export function LinkedAccounts() {
  const queryClient = useQueryClient();
  const [error, setError] = useState('');
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
      queryClient.invalidateQueries({ queryKey: ['oauth2-accounts'] });
    },
    onError: (err) => setError(getErrorMessage(err, 'Could not unlink the account')),
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
      setError(getErrorMessage(err, 'Google sign-in is not available'));
    }
  };

  // Hidden only when there is nothing to do: no provider configured AND
  // nothing linked to manage. A linked row stays visible so it can be unlinked
  // even if the provider is later switched off.
  if (!googleEnabled && !google) {
    return null;
  }

  return (
    <section className="bg-white rounded-2xl shadow p-6">
      <div className="flex items-start justify-between gap-4 mb-5">
        <div>
          <div className="flex items-center gap-2 text-gray-900 font-semibold">
            <Link2 className="w-5 h-5" /> Linked accounts
          </div>
          <p className="text-sm text-gray-500 mt-1">
            Sign in faster by connecting an external account.
          </p>
        </div>
      </div>

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
          <span className="text-sm font-medium text-gray-900">Google</span>
          <p className="text-xs text-gray-500 mt-1">
            {isLoading
              ? 'Loading…'
              : google
                ? `Linked${google.provider_email ? ` as ${google.provider_email}` : ''}`
                : 'Not linked'}
          </p>
        </div>
        {google ? (
          <button
            type="button"
            onClick={() => {
              if (window.confirm('Unlink your Google account? You can link it again later.')) {
                unlink.mutate('google');
              }
            }}
            disabled={unlink.isPending}
            className="text-sm font-medium text-red-600 hover:text-red-700 disabled:opacity-50 whitespace-nowrap"
          >
            {unlink.isPending ? 'Unlinking…' : 'Unlink'}
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
                Link account
              </button>
            </div>
          )
        )}
      </div>
    </section>
  );
}
