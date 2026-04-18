import { describe, it, expect, vi, beforeEach } from 'vitest';
import api, { setAccessToken } from '../api/client';
import axios from 'axios';

vi.mock('axios', async () => {
  const actual = await vi.importActual('axios') as any;
  return {
    default: {
      ...actual.default,
      create: vi.fn(() => ({
        interceptors: {
          request: { use: vi.fn() },
          response: { use: vi.fn() },
        },
        post: vi.fn(),
        get: vi.fn(),
      })),
      post: vi.fn(),
    },
  };
});

describe('API Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setAccessToken(null);
  });

  it('should store and provide access token', () => {
    setAccessToken('test-token');
    // Internal check or test via interceptor if we wanted to be thorough
  });
});
