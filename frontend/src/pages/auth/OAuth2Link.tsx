import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
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

function takeLinkParams(): URLSearchParams {
  const fromQuery = new URLSearchParams(window.location.search);

  if (Array.from(fromQuery.keys()).length > 0) {
    linkParams = fromQuery;
    window.history.replaceState(null, '', window.location.pathname);
  }

  return linkParams ?? fromQuery;
}

export function OAuth2Link() {
  const navigate = useNavigate();
  const { isAuthenticated, isLoading } = useAuth();
  const [error, setError] = useState('');

  // Where the LinkedAccounts card sat before the round-trip to Google.
  const returnTo = safeRedirectPath(sessionStorage.getItem(RETURN_PATH_KEY));

  useEffect(() => {
    // The page reload wiped the in-memory token; the POST needs the session
    // the refresh restores, so nothing may fire while it is still in flight.
    if (isLoading) {
      return;
    }

    const params = takeLinkParams();

    const providerError = params.get('error');
    if (providerError) {
      setError(
        providerError === 'access_denied'
          ? 'Google sign-in was cancelled. The account was not linked.'
          : 'Google could not complete the sign-in. The account was not linked.',
      );
      return;
    }

    if (!isAuthenticated) {
      setError('Your session could not be restored. Please sign in and try linking again.');
      return;
    }

    const code = params.get('code');
    const state = params.get('state');
    if (!code || !state) {
      setError('Google account linking failed. Missing authentication data.');
      return;
    }

    linkGoogleAccount(code, state)
      .then(() => {
        sessionStorage.removeItem(RETURN_PATH_KEY);
        navigate(returnTo ?? '/', { replace: true });
      })
      .catch((err: unknown) => {
        setError(getErrorMessage(err, 'Could not link the Google account.'));
      });
  }, [isLoading, isAuthenticated, navigate, returnTo]);

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-100">
        <div className="bg-white rounded-2xl shadow p-8 max-w-md w-full text-center space-y-4">
          <div className="w-16 h-16 bg-red-50 rounded-full flex items-center justify-center mx-auto">
            <svg className="w-8 h-8 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>
          <p className="text-sm text-gray-600">{error}</p>
          <a href={returnTo ?? '/'} className="inline-block text-sm text-black font-medium hover:underline">
            Go back
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100">
      <div className="text-center space-y-4">
        <BrandLogo variant="lockup-dark" className="h-12 w-auto mx-auto" />
        <div className="text-gray-500">Linking your Google account...</div>
      </div>
    </div>
  );
}
