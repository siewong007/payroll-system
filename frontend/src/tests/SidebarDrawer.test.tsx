import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import type { User } from '@/types';
import { AuthContext, type AuthContextType } from '@/context/AuthContext';
import { Sidebar } from '@/components/layout/Sidebar';

vi.mock('@/api/admin', () => ({
  getMyCompanies: vi.fn().mockResolvedValue([]),
}));

const adminUser: User = {
  id: 'u1',
  email: 'admin@test.local',
  full_name: 'Admin User',
  roles: ['admin', 'payroll_admin'],
  company_id: 'c1',
  employee_id: null,
};

function makeAuth(user: User): AuthContextType {
  return {
    user,
    token: 'test-token',
    login: vi.fn(),
    completeTwoFactorLogin: vi.fn(),
    logout: vi.fn(),
    switchCompany: vi.fn(),
    setSession: vi.fn(),
    isAuthenticated: true,
    isLoading: false,
  };
}

function PathProbe() {
  const { pathname } = useLocation();
  return <div data-testid="path">{pathname}</div>;
}

function renderSidebar(onClose: () => void) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={makeAuth(adminUser)}>
        <MemoryRouter initialEntries={['/payroll']}>
          <Sidebar open onClose={onClose} />
          <Routes>
            <Route path="*" element={<PathProbe />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

describe('Sidebar mobile drawer', () => {
  it('navigates and closes the drawer when a nav link is clicked', async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderSidebar(onClose);

    // The desktop rail renders first (no onClose); the drawer copy is second.
    const employeesLinks = screen.getAllByRole('link', { name: 'Employees' });
    const drawerLink = employeesLinks[employeesLinks.length - 1];
    await user.click(drawerLink);

    expect(screen.getByTestId('path')).toHaveTextContent('/employees');
    expect(onClose).toHaveBeenCalled();
  });

  it('shows both nav sections for an admin', () => {
    renderSidebar(vi.fn());
    expect(screen.getAllByText('Workspace').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Administration').length).toBeGreaterThan(0);
    expect(screen.getAllByRole('link', { name: 'Audit Trail' }).length).toBeGreaterThan(0);
  });
});
