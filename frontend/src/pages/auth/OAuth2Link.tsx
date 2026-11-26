import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { linkGoogleAccount } from '@/api/oauth2';
import { getErrorMessage, safeRedirectPath } from '@/lib/utils';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { Skeleton } from '@/components/ui/Skeleton';
import { AuthLayout } from '@/components/layout/AuthLayout';

const RETURN_PATH_KEY = 'oauth2_link_return';

/**
 * Google lands here with ?code&state in the query string (the authorization
 * response is a redirect, not our fragment convention). Same single-read rule
 * as OAuth2Callback: the params are stripped from the URL on first read, so a
 * StrictMode remount must see the memo, not an empty search string.
 */
let linkParams: URLSearchParams | null = null;

function takeLinkParams(): URLSearchParams {
  const fromQuery = new URLSearchParams(window.location.search);

  if (Array.from(fromQuery.keys()).length > 0) {
    linkParams = fromQuery;
    window.history.replaceState(null, '', window.location.pathname);
  }

  return linkParams ?? fromQuery;
}

function readReturnPath(): string | null {
  try {
    return safeRedirectPath(sessionStorage.getItem(RETURN_PATH_KEY));
  } catch {
    return null;
  }
}

function clearReturnPath() {
  try {
    sessionStorage.removeItem(RETURN_PATH_KEY);
  } catch {
    // Ignore storage errors in restricted environments.
  }
}

export function OAuth2Link() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { user, isAuthenticated, isLoading } = useAuth();
  const [error, setError] = useState('');
  const [pendingLink, setPendingLink] = useState<{ code: string; state: string } | null>(null);
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const twoFactorEnabled = Boolean(user?.two_factor_enabled);

  // Where the LinkedAccounts card sat before the round-trip to Google.
  const returnTo = readReturnPath();

  useEffect(() => {
    // The page reload wiped the in-memory token; linking needs the session
    // the refresh restores, so nothing may proceed while it is still in flight.
    if (isLoading) {
      return;
    }

    const params = takeLinkParams();

    const providerError = params.get('error');
    if (providerError) {
      linkParams = null;
      setError(
        providerError === 'access_denied'
          ? t('auth.oauthLink.cancelled')
          : t('auth.oauthLink.failed'),
      );
      return;
    }

    if (!isAuthenticated) {
      linkParams = null;
      setError(t('auth.oauthLink.sessionLost'));
      return;
    }

    const code = params.get('code');
    const state = params.get('state');
    if (!code || !state) {
      linkParams = null;
      setError(t('auth.oauthLink.missingData'));
      return;
    }

    setPendingLink({ code, state });
  }, [isLoading, isAuthenticated, t]);

  const handleConfirm = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!pendingLink || !password || (twoFactorEnabled && !totpCode.trim())) {
      return;
    }
    setError('');
    setSubmitting(true);
    try {
      await linkGoogleAccount(
        pendingLink.code,
        pendingLink.state,
        password,
        ...(twoFactorEnabled && totpCode.trim() ? [totpCode.trim()] : []),
      );
      linkParams = null;
      clearReturnPath();
      navigate(user?.must_change_password ? '/change-password' : (returnTo ?? '/'), { replace: true });
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('auth.oauthLink.linkFailed')));
    } finally {
      setSubmitting(false);
    }
  };

  if (pendingLink) {
    return (
      <AuthLayout>
        <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8 space-y-5">
          <div className="text-center space-y-2">
            <BrandLogo variant="lockup-dark" className="h-10 w-auto mx-auto rounded-xl" />
            <h1 className="text-lg font-semibold text-gray-900 dark:text-slate-100">
              {t('auth.oauthLink.confirmTitle')}
            </h1>
            <p className="text-sm text-gray-600 dark:text-slate-300">
              {t('auth.oauthLink.confirmPrompt')}
            </p>
          </div>

          <form onSubmit={handleConfirm} className="space-y-4">
            {error && (
              <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm px-4 py-3 rounded-xl">
                {error}
              </div>
            )}

            <div>
              <label className="form-label" htmlFor="oauth-link-password">
                {t('auth.twoFactor.confirmPasswordPrompt')}
              </label>
              <input
                id="oauth-link-password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
                className="form-input !p-2.5"
                autoFocus
                required
              />
            </div>

            {twoFactorEnabled && (
              <div>
                <label className="form-label" htmlFor="oauth-link-totp">
                  {t('auth.twoFactor.title')}
                </label>
                <input
                  id="oauth-link-totp"
                  type="text"
                  inputMode="text"
                  autoComplete="one-time-code"
                  value={totpCode}
                  onChange={(e) => setTotpCode(e.target.value)}
                  placeholder="123456"
                  className="form-input !p-2.5 text-center tracking-widest"
                  required
                />
              </div>
            )}

            <button
              type="submit"
              disabled={submitting || !password || (twoFactorEnabled && !totpCode.trim())}
              className="btn-primary w-full inline-flex items-center justify-center gap-2"
            >
              {submitting ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {t('auth.twoFactor.confirming')}
                </>
              ) : (
                t('auth.oauthLink.confirmAction')
              )}
            </button>

            <div className="text-center">
              <Link
                to={returnTo ?? '/'}
                onClick={() => {
                  linkParams = null;
                  clearReturnPath();
                }}
                className="inline-flex min-h-11 items-center justify-center px-3 rounded-xl text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
              >
                {t('common.cancel')}
              </Link>
            </div>
          </form>
        </div>
      </AuthLayout>
    );
  }

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
            to={returnTo ?? '/'}
            onClick={() => {
              linkParams = null;
              clearReturnPath();
            }}
            className="inline-flex min-h-11 items-center justify-center px-3 text-sm text-black dark:text-indigo-400 font-medium hover:underline rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            {t('common.goBack')}
          </Link>
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
          <Skeleton className="h-3 w-36 mx-auto" />
        </div>
        <p className="text-sm text-gray-600 dark:text-slate-300">{t('auth.oauthLink.linking')}</p>
      </div>
    </AuthLayout>
  );
}
