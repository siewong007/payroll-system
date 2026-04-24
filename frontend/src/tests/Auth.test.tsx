import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AuthProvider } from '../context/AuthProvider';
import { useAuth } from '../context/AuthContext';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/api/client', () => ({
  default: {
    post: vi.fn().mockRejectedValue(new Error('No session')),
  },
  setAccessToken: vi.fn(),
}));

// Helper component to test useAuth
const TestComponent = () => {
  const { isAuthenticated, isLoading } = useAuth();
  return (
    <div>
      <span data-testid="auth-status">{isAuthenticated ? 'authenticated' : 'not-authenticated'}</span>
      <span data-testid="loading-status">{isLoading ? 'loading' : 'ready'}</span>
    </div>
  );
};

describe('AuthContext', () => {
  it('should provide initial auth state', () => {
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <TestComponent />
        </AuthProvider>
      </QueryClientProvider>
    );

    expect(screen.getByTestId('auth-status')).toHaveTextContent('not-authenticated');
  });
});
