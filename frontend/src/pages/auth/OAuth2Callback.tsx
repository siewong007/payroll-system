import { useCallback, useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import api from '@/api/client';
import { useAuth } from '@/context/AuthContext';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { safeRedirectPath } from '@/lib/utils';
import { TwoFactorPrompt } from '@/components/TwoFactorPrompt';
import { BrandLogo } from '@/components/ui/BrandLogo';
import type { LoginResponse, User } from '@/types';

/**
 * The callback outcome arrives in the URL fragment and must be read once per
 * page load: reading also clears the hash, so a second read sees nothing.
 *
 * This deliberately lives outside the component. StrictMode remounts it in
 * development, and a remount resets hooks — so a `useRef`/`useState` guard still
 * ran the effect a second time, found an empty hash, and overwrote the real
 * outcome with "Missing authentication data". That masked both the server's
 * error message and the 2FA prompt.
 */
let callbackParams: URLSearchParams | null = null;
let inflightRefresh: Promise<LoginResponse> | null = null;

function takeCallbackParams(): URLSearchParams {
  const fromHash = new URLSearchParams(window.location.hash.substring(1));

  // Only a hash that actually carried something replaces the memo. Caching an
  // empty read would poison every later mount in the same JS context — and the
  // remount is precisely when the hash is already gone.
  if (Array.from(fromHash.keys()).length > 0) {
    callbackParams = fromHash;
    // The fragment is single-use and shouldn't linger in the URL or history.
    window.history.replaceState(null, '', window.location.pathname);
  }

  return callbackParams ?? fromHash;
}

function consumeOAuth2Redirect(): string | null {
  try {
    const target = safeRedirectPath(sessionStorage.getItem('oauth2_login_redirect'));
    sessionStorage.removeItem('oauth2_login_redirect');
    return target;
  } catch {
    return null;
  }
}

export function OAuth2Callback() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { user, isLoading, setSession } = useAuth();
  const [error, setError] = useState('');
  const [mfaToken, setMfaToken] = useState<string | null>(null);

  const goPostLogin = useCallback(
    (loggedInUser: User) => {
      const storedRedirect = consumeOAuth2Redirect();
      if (loggedInUser.must_change_password) {
        navigate('/change-password', { replace: true });
        return;
      }
      navigate(storedRedirect ?? (hasOnlyEmployeeRole(loggedInUser) ? '/portal' : '/'), { replace: true });
    },
    [navigate],
  );

  useEffect(() => {
    const params = takeCallbackParams();

    const status = params.get('status');
    const pendingMfaToken = params.get('mfa_token');
    // The backend redirects here with a human-readable reason when the exchange
    // fails, rather than leaving the browser on a raw JSON error body.
    const serverError = params.get('error');

    if (serverError) {
      setError(serverError);
      return;
    }

    if (pendingMfaToken) {
      setMfaToken(pendingMfaToken);
      return;
    }

    if (status !== 'ok') {
      setError(t('auth.oauth.missingData'));
      return;
    }

    if (isLoading) {
      return;
    }

    if (user) {
      goPostLogin(user);
      return;
    }

    let cancelled = false;
    const promise =
      inflightRefresh ??
      api.post<LoginResponse>('/auth/refresh', {}).then((res) => res.data);
    inflightRefresh = promise;

    promise
      .then((data) => {
        if (cancelled) return;
        setSession(data.token, data.user);
        goPostLogin(data.user);
      })
      .catch(() => {
        inflightRefresh = null;
        if (!cancelled) {
          setError(t('auth.oauth.processFailed'));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [goPostLogin, isLoading, setSession, t, user]);

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 px-4">
        <div role="alert" className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-8 max-w-md w-full text-center space-y-4">
          <div className="w-16 h-16 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center mx-auto">
            <svg className="w-8 h-8 text-red-500 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
          <p className="text-sm text-gray-600 dark:text-slate-300">{error}</p>
          <Link to="/login" className="inline-block text-sm text-black dark:text-indigo-400 font-medium hover:underline">
            {t('auth.backToLogin')}
          </Link>
        </div>
      </div>
    );
  }

  if (mfaToken) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 px-4">
        <div className="w-full max-w-md">
          <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8">
            <div className="text-center mb-8">
              <BrandLogo variant="lockup-dark" className="h-12 w-auto mx-auto mb-4 rounded-xl" />
            </div>
            <TwoFactorPrompt mfaToken={mfaToken} onSuccess={goPostLogin} />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 px-4">
      <div className="text-gray-500 dark:text-slate-400">{t('auth.oauth.completing')}</div>
    </div>
  );
}
