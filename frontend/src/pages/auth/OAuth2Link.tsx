import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from '@/context/AuthContext';
import { linkGoogleAccount } from '@/api/oauth2';
import { getErrorMessage, safeRedirectPath } from '@/lib/utils';
import { BrandLogo } from '@/components/ui/BrandLogo';

const RETURN_PATH_KEY = 'oauth2_link_return';

/**
 * Google lands here with ?code&state in the query string (the authorization
 * response is a redirect, not our fragment convention). Same single-read rule
 * as OAuth2Callback: the params are stripped from the URL on first read, so a
 * StrictMode remount must see the memo, not an empty search string.
 */
let linkParams: URLSearchParams | null = null;
let inflightLinkKey: string | null = null;
let inflightLinkPromise: Promise<unknown> | null = null;

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

  // Where the LinkedAccounts card sat before the round-trip to Google.
  const returnTo = readReturnPath();

  useEffect(() => {
    // The page reload wiped the in-memory token; the POST needs the session
    // the refresh restores, so nothing may fire while it is still in flight.
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

    let cancelled = false;
    const key = `${code}:${state}`;
    if (!inflightLinkPromise || inflightLinkKey !== key) {
      inflightLinkKey = key;
      inflightLinkPromise = linkGoogleAccount(code, state);
    }

    inflightLinkPromise
      .then(() => {
        if (inflightLinkKey === key) {
          inflightLinkPromise = null;
          inflightLinkKey = null;
        }
        linkParams = null;
        clearReturnPath();
        if (!cancelled) {
          navigate(user?.must_change_password ? '/change-password' : (returnTo ?? '/'), { replace: true });
        }
      })
      .catch((err: unknown) => {
        if (inflightLinkKey === key) {
          inflightLinkPromise = null;
          inflightLinkKey = null;
        }
        linkParams = null;
        if (!cancelled) {
          setError(getErrorMessage(err, t('auth.oauthLink.linkFailed')));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isLoading, isAuthenticated, navigate, returnTo, t, user?.must_change_password]);

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
          <Link to={returnTo ?? '/'} className="inline-block text-sm text-black dark:text-indigo-400 font-medium hover:underline">
            {t('common.goBack')}
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 px-4">
      <div className="text-center space-y-4">
        <BrandLogo variant="lockup-dark" className="h-12 w-auto mx-auto rounded-xl" />
        <div className="text-gray-500 dark:text-slate-400">{t('auth.oauthLink.linking')}</div>
      </div>
    </div>
  );
}
