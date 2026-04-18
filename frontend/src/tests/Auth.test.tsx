import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AuthProvider } from '../context/AuthProvider';
import { useAuth } from '../context/AuthContext';
import React from 'react';

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
    render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    expect(screen.getByTestId('auth-status')).toHaveTextContent('not-authenticated');
  });
});
