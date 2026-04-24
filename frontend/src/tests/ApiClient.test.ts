import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getAccessToken, setAccessToken } from '../api/client';

vi.mock('axios', () => ({
  default: {
    create: vi.fn(() => ({
      interceptors: {
        request: { use: vi.fn() },
        response: { use: vi.fn() },
      },
    })),
    post: vi.fn(),
  },
}));

describe('API Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setAccessToken(null);
  });

  it('should store and provide access token', () => {
    setAccessToken('test-token');
    expect(getAccessToken()).toBe('test-token');
  });
});
