import { useCallback, useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import api from '@/api/client';
import { useAuth } from '@/context/AuthContext';
import { createSingleReadUrlParams } from '@/lib/oauthParams';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { safeRedirectPath, translateServerMessage } from '@/lib/utils';
import { TwoFactorPrompt } from '@/components/TwoFactorPrompt';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { Skeleton } from '@/components/ui/Skeleton';
import { AuthLayout } from '@/components/layout/AuthLayout';
import type { LoginResponse, User } from '@/types';

const callbackUrlParams = createSingleReadUrlParams('hash');
let inflightRefresh: Promise<LoginResponse> | null = null;

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
    const params = callbackUrlParams.take();

    const status = params.get('status');
    const pendingMfaToken = params.get('mfa_token');
    // The backend redirects here with a human-readable reason when the exchange
    // fails, rather than leaving the browser on a raw JSON error body.
    const serverError = params.get('error');

    if (serverError) {
      setError(translateServerMessage(serverError));
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
      callbackUrlParams.clear();
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
        inflightRefresh = null;
        callbackUrlParams.clear();
        if (cancelled) return;
        setSession(data.token, data.user);
        goPostLogin(data.user);
      })
      .catch(() => {
        inflightRefresh = null;
        callbackUrlParams.clear();
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
      <AuthLayout>
        <div role="alert" className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8 text-center space-y-4">
          <div className="w-16 h-16 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center mx-auto">
            <svg className="w-8 h-8 text-red-600 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
          <p className="text-sm text-gray-600 dark:text-slate-300">{error}</p>
          <Link
            to="/login"
            onClick={() => {
              callbackUrlParams.clear();
            }}
            className="inline-flex min-h-11 items-center justify-center px-3 text-sm text-black dark:text-indigo-400 font-medium hover:underline rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            {t('auth.backToLogin')}
          </Link>
        </div>
      </AuthLayout>
    );
  }

  if (mfaToken) {
    return (
      <AuthLayout>
        <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8">
          <div className="text-center mb-8">
            <BrandLogo variant="lockup-dark" className="h-12 w-auto mx-auto mb-4 rounded-xl" />
          </div>
          <TwoFactorPrompt
            mfaToken={mfaToken}
            onSuccess={(u) => {
              callbackUrlParams.clear();
              goPostLogin(u);
            }}
            onBack={() => {
              callbackUrlParams.clear();
              navigate('/login', { replace: true });
            }}
          />
        </div>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout>
      <div
        role="status"
        className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8 text-center space-y-4"
      >
        <BrandLogo variant="lockup-dark" className="h-12 w-auto mx-auto rounded-xl" />
        <div className="space-y-2.5 py-2">
          <div className="spinner mx-auto" />
          <Skeleton className="h-3 w-32 mx-auto" />
        </div>
        <p className="text-sm text-gray-600 dark:text-slate-300">{t('auth.oauth.completing')}</p>
      </div>
    </AuthLayout>
  );
}
