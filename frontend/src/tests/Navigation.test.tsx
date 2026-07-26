import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppRole, User } from '@/types';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const adminMocks = vi.hoisted(() => ({ getMyCompanies: vi.fn() }));
const navigateMock = vi.hoisted(() => vi.fn());

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/admin', () => ({ getMyCompanies: adminMocks.getMyCompanies }));
vi.mock('react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-router')>()),
  useNavigate: () => navigateMock,
}));

import { Sidebar } from '@/components/layout/Sidebar';
import { CompanySwitcher } from '@/components/layout/CompanySwitcher';

function asUser(roles: AppRole[], overrides: Partial<User> = {}): User {
  return {
    id: 'user-1',
    email: 'person@example.com',
    full_name: 'Aisyah Rahman',
    roles,
    company_id: 'company-1',
    employee_id: null,
    ...overrides,
  };
}

function renderWithProviders(ui: ReactNode, route = '/company') {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[route]}>{ui}</MemoryRouter>
    </QueryClientProvider>,
  );
}

function navLinks() {
  return within(screen.getByRole('navigation'))
    .getAllByRole('link')
    .map((link) => link.textContent?.trim());
}

beforeEach(() => {
  authMocks.useAuth.mockReset();
  adminMocks.getMyCompanies.mockReset().mockResolvedValue([]);
  navigateMock.mockReset();
  authMocks.useAuth.mockReturnValue({ user: asUser(['admin']), logout: vi.fn(), switchCompany: vi.fn() });
});

