import { StrictMode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AuthContext, type AuthContextType } from '@/context/AuthContext';
import { userWithRoles } from './support/permissions';

const mockNavigate = vi.hoisted(() => vi.fn());

vi.mock('react-router', async () => {
  const actual = await vi.importActual<typeof import('react-router')>('react-router');
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

vi.mock('@/api/attendance', () => ({
  getAttendanceMethod: vi.fn(),
  checkInQr: vi.fn(),
}));

vi.mock('@/api/notifications', () => ({
  getNotifications: vi.fn(),
  getNotificationCount: vi.fn(),
  markAsRead: vi.fn(),
  markAllRead: vi.fn(),
}));

vi.mock('@/components/pwa/NotificationActivationCard', () => ({
  NotificationActivationCard: () => null,
}));

import { checkInQr, getAttendanceMethod } from '@/api/attendance';
import { getNotificationCount, getNotifications, markAsRead } from '@/api/notifications';
import { AttendanceScanPage } from '@/pages/attendance/AttendanceScanPage';
import { Notifications } from '@/pages/portal/Notifications';

function authContextValue(overrides: Partial<AuthContextType['user']> = {}): AuthContextType {
  const user = userWithRoles(['employee'], {
    id: 'user-1',
    company_id: 'company-1',
    employee_id: 'emp-1',
    ...overrides,
  });
  return {
    user,
    token: 'token-1',
    isAuthenticated: true,
    isLoading: false,
    login: vi.fn(),
    completeTwoFactorLogin: vi.fn(),
    logout: vi.fn(),
    setSession: vi.fn(),
    switchCompany: vi.fn(),
  };
}

describe('AttendanceScanPage under StrictMode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getAttendanceMethod).mockResolvedValue({
      method: 'qr_code',
      geofence_mode: 'none',
    } as Awaited<ReturnType<typeof getAttendanceMethod>>);
    vi.mocked(checkInQr).mockResolvedValue({
      id: 'att-1',
      company_id: 'company-1',
      employee_id: 'emp-1',
      check_in_at: '2026-07-01T01:00:00Z',
      check_out_at: null,
      method: 'qr_code',
      status: 'present',
      latitude: null,
      longitude: null,
      checkout_latitude: null,
      checkout_longitude: null,
      geofence_status: null,
      distance_meters: null,
      network_status: null,
      network_name: null,
      notes: null,
    } as unknown as Awaited<ReturnType<typeof checkInQr>>);
  });

  it('completes QR check-in without getting stuck by in-flight StrictMode cleanup', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <StrictMode>
        <QueryClientProvider client={qc}>
          <AuthContext.Provider value={authContextValue()}>
            <MemoryRouter initialEntries={['/attendance/scan?token=qr-strict-123']}>
              <AttendanceScanPage />
            </MemoryRouter>
          </AuthContext.Provider>
        </QueryClientProvider>
      </StrictMode>,
    );

    await waitFor(() => {
      expect(checkInQr).toHaveBeenCalledWith('qr-strict-123', undefined, undefined);
    });
  });
});

describe('Notifications filter & permission-aware routing', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(markAsRead).mockResolvedValue({ ok: true });
    vi.mocked(getNotificationCount).mockResolvedValue({ unread: 2, total: 2 });
    vi.mocked(getNotifications).mockResolvedValue([
      {
        id: 'n-submit',
        user_id: 'user-1',
        company_id: 'company-1',
        notification_type: 'leave_submitted',
        title: 'Leave request submitted',
        message: 'Aisyah submitted annual leave',
        entity_type: 'leave_request',
        entity_id: 'lr-1',
        is_read: false,
        read_at: null,
        created_at: '2026-07-01T02:00:00Z',
      },
      {
        id: 'n-approved',
        user_id: 'user-1',
        company_id: 'company-1',
        notification_type: 'leave_approved',
        title: 'Your leave was approved',
        message: 'Approved by manager',
        entity_type: 'leave_request',
        entity_id: 'lr-2',
        is_read: false,
        read_at: null,
        created_at: '2026-07-01T03:00:00Z',
      },
    ]);
  });

  it('passes unreadOnly flag on tab switch and gates /approvals vs /portal routes by permissions and employee_id', async () => {
    const user = userEvent.setup();
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AuthContext.Provider
          value={authContextValue({
            roles: ['hr_manager'],
            permissions: ['view_approvals'],
            employee_id: null,
          })}
        >
          <MemoryRouter initialEntries={['/notifications']}>
            <Notifications />
          </MemoryRouter>
        </AuthContext.Provider>
      </QueryClientProvider>,
    );

    const submittedBtn = await screen.findByText('Leave request submitted');
    expect(getNotifications).toHaveBeenCalledWith(false, 100);

    await user.click(submittedBtn);
    expect(mockNavigate).toHaveBeenCalledWith('/approvals?tab=leave');

    mockNavigate.mockClear();
    await user.click(screen.getByText('Your leave was approved'));
    expect(mockNavigate).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Unread' }));
    await waitFor(() => {
      expect(getNotifications).toHaveBeenCalledWith(true, 100);
    });
  });

  it('routes non-approver employees with leave_submitted away from /approvals', async () => {
    const user = userEvent.setup();
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AuthContext.Provider
          value={authContextValue({
            roles: ['employee'],
            permissions: [],
            employee_id: 'emp-1',
          })}
        >
          <MemoryRouter initialEntries={['/notifications']}>
            <Notifications />
          </MemoryRouter>
        </AuthContext.Provider>
      </QueryClientProvider>,
    );

    await user.click(await screen.findByText('Leave request submitted'));
    expect(mockNavigate).toHaveBeenCalledWith('/portal/leave');
  });
});
