import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '@/i18n';

const portalMocks = vi.hoisted(() => ({
  getTeamCalendar: vi.fn(),
  getPortalHolidays: vi.fn(),
}));

vi.mock('@/api/portal', () => portalMocks);

import { TeamCalendar } from '@/pages/portal/TeamCalendar';

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
});

describe('TeamCalendar loading, error, and success states', () => {
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
});