describe('Sidebar role-based navigation', () => {
  it('shows the workspace rail to a company admin and hides super-admin-only entries', () => {
    renderWithProviders(<Sidebar />);

    const links = navLinks();
    expect(links).toEqual(
      expect.arrayContaining(['Company', 'Employees', 'Teams', 'Calendar', 'Attendance', 'Approvals', 'Reports']),
    );
    expect(links).not.toContain('Companies');
    expect(links).not.toContain('Users');
    expect(links).not.toContain('Roles');
  });

  it('gives super_admin only the administration rail, not the tenant workspace', () => {
    authMocks.useAuth.mockReturnValue({ user: asUser(['super_admin']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    const links = navLinks();
    expect(links).toEqual(
      expect.arrayContaining(['Companies', 'Users', 'Roles', 'Attendance Settings', 'Audit Trail', 'Backup']),
    );
    // Every tenant-scoped entry is hidden; the platform-wide Payroll and
    // Reports entries stay because super_admin is on their allow-lists.
    for (const hidden of ['Company', 'Employees', 'Teams', 'Calendar', 'Attendance', 'Approvals', 'Documents', 'Letters', 'Settings']) {
      expect(links).not.toContain(hidden);
    }
    expect(links).toEqual(expect.arrayContaining(['Payroll', 'Reports']));
  });

  it('shows Payroll only to roles that may see payroll figures', () => {
    for (const role of ['payroll_admin', 'finance'] as AppRole[]) {
      authMocks.useAuth.mockReturnValue({ user: asUser([role]), logout: vi.fn() });
      const { unmount } = renderWithProviders(<Sidebar />);
      expect(navLinks()).toContain('Payroll');
      unmount();
    }

    for (const role of ['admin', 'hr_manager', 'exec'] as AppRole[]) {
      authMocks.useAuth.mockReturnValue({ user: asUser([role]), logout: vi.fn() });
      const { unmount } = renderWithProviders(<Sidebar />);
      expect(navLinks()).not.toContain('Payroll');
      unmount();
    }
  });

  it('hides Reports from exec, matching the /reports RoleGuard', () => {
    authMocks.useAuth.mockReturnValue({ user: asUser(['exec']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(navLinks()).not.toContain('Reports');
    // exec may still read attendance, so that link stays.
    expect(navLinks()).toContain('Attendance');
  });

  it('hides Audit Trail and Backup from hr_manager', () => {
    // These links previously showed for hr_manager and 403'd on click; the nav
    // must match the backend's super_admin + admin gate.
    authMocks.useAuth.mockReturnValue({ user: asUser(['hr_manager']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(navLinks()).not.toContain('Audit Trail');
    expect(navLinks()).not.toContain('Backup');
  });

  it('shows Audit Trail and Backup to an admin', () => {
    renderWithProviders(<Sidebar />);

    expect(navLinks()).toEqual(expect.arrayContaining(['Audit Trail', 'Backup']));
  });

  it('drops a section heading entirely when the role sees none of its items', () => {
    authMocks.useAuth.mockReturnValue({ user: asUser(['hr_manager']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(screen.getByText('Workspace')).toBeInTheDocument();
    expect(screen.queryByText('Administration')).not.toBeInTheDocument();
  });
});

describe('Sidebar user panel', () => {
  it('renders the display name, humanised roles, and an initial', () => {
    authMocks.useAuth.mockReturnValue({ user: asUser(['payroll_admin', 'finance']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(screen.getByText('Aisyah Rahman')).toBeInTheDocument();
    expect(screen.getByText('payroll admin, finance')).toBeInTheDocument();
    expect(screen.getByText('A')).toBeInTheDocument();
  });

  it('signs out through the auth context', async () => {
    const user = userEvent.setup();
    const logout = vi.fn();
    authMocks.useAuth.mockReturnValue({ user: asUser(['admin']), logout });
    renderWithProviders(<Sidebar />);

    await user.click(screen.getByTitle('Sign Out'));

    expect(logout).toHaveBeenCalledOnce();
  });

  it('falls back to placeholder identity when the session is still loading', () => {
    authMocks.useAuth.mockReturnValue({ user: null, logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(screen.getByText('User')).toBeInTheDocument();
    expect(screen.getByText('U')).toBeInTheDocument();
  });
});

describe('CompanySwitcher', () => {
  const companies = [
    { id: 'company-1', name: 'Acme Sdn Bhd' },
    { id: 'company-2', name: 'Beta Holdings' },
  ];

  it('stays hidden when the user belongs to a single company', async () => {
    adminMocks.getMyCompanies.mockResolvedValue([companies[0]]);
    const { container } = renderWithProviders(<CompanySwitcher />);

    await waitFor(() => expect(adminMocks.getMyCompanies).toHaveBeenCalled());
    // A switcher with one option is a no-op control.
    expect(container).toBeEmptyDOMElement();
  });

  it('stays hidden when the user belongs to no company', async () => {
    adminMocks.getMyCompanies.mockResolvedValue([]);
    const { container } = renderWithProviders(<CompanySwitcher />);

    await waitFor(() => expect(adminMocks.getMyCompanies).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the active company once more than one is available', async () => {
    adminMocks.getMyCompanies.mockResolvedValue(companies);
    renderWithProviders(<CompanySwitcher />);

    expect(await screen.findByText('Acme Sdn Bhd')).toBeInTheDocument();
  });

  it('switches company and returns to the home route', async () => {
    const user = userEvent.setup();
    const switchCompany = vi.fn().mockResolvedValue(undefined);
    authMocks.useAuth.mockReturnValue({ user: asUser(['admin']), switchCompany, logout: vi.fn() });
    adminMocks.getMyCompanies.mockResolvedValue(companies);
    renderWithProviders(<CompanySwitcher />);

    await user.click(await screen.findByRole('button'));
    await user.click(await screen.findByRole('button', { name: /Beta Holdings/ }));

    await waitFor(() => expect(switchCompany).toHaveBeenCalledWith('company-2'));
    // A stale company-scoped route would 404 or leak the previous tenant's view.
    expect(navigateMock).toHaveBeenCalledWith('/', { replace: true });
  });

  it('does not re-issue a token when the active company is reselected', async () => {
    const user = userEvent.setup();
    const switchCompany = vi.fn();
    authMocks.useAuth.mockReturnValue({ user: asUser(['admin']), switchCompany, logout: vi.fn() });
    adminMocks.getMyCompanies.mockResolvedValue(companies);
    renderWithProviders(<CompanySwitcher />);

    // The trigger shows the active company name too, so pick the option out of
    // the opened dropdown rather than by name alone.
    const trigger = await screen.findByRole('button');
    await user.click(trigger);
    const options = screen.getAllByRole('button').filter((button) => button !== trigger);
    await user.click(options[0]);

    expect(switchCompany).not.toHaveBeenCalled();
    expect(navigateMock).not.toHaveBeenCalled();
  });
});
