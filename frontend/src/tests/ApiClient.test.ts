import { beforeEach, describe, expect, it, vi } from 'vitest';

interface TestRequestConfig {
  headers: Record<string, string>;
  url?: string;
  _retry?: boolean;
}

interface TestResponseError {
  config: TestRequestConfig;
  response?: { status: number };
}

type RequestHandler = (config: TestRequestConfig) => TestRequestConfig;
type ResponseHandler = (response: unknown) => unknown;
type ResponseErrorHandler = (error: TestResponseError) => Promise<unknown>;

const axiosMocks = vi.hoisted(() => {
  const requestUse = vi.fn();
  const responseUse = vi.fn();
  const apiInstance = vi.fn();

  Object.assign(apiInstance, {
    interceptors: {
      request: { use: requestUse },
      response: { use: responseUse },
    },
  });

  return {
    apiInstance,
    create: vi.fn(() => apiInstance),
    post: vi.fn(),
    requestUse,
    responseUse,
  };
});

vi.mock('axios', () => ({
  default: {
    create: axiosMocks.create,
    post: axiosMocks.post,
  },
}));

import { getAccessToken, setAccessToken } from '../api/client';

const requestHandler = axiosMocks.requestUse.mock.calls[0][0] as RequestHandler;
const responseHandler = axiosMocks.responseUse.mock.calls[0][0] as ResponseHandler;
const responseErrorHandler = axiosMocks.responseUse.mock.calls[0][1] as ResponseErrorHandler;

const refreshedUser = {
  id: 'user-1',
  email: 'payroll@example.com',
  full_name: 'Payroll Admin',
  roles: ['payroll_admin'],
  company_id: 'company-1',
  employee_id: null,
};

function unauthorized(url: string): TestResponseError {
  return {
    config: { url, headers: {} },
    response: { status: 401 },
  };
}

describe('API client', () => {
  beforeEach(() => {
    axiosMocks.post.mockReset();
    axiosMocks.apiInstance.mockReset();
    setAccessToken(null);
    localStorage.clear();
  });

  it('creates one credentialed JSON client at the API root', () => {
    expect(axiosMocks.create).toHaveBeenCalledWith({
      baseURL: '/api',
      headers: { 'Content-Type': 'application/json' },
      withCredentials: true,
    });
  });

  it('stores the access token in memory only', () => {
    setAccessToken('test-token');

    expect(getAccessToken()).toBe('test-token');
    expect(localStorage.getItem('access_token')).toBeNull();

    setAccessToken(null);
    expect(getAccessToken()).toBeNull();
  });

  it('adds a bearer token while preserving an explicit authorization scheme', () => {
    setAccessToken('access-token');

    expect(requestHandler({ headers: {} }).headers.Authorization).toBe('Bearer access-token');
    expect(requestHandler({ headers: { Authorization: 'Kiosk kiosk-secret' } }).headers.Authorization)
      .toBe('Kiosk kiosk-secret');

    setAccessToken(null);
    expect(requestHandler({ headers: {} }).headers.Authorization).toBeUndefined();
  });

  it('passes successful responses through unchanged', () => {
    const response = { status: 200, data: { ok: true } };
    expect(responseHandler(response)).toBe(response);
  });

  it('refreshes once, updates session state, and retries the failed request', async () => {
    const retryResult = { status: 200, data: { employees: [] } };
    const error = unauthorized('/employees');
    axiosMocks.post.mockResolvedValueOnce({
      data: { token: 'fresh-token', user: refreshedUser },
    });
    axiosMocks.apiInstance.mockResolvedValueOnce(retryResult);

    await expect(responseErrorHandler(error)).resolves.toBe(retryResult);

    expect(axiosMocks.post).toHaveBeenCalledOnce();
    expect(axiosMocks.post).toHaveBeenCalledWith(
      '/api/auth/refresh',
      {},
      { withCredentials: true },
    );
    expect(error.config._retry).toBe(true);
    expect(error.config.headers.Authorization).toBe('Bearer fresh-token');
    expect(axiosMocks.apiInstance).toHaveBeenCalledWith(error.config);
    expect(getAccessToken()).toBe('fresh-token');
    expect(JSON.parse(localStorage.getItem('user') ?? 'null')).toEqual(refreshedUser);
  });

  it('queues concurrent 401 responses behind a single refresh request', async () => {
    let resolveRefresh: ((value: { data: { token: string; user: typeof refreshedUser } }) => void) | undefined;
    axiosMocks.post.mockImplementationOnce(() => new Promise((resolve) => {
      resolveRefresh = resolve;
    }));
    axiosMocks.apiInstance.mockResolvedValue({ status: 200 });

    const firstError = unauthorized('/employees');
    const secondError = unauthorized('/payroll');
    const firstRetry = responseErrorHandler(firstError);
    const secondRetry = responseErrorHandler(secondError);

    expect(axiosMocks.post).toHaveBeenCalledOnce();
    expect(resolveRefresh).toBeDefined();
    resolveRefresh?.({ data: { token: 'shared-token', user: refreshedUser } });

    await expect(Promise.all([firstRetry, secondRetry])).resolves.toEqual([
      { status: 200 },
      { status: 200 },
    ]);
    expect(axiosMocks.apiInstance).toHaveBeenCalledTimes(2);
    expect(firstError.config.headers.Authorization).toBe('Bearer shared-token');
    expect(secondError.config.headers.Authorization).toBe('Bearer shared-token');
  });

  it('surfaces a revoked kiosk credential without refreshing or clearing the user session', async () => {
    setAccessToken('existing-token');
    localStorage.setItem('user', JSON.stringify(refreshedUser));
    const error = unauthorized('/attendance/kiosk/qr');

    await expect(responseErrorHandler(error)).rejects.toBe(error);

    expect(axiosMocks.post).not.toHaveBeenCalled();
    expect(axiosMocks.apiInstance).not.toHaveBeenCalled();
    expect(getAccessToken()).toBe('existing-token');
    expect(JSON.parse(localStorage.getItem('user') ?? 'null')).toEqual(refreshedUser);
  });
});
