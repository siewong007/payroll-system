import api from './client';

export interface OAuth2Provider {
  provider: string;
  enabled: boolean;
}

export interface LinkedAccount {
  id: string;
  provider: string;
  provider_email: string | null;
  provider_name: string | null;
  avatar_url: string | null;
  linked_at: string;
}

export async function getOAuth2Providers(): Promise<OAuth2Provider[]> {
  const { data } = await api.get('/auth/oauth2/providers');
  return Array.isArray(data) ? data : [];
}

/**
 * `flow: 'link'` sends the browser to the SPA's /oauth2/link route instead of
 * the login callback, so the returned code can be POSTed with the user's JWT.
 */
export async function getGoogleAuthorizeUrl(
  flow?: 'link',
  turnstileToken?: string,
): Promise<string> {
  const { data } = await api.get<{ authorize_url: string }>('/auth/oauth2/google/authorize', {
    params: {
      ...(flow ? { flow } : {}),
      ...(turnstileToken ? { turnstile_token: turnstileToken } : {}),
    },
  });
  return data.authorize_url;
}

export async function getLinkedAccounts(): Promise<LinkedAccount[]> {
  const { data } = await api.get('/auth/oauth2/accounts');
  return data;
}

export async function linkGoogleAccount(code: string, state: string): Promise<LinkedAccount> {
  const { data } = await api.post('/auth/oauth2/google/link', { code, state });
  return data;
}

export async function unlinkOAuth2Provider(provider: string): Promise<void> {
  await api.delete(`/auth/oauth2/accounts/${provider}`);
}
