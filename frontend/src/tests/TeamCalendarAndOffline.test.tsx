import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '@/i18n';
import { userWithRoles } from './support/permissions';

const portalMocks = vi.hoisted(() => ({
  getTeamCalendar: vi.fn(),
  getPortalHolidays: vi.fn(),
}));
const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const adminMocks = vi.hoisted(() => ({ getMyCompanies: vi.fn() }));
const notificationMocks = vi.hoisted(() => ({ getNotificationCount: vi.fn() }));

vi.mock('@/api/portal', () => portalMocks);
vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/admin', () => ({ getMyCompanies: adminMocks.getMyCompanies }));
vi.mock('@/api/notifications', () => ({
  getNotificationCount: notificationMocks.getNotificationCount,
}));

import { TeamCalendar } from '@/pages/portal/TeamCalendar';
import { OfflineBanner } from '@/components/ui/OfflineBanner';
import { AppLayout } from '@/components/layout/AppLayout';
import { PortalLayout } from '@/components/layout/PortalLayout';

function renderTeamCalendar() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <TeamCalendar />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(window.navigator, 'onLine', {
    value: true,
    configurable: true,
  });
  adminMocks.getMyCompanies.mockResolvedValue([]);
  notificationMocks.getNotificationCount.mockResolvedValue({ unread: 0 });
  authMocks.useAuth.mockReturnValue({
    user: userWithRoles(['admin'], { full_name: 'Admin User', must_change_password: false }),
    isAuthenticated: true,
    isLoading: false,
    logout: vi.fn(),
    switchCompany: vi.fn(),
  });
});

describe('TeamCalendar loading, error, and success states', () => {
  beforeEach(() => {
    authMocks.useAuth.mockReturnValue({
      user: userWithRoles(['admin'], {
        full_name: 'Admin User',
        employee_id: 'emp-1',
        must_change_password: false,
      }),
      isAuthenticated: true,
      isLoading: false,
      logout: vi.fn(),
      switchCompany: vi.fn(),
    });
  });

  it('renders a loading placeholder while queries are pending', () => {
    portalMocks.getTeamCalendar.mockImplementation(() => new Promise(() => {}));
    portalMocks.getPortalHolidays.mockImplementation(() => new Promise(() => {}));

    renderTeamCalendar();

    expect(screen.getByLabelText(i18n.t('a11y.loading'))).toHaveAttribute('aria-busy', 'true');
  });

  it('renders error fallback and retries both queries on retry click', async () => {
    const user = userEvent.setup();
    portalMocks.getTeamCalendar.mockRejectedValueOnce(new Error('Network down'));
    portalMocks.getPortalHolidays.mockResolvedValue([]);

    renderTeamCalendar();

    expect(await screen.findByText(i18n.t('portal.teamCalendar.loadFailed'))).toBeInTheDocument();

    portalMocks.getTeamCalendar.mockResolvedValueOnce([]);
    await user.click(screen.getByRole('button', { name: i18n.t('common.retry') }));

    await waitFor(() => {
      expect(portalMocks.getTeamCalendar).toHaveBeenCalledTimes(2);
    });
  });

  it('renders team leave entries once loaded', async () => {
    const now = new Date();
    const year = now.getFullYear();
    const month = String(now.getMonth() + 1).padStart(2, '0');
    portalMocks.getTeamCalendar.mockResolvedValue([
      {
        id: 'leave-1',
        employee_name: 'Siti Aminah',
        leave_type_name: 'Annual Leave',
        start_date: `${year}-${month}-10`,
        end_date: `${year}-${month}-10`,
        days: '1',
      },
    ]);
    portalMocks.getPortalHolidays.mockResolvedValue([]);

    renderTeamCalendar();

    expect(await screen.findByText('Siti Aminah')).toBeInTheDocument();
  });

  it('shows noEmployeeProfile without querying team calendar when employee_id is null', async () => {
    authMocks.useAuth.mockReturnValue({
      user: userWithRoles(['admin'], {
        full_name: 'Admin User',
        employee_id: null,
        must_change_password: false,
      }),
      isAuthenticated: true,
      isLoading: false,
      logout: vi.fn(),
      switchCompany: vi.fn(),
    });
    portalMocks.getPortalHolidays.mockResolvedValue([]);

    renderTeamCalendar();

    expect(
      await screen.findByText(i18n.t('attendance.scan.noEmployeeProfile')),
    ).toBeInTheDocument();
    expect(portalMocks.getTeamCalendar).not.toHaveBeenCalled();
  });
});

describe('OfflineBanner and layout accessibility', () => {
  it('shows offline banner when browser transitions offline and hides on online', () => {
    render(<OfflineBanner />);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();

    act(() => {
      window.dispatchEvent(new Event('offline'));
    });
    expect(screen.getByRole('status')).toHaveTextContent(i18n.t('common.offlineBanner'));

    act(() => {
      window.dispatchEvent(new Event('online'));
    });
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('renders skip-to-content link and >=44px mobile header buttons in AppLayout', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter initialEntries={['/dashboard']}>
          <Routes>
            <Route element={<AppLayout />}>
              <Route path="/dashboard" element={<div>Dashboard Content</div>} />
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    const skipLink = screen.getByRole('link', { name: i18n.t('a11y.skipToContent') });
    expect(skipLink).toHaveAttribute('href', '#main-content');
    expect(screen.getByRole('main')).toHaveAttribute('id', 'main-content');
    expect(screen.getByRole('button', { name: i18n.t('a11y.openMenu') })).toHaveClass(
      'min-h-11',
      'min-w-11',
    );
  });

  it('renders skip-to-content link and >=44px header controls in PortalLayout', () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter initialEntries={['/portal/profile']}>
          <Routes>
            <Route element={<PortalLayout />}>
              <Route path="/portal/profile" element={<div>Portal Content</div>} />
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    const skipLink = screen.getByRole('link', { name: i18n.t('a11y.skipToContent') });
    expect(skipLink).toHaveAttribute('href', '#main-content');
    expect(screen.getByRole('main')).toHaveAttribute('id', 'main-content');
    expect(screen.getByRole('button', { name: i18n.t('nav.quickSearch') })).toHaveClass(
      'min-h-11',
      'min-w-11',
    );
  });
});
