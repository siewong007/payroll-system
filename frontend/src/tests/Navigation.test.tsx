import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppRole, User } from '@/types';
import { userWithRoles } from './support/permissions';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const adminMocks = vi.hoisted(() => ({ getMyCompanies: vi.fn() }));
const notificationMocks = vi.hoisted(() => ({ getNotificationCount: vi.fn() }));
const navigateMock = vi.hoisted(() => vi.fn());

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/admin', () => ({ getMyCompanies: adminMocks.getMyCompanies }));
vi.mock('@/api/notifications', () => ({ getNotificationCount: notificationMocks.getNotificationCount }));
vi.mock('react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-router')>()),
  useNavigate: () => navigateMock,
}));

import { Sidebar } from '@/components/layout/Sidebar';
import { CompanySwitcher } from '@/components/layout/CompanySwitcher';
import { PortalLayout } from '@/components/layout/PortalLayout';

// The sidebar filters links by permission now, so a fixture user must carry the
// permissions its roles imply — see `support/permissions`.
function asUser(roles: AppRole[], overrides: Partial<User> = {}): User {
  return userWithRoles(roles, { full_name: 'Aisyah Rahman', ...overrides });
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
  notificationMocks.getNotificationCount.mockReset().mockResolvedValue({ unread: 0 });
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

  it('hides Reports from exec, matching the /reports PermissionGuard', () => {
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

  it('shows Audit Trail to an admin but not Backup', () => {
    renderWithProviders(<Sidebar />);

    expect(navLinks()).toContain('Audit Trail');
    // Backup is super_admin-only: the archive carries payroll_items,
    // salary_history and raw employee rows, and `admin` is excluded from
    // payroll entirely. The link used to be offered here and 403'd on click.
    expect(navLinks()).not.toContain('Backup');
  });

  it('shows Backup to a super_admin', () => {
    authMocks.useAuth.mockReturnValue({ user: asUser(['super_admin']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(navLinks()).toContain('Backup');
  });

  it('offers My Attendance to staff held in the admin shell by a second role', () => {
    // `AppLayout` redirects only *sole-role* employees to the portal, so a
    // supervisor holding ['employee', 'hr_manager'] never sees the portal home's
    // check-in card. Without this link their own attendance is reachable only by
    // typing the URL.
    authMocks.useAuth.mockReturnValue({
      user: asUser(['employee', 'hr_manager'], { employee_id: 'employee-1' }),
      logout: vi.fn(),
    });
    renderWithProviders(<Sidebar />);

    expect(navLinks()).toContain('My Attendance');
    // Not /portal/attendance: that leaves the admin shell, which has no way back.
    expect(screen.getByRole('link', { name: 'My Attendance' })).toHaveAttribute('href', '/my/attendance');
  });

  it('hides My Attendance from a login with no employee record', () => {
    // Gated on the employee link, not on a role name — the page could only tell
    // an unlinked account to contact HR.
    authMocks.useAuth.mockReturnValue({ user: asUser(['admin']), logout: vi.fn() });
    renderWithProviders(<Sidebar />);

    expect(navLinks()).not.toContain('My Attendance');
    expect(screen.queryByText('Me')).not.toBeInTheDocument();
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

describe('Portal header', () => {
  function renderPortal(roles: AppRole[]) {
    authMocks.useAuth.mockReturnValue({
      user: asUser(roles),
      logout: vi.fn(),
      isAuthenticated: true,
      isLoading: false,
    });
    return renderWithProviders(<PortalLayout />, '/portal/profile');
  }

  function openUserMenu() {
    return userEvent.click(screen.getByRole('button', { name: /Aisyah Rahman/ }));
  }

  it('offers a way back to the console to staff holding a second role', async () => {
    // AppLayout holds everyone who is not a sole-role employee, so a supervisor
    // standing in the portal is here by choice — and until now their only route
    // out was the browser's back button or a hand-typed URL.
    renderPortal(['employee', 'hr_manager']);
    await openUserMenu();

    // `/` rather than a fixed page: HomeRedirect resolves the landing screen
    // their roles allow, which is /companies for a super_admin and /company
    // otherwise. Hardcoding either would 403 half the people who see this link.
    expect(await screen.findByRole('link', { name: /admin console/i })).toHaveAttribute('href', '/');
  });

  it('shows no console link to a sole-role employee', async () => {
    renderPortal(['employee']);
    await openUserMenu();

    // Sign Out first: it proves the menu actually opened, so the absence below
    // is an assertion about the link rather than about an unopened dropdown.
    expect(await screen.findByRole('button', { name: /sign out/i })).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: /admin console/i })).not.toBeInTheDocument();
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
