import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const networkMocks = vi.hoisted(() => ({
  listNetworks: vi.fn(),
  createNetwork: vi.fn(),
  updateNetwork: vi.fn(),
  deleteNetwork: vi.fn(),
  getNetworkMode: vi.fn(),
  setNetworkMode: vi.fn(),
  listCandidates: vi.fn(),
  approveCandidate: vi.fn(),
  dismissCandidate: vi.fn(),
  getNetworkWhoami: vi.fn(),
  toCidr: (n: { network: string; prefix_len: number }) => `${n.network}/${n.prefix_len}`,
}));

vi.mock('@/api/attendanceNetworks', () => networkMocks);

import { NetworkCard } from '@/components/attendance/NetworkCard';

const network = {
  id: 'net-1',
  company_id: 'company-1',
  label: 'HQ Wi-Fi',
  network: '203.0.113.0',
  prefix_len: 24,
  is_active: true,
  approved_by: 'user-1',
  approved_at: '2026-07-01T00:00:00Z',
  learned_from_observation: false,
  created_at: '2026-07-01T00:00:00Z',
  updated_at: '2026-07-01T00:00:00Z',
};

const whoami = {
  client_ip: '203.0.113.5',
  suggested_cidr: '203.0.113.5/32',
  is_approved: true,
  matched_label: 'HQ Wi-Fi',
  has_approved_networks: true,
  trust_proxy_headers: true,
};

function renderWithClient(ui: ReactNode) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

beforeEach(() => {
  vi.clearAllMocks();
  networkMocks.listNetworks.mockResolvedValue([network]);
  networkMocks.getNetworkMode.mockResolvedValue({ mode: 'none' });
  networkMocks.setNetworkMode.mockResolvedValue(undefined);
  networkMocks.listCandidates.mockResolvedValue([]);
  networkMocks.getNetworkWhoami.mockResolvedValue(whoami);
  networkMocks.createNetwork.mockResolvedValue(network);
  networkMocks.deleteNetwork.mockResolvedValue(undefined);
  networkMocks.approveCandidate.mockResolvedValue(network);
  networkMocks.dismissCandidate.mockResolvedValue(undefined);
});

describe('NetworkCard', () => {
  it('offers all four modes including Learn', async () => {
    renderWithClient(<NetworkCard />);

    await screen.findByText('HQ Wi-Fi');
    for (const label of ['Off', 'Learn', 'Warn', 'Enforce']) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
  });

  it('defaults to off when the server reports no mode', async () => {
    networkMocks.getNetworkMode.mockResolvedValue({});
    renderWithClient(<NetworkCard />);

    await screen.findByText('HQ Wi-Fi');
    // An unrecognised mode must never start blocking check-ins.
    expect(networkMocks.setNetworkMode).not.toHaveBeenCalled();
  });

  it('surfaces the server refusal when enforcing with nothing approved', async () => {
    // The server refuses this to prevent a company-wide lockout. Swallowing the
    // error would look like a UI glitch — the toggle simply would not move.
    networkMocks.setNetworkMode.mockRejectedValue({
      response: { data: { error: 'Approve at least one office network before switching to Enforce, or nobody will be able to check in.' } },
    });
    const typer = userEvent.setup();
    renderWithClient(<NetworkCard />);

    await screen.findByText('HQ Wi-Fi');
    await typer.click(screen.getByText('Enforce'));

    expect(await screen.findByText(/Approve at least one office network/)).toBeInTheDocument();
  });

  it('states what enforcement does not prove once it is on', async () => {
    networkMocks.getNetworkMode.mockResolvedValue({ mode: 'enforce' });
    renderWithClient(<NetworkCard />);

    await screen.findByText('HQ Wi-Fi');
    // Whoever turns this on must read that it is a deterrent, not proof.
    expect(await screen.findByText(/not that the person was in the building/)).toBeInTheDocument();
    expect(screen.getByText(/VPN or SSH tunnel/)).toBeInTheDocument();
  });

  it('shows the observed address and that it matched', async () => {
    renderWithClient(<NetworkCard />);

    expect(await screen.findByText('203.0.113.5')).toBeInTheDocument();
    expect(screen.getAllByText('HQ Wi-Fi').length).toBeGreaterThan(0);
  });

  it('warns loudly when the observed address is private', async () => {
    // Every client would resolve to this same address, so approving it would
    // admit the whole internet. This is the deployment error the panel exists
    // to catch.
    networkMocks.getNetworkWhoami.mockResolvedValue({
      ...whoami,
      client_ip: '172.18.0.1',
      suggested_cidr: '172.18.0.1/32',
      is_approved: false,
      matched_label: null,
    });
    renderWithClient(<NetworkCard />);

    expect(await screen.findByText(/That is a private address/)).toBeInTheDocument();
    // and must not offer to approve it
    expect(screen.queryByText('Approve this')).not.toBeInTheDocument();
  });

  it('does not offer to approve a candidate that has not met the bar', async () => {
    networkMocks.listCandidates.mockResolvedValue([
      {
        network: '198.51.100.9',
        prefix_len: 32,
        distinct_employees: 1,
        observation_count: 40,
        anchored_count: 0,
        denied_count: 0,
        first_seen_at: '2026-07-01T00:00:00Z',
        last_seen_at: '2026-07-20T00:00:00Z',
        is_anchored: false,
        is_proposable: false,
        blocked_reason: 'No corroborating kiosk or geofence signal.',
      },
    ]);
    renderWithClient(<NetworkCard />);

    await screen.findByText('198.51.100.9/32');
    expect(screen.getByText(/No corroborating kiosk or geofence signal/)).toBeInTheDocument();
    // One employee's home address, seen 40 times — visible, but not approvable.
    expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeInTheDocument();
  });

  it('requires a name before a proposable candidate can be approved', async () => {
    networkMocks.listCandidates.mockResolvedValue([
      {
        network: '203.0.113.77',
        prefix_len: 32,
        distinct_employees: 4,
        observation_count: 30,
        anchored_count: 12,
        denied_count: 0,
        first_seen_at: '2026-07-01T00:00:00Z',
        last_seen_at: '2026-07-20T00:00:00Z',
        is_anchored: true,
        is_proposable: true,
        blocked_reason: null,
      },
    ]);
    const typer = userEvent.setup();
    renderWithClient(<NetworkCard />);

    await screen.findByText('203.0.113.77/32');
    await typer.click(screen.getByRole('button', { name: 'Approve' }));

    const save = screen.getByRole('button', { name: 'Save' });
    expect(save).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('Name this network'), 'Branch office');
    await waitFor(() => expect(save).toBeEnabled());
    await typer.click(save);

    await waitFor(() =>
      expect(networkMocks.approveCandidate).toHaveBeenCalledWith({
        cidr: '203.0.113.77/32',
        label: 'Branch office',
      })
    );
  });

  it('warns when enforcing against a single approved network', async () => {
    networkMocks.getNetworkMode.mockResolvedValue({ mode: 'enforce' });
    renderWithClient(<NetworkCard />);

    await screen.findByText('HQ Wi-Fi');
    expect(
      await screen.findByText(/If the office address changes, nobody will be able to check in/)
    ).toBeInTheDocument();
  });
});
