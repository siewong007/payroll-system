import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CompanySetting } from '@/types';

const settingsMocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  bulkUpdateSettings: vi.fn(),
}));

vi.mock('@/api/settings', () => settingsMocks);
vi.mock('@/components/SessionManagement', () => ({
  SessionManagement: () => <div data-testid="session-management" />,
}));
vi.mock('@/components/ChangePasswordCard', () => ({
  ChangePasswordCard: () => <div data-testid="change-password-card" />,
}));
vi.mock('@/components/TwoFactorSetup', () => ({
  TwoFactorSetup: () => <div data-testid="two-factor-setup" />,
}));
vi.mock('@/components/PasskeyManagement', () => ({
  PasskeyManagement: () => <div data-testid="passkey-management" />,
}));
vi.mock('@/components/LinkedAccounts', () => ({
  LinkedAccounts: () => <div data-testid="linked-accounts" />,
}));

import { SettingsPage } from '@/pages/settings/SettingsPage';

function makeSetting(partial: Partial<CompanySetting>): CompanySetting {
  return {
    id: 's-1',
    company_id: 'company-1',
    category: 'system',
    key: 'dark_mode',
    value: false,
    label: 'Dark mode',
    description: null,
    updated_at: '2026-01-01T00:00:00Z',
    updated_by: null,
    ...partial,
  };
}

const fixtures: CompanySetting[] = [
  makeSetting({}),
  makeSetting({
    id: 's-2',
    category: 'payroll',
    key: 'rounding_method',
    value: 'nearest',
    label: 'Rounding method',
  }),
];

function renderPage(entry = '/settings') {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[entry]}>
        <SettingsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('SettingsPage', () => {
  beforeEach(() => {
    Object.values(settingsMocks).forEach((m) => m.mockReset());
    settingsMocks.getSettings.mockResolvedValue(fixtures);
    settingsMocks.bulkUpdateSettings.mockResolvedValue([]);
  });

  it('renders the grouped nav and defaults to the first workspace section', async () => {
    renderPage();

    const nav = await screen.findByRole('navigation', { name: 'Settings sections' });
    expect(nav).toHaveTextContent('General');
    expect(nav).toHaveTextContent('Payroll');
    expect(nav).toHaveTextContent('Security');
    expect(nav).toHaveTextContent('Sessions');
    // First workspace section active: its settings render.
    expect(await screen.findByText('Dark mode')).toBeInTheDocument();
  });

  it('renders the sessions section when ?section=sessions', async () => {
    renderPage('/settings?section=sessions');

    expect(await screen.findByTestId('session-management')).toBeInTheDocument();
  });

  it('switches sections via the nav', async () => {
    const typer = userEvent.setup();
    renderPage();

    await screen.findByText('Dark mode');
    await typer.click(screen.getByRole('button', { name: /Security/ }));

    expect(await screen.findByTestId('change-password-card')).toBeInTheDocument();
    expect(screen.getByTestId('two-factor-setup')).toBeInTheDocument();
    expect(screen.getByTestId('passkey-management')).toBeInTheDocument();
    expect(screen.getByTestId('linked-accounts')).toBeInTheDocument();
  });

  it('shows the save bar only after an edit and saves the active category', async () => {
    const typer = userEvent.setup();
    renderPage();

    // Wait for the General section, then flip the boolean setting.
    const toggle = await screen.findByRole('button', { name: '' });
    expect(screen.queryByText(/unsaved change/)).not.toBeInTheDocument();
    await typer.click(toggle);

    expect(await screen.findByText('1 unsaved change')).toBeInTheDocument();
    await typer.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() =>
      expect(settingsMocks.bulkUpdateSettings).toHaveBeenCalledWith([
        { category: 'system', key: 'dark_mode', value: true },
      ]),
    );
  });

  it('falls back to the first section for an unknown ?section', async () => {
    renderPage('/settings?section=bogus');
    expect(await screen.findByText('Dark mode')).toBeInTheDocument();
  });
});
