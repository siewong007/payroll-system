import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { User } from '@/types';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const sessionMocks = vi.hoisted(() => ({
  getSessions: vi.fn(),
  revokeSession: vi.fn(),
  revokeOtherSessions: vi.fn(),
}));
const geofenceMocks = vi.hoisted(() => ({
  listLocations: vi.fn(),
  createLocation: vi.fn(),
  deleteLocation: vi.fn(),
  getGeofenceMode: vi.fn(),
  setGeofenceMode: vi.fn(),
}));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/sessions', () => sessionMocks);
vi.mock('@/api/geofence', () => geofenceMocks);

import { TwoFactorPrompt } from '@/components/TwoFactorPrompt';
import { SessionManagement } from '@/components/SessionManagement';
import { GeofenceCard } from '@/components/attendance/GeofenceCard';

const user: User = {
  id: 'user-1',
  email: 'person@example.com',
  full_name: 'Aisyah Rahman',
  roles: ['admin'],
  company_id: 'company-1',
  employee_id: null,
};

function renderWithClient(ui: ReactNode) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return {
    queryClient,
    ...render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>),
  };
}

beforeEach(() => {
  authMocks.useAuth.mockReset();
  Object.values(sessionMocks).forEach((m) => m.mockReset());
  Object.values(geofenceMocks).forEach((m) => m.mockReset());
});

describe('TwoFactorPrompt', () => {
  function setup(completeTwoFactorLogin = vi.fn().mockResolvedValue(user)) {
    authMocks.useAuth.mockReturnValue({ completeTwoFactorLogin });
    return completeTwoFactorLogin;
  }

  it('keeps Verify disabled until a code is entered', async () => {
    const typer = userEvent.setup();
    setup();
    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={vi.fn()} />);

    const verify = screen.getByRole('button', { name: 'Verify' });
    expect(verify).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('123456'), '123456');
    expect(verify).toBeEnabled();
  });

  it('submits the trimmed code with the mfa token and reports the user back', async () => {
    const typer = userEvent.setup();
    const complete = setup();
    const onSuccess = vi.fn();
    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={onSuccess} />);

    await typer.type(screen.getByPlaceholderText('123456'), '  123456  ');
    await typer.click(screen.getByRole('button', { name: 'Verify' }));

    await waitFor(() => expect(complete).toHaveBeenCalledWith('mfa-token', '123456'));
    expect(onSuccess).toHaveBeenCalledWith(user);
  });

  it('shows the server message on a rejected code and does not complete login', async () => {
    const typer = userEvent.setup();
    const complete = vi
      .fn()
      .mockRejectedValue({ response: { data: { error: 'Invalid or expired code' } } });
    setup(complete);
    const onSuccess = vi.fn();
    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={onSuccess} />);

    await typer.type(screen.getByPlaceholderText('123456'), '000000');
    await typer.click(screen.getByRole('button', { name: 'Verify' }));

    expect(await screen.findByText('Invalid or expired code')).toBeInTheDocument();
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it('re-enables the form after a failure so the code can be retried', async () => {
    const typer = userEvent.setup();
    const complete = vi.fn().mockRejectedValue(new Error('Network unavailable'));
    setup(complete);
    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={vi.fn()} />);

    await typer.type(screen.getByPlaceholderText('123456'), '123456');
    await typer.click(screen.getByRole('button', { name: 'Verify' }));

    expect(await screen.findByText('Network unavailable')).toBeInTheDocument();
    // A stuck "Verifying..." state would strand the user mid-login.
    await waitFor(() => expect(screen.getByRole('button', { name: 'Verify' })).toBeEnabled());
  });

  it('accepts a backup code, which is not six digits', async () => {
    const typer = userEvent.setup();
    const complete = setup();
    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={vi.fn()} />);

    await typer.type(screen.getByPlaceholderText('123456'), 'a1b2c3d4');
    await typer.click(screen.getByRole('button', { name: 'Verify' }));

    await waitFor(() => expect(complete).toHaveBeenCalledWith('mfa-token', 'a1b2c3d4'));
  });

  it('offers a way back to login only when a handler is supplied', async () => {
    const typer = userEvent.setup();
    setup();
    const onBack = vi.fn();
    const { unmount } = render(
      <TwoFactorPrompt mfaToken="mfa-token" onSuccess={vi.fn()} onBack={onBack} />,
    );

    await typer.click(screen.getByRole('button', { name: 'Back to login' }));
    expect(onBack).toHaveBeenCalledOnce();
    unmount();

    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={vi.fn()} />);
    expect(screen.queryByRole('button', { name: 'Back to login' })).not.toBeInTheDocument();
  });

  it('marks the input as a one-time code for autofill', () => {
    setup();
    render(<TwoFactorPrompt mfaToken="mfa-token" onSuccess={vi.fn()} />);

    const input = screen.getByPlaceholderText('123456');
    expect(input).toHaveAttribute('autocomplete', 'one-time-code');
    expect(input).toHaveAttribute('inputmode', 'numeric');
  });
});

describe('SessionManagement', () => {
  const sessions = [
    {
      id: 'sess-current',
      user_agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)',
      created_at: '2026-07-20T02:00:00Z',
      last_seen_at: '2026-07-27T02:00:00Z',
      expires_at: '2026-08-27T02:00:00Z',
      current: true,
    },
    {
      id: 'sess-phone',
      user_agent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0)',
      created_at: '2026-07-21T02:00:00Z',
      last_seen_at: '2026-07-26T02:00:00Z',
      expires_at: '2026-08-26T02:00:00Z',
      current: false,
    },
  ];

  it('labels devices from the user agent and badges the current one', async () => {
    sessionMocks.getSessions.mockResolvedValue(sessions);
    renderWithClient(<SessionManagement />);

    expect(await screen.findByText('Mac computer')).toBeInTheDocument();
    expect(screen.getByText('Mobile device')).toBeInTheDocument();
    expect(screen.getByText('This device')).toBeInTheDocument();
  });

  it('falls back to a placeholder when the user agent is missing', async () => {
    sessionMocks.getSessions.mockResolvedValue([{ ...sessions[1], user_agent: null }]);
    renderWithClient(<SessionManagement />);

    expect(await screen.findByText('Unknown device')).toBeInTheDocument();
  });

  it('offers no sign-out control for the current session', async () => {
    sessionMocks.getSessions.mockResolvedValue(sessions);
    renderWithClient(<SessionManagement />);

    await screen.findByText('Mac computer');
    // Revoking your own session would log you out of the page you are on.
    expect(screen.getAllByRole('button', { name: 'Sign out device' })).toHaveLength(1);
  });

  it('revokes a single session and refreshes the list', async () => {
    const typer = userEvent.setup();
    sessionMocks.getSessions.mockResolvedValue(sessions);
    sessionMocks.revokeSession.mockResolvedValue(undefined);
    renderWithClient(<SessionManagement />);

    await screen.findByText('Mobile device');
    await typer.click(screen.getByRole('button', { name: 'Sign out device' }));

    // React Query v5 appends a mutation-context argument, so assert on the id.
    await waitFor(() => expect(sessionMocks.revokeSession).toHaveBeenCalled());
    expect(sessionMocks.revokeSession.mock.calls[0][0]).toBe('sess-phone');
    await waitFor(() => expect(sessionMocks.getSessions).toHaveBeenCalledTimes(2));
  });

  it('confirms before signing out every other device', async () => {
    const typer = userEvent.setup();
    sessionMocks.getSessions.mockResolvedValue(sessions);
    sessionMocks.revokeOtherSessions.mockResolvedValue(undefined);
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
    renderWithClient(<SessionManagement />);

    await screen.findByText('Mac computer');
    await typer.click(screen.getByRole('button', { name: 'Sign out all others' }));

    expect(confirm).toHaveBeenCalled();
    expect(sessionMocks.revokeOtherSessions).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    await typer.click(screen.getByRole('button', { name: 'Sign out all others' }));
    await waitFor(() => expect(sessionMocks.revokeOtherSessions).toHaveBeenCalledOnce());
    confirm.mockRestore();
  });

  it('hides the bulk control when this is the only session', async () => {
    sessionMocks.getSessions.mockResolvedValue([sessions[0]]);
    renderWithClient(<SessionManagement />);

    await screen.findByText('Mac computer');
    expect(screen.queryByRole('button', { name: 'Sign out all others' })).not.toBeInTheDocument();
  });

  it('reports an empty list rather than rendering nothing', async () => {
    sessionMocks.getSessions.mockResolvedValue([]);
    renderWithClient(<SessionManagement />);

    expect(await screen.findByText('No active sessions found.')).toBeInTheDocument();
  });
});

describe('GeofenceCard', () => {
  const location = {
    id: 'loc-1',
    company_id: 'company-1',
    name: 'HQ Office',
    latitude: 3.157_64,
    longitude: 101.711_86,
    radius_meters: 200,
    is_active: true,
    created_at: '2026-07-01T00:00:00Z',
    updated_at: '2026-07-01T00:00:00Z',
  };

  /** The section toggle and the form's submit button are both labelled "Add";
   *  the toggle renders first, so index order disambiguates them. */
  const openAddForm = () => screen.getAllByRole('button', { name: 'Add' })[0];
  const submitAddForm = () => screen.getAllByRole('button', { name: 'Add' })[1];

  beforeEach(() => {
    geofenceMocks.listLocations.mockResolvedValue([location]);
    geofenceMocks.getGeofenceMode.mockResolvedValue({ mode: 'none' });
    geofenceMocks.setGeofenceMode.mockResolvedValue(undefined);
    geofenceMocks.createLocation.mockResolvedValue(location);
    geofenceMocks.deleteLocation.mockResolvedValue(undefined);
  });

  it('offers all three enforcement modes', async () => {
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    for (const label of ['Off', 'Warn', 'Enforce']) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
  });

  it('switches the enforcement mode and refreshes it', async () => {
    const typer = userEvent.setup();
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    await typer.click(screen.getByText('Enforce'));

    await waitFor(() => expect(geofenceMocks.setGeofenceMode).toHaveBeenCalledWith('enforce'));
    await waitFor(() => expect(geofenceMocks.getGeofenceMode).toHaveBeenCalledTimes(2));
  });

  it('defaults to off when the server has no mode recorded', async () => {
    geofenceMocks.getGeofenceMode.mockResolvedValue({});
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    // Failing open to "none" is deliberate: an unknown mode must not start
    // blocking check-ins.
    expect(geofenceMocks.setGeofenceMode).not.toHaveBeenCalled();
  });

  it('renders a location with its rounded coordinates and radius', async () => {
    renderWithClient(<GeofenceCard />);

    expect(await screen.findByText('HQ Office')).toBeInTheDocument();
    expect(screen.getByText(/3\.1576, 101\.7119 · 200m radius/)).toBeInTheDocument();
  });

  it('keeps Add disabled until name and both coordinates are supplied', async () => {
    const typer = userEvent.setup();
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    await typer.click(openAddForm());

    const add = submitAddForm();
    expect(add).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('Location name (e.g. HQ Office)'), 'Branch');
    await typer.type(screen.getByPlaceholderText('Latitude'), '3.20');
    expect(add).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('Longitude'), '101.60');
    expect(add).toBeEnabled();
  });

  it('creates a location with parsed numeric coordinates', async () => {
    const typer = userEvent.setup();
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    await typer.click(openAddForm());
    await typer.type(screen.getByPlaceholderText('Location name (e.g. HQ Office)'), 'Branch');
    await typer.type(screen.getByPlaceholderText('Latitude'), '3.20');
    await typer.type(screen.getByPlaceholderText('Longitude'), '101.60');

    await typer.click(submitAddForm());

    // The API contract is numeric; sending the raw strings would 422.
    await waitFor(() =>
      expect(geofenceMocks.createLocation).toHaveBeenCalledWith({
        name: 'Branch',
        latitude: 3.2,
        longitude: 101.6,
        radius_meters: 200,
      }),
    );
  });

  it('surfaces a rejected radius from the server', async () => {
    const typer = userEvent.setup();
    geofenceMocks.createLocation.mockRejectedValue({
      response: { data: { error: 'Radius must be between 10 and 10,000 meters' } },
    });
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    await typer.click(openAddForm());
    await typer.type(screen.getByPlaceholderText('Location name (e.g. HQ Office)'), 'Branch');
    await typer.type(screen.getByPlaceholderText('Latitude'), '3.20');
    await typer.type(screen.getByPlaceholderText('Longitude'), '101.60');

    await typer.click(submitAddForm());

    expect(
      await screen.findByText('Radius must be between 10 and 10,000 meters'),
    ).toBeInTheDocument();
  });

  it('fills coordinates from the browser geolocation API', async () => {
    const typer = userEvent.setup();
    const getCurrentPosition = vi.fn((success: PositionCallback) =>
      success({ coords: { latitude: 3.123456, longitude: 101.654321 } } as GeolocationPosition),
    );
    vi.stubGlobal('navigator', { ...navigator, geolocation: { getCurrentPosition } });
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    await typer.click(openAddForm());
    await typer.click(screen.getByText('Use my current location'));

    expect(screen.getByPlaceholderText('Latitude')).toHaveValue('3.123456');
    expect(screen.getByPlaceholderText('Longitude')).toHaveValue('101.654321');
    vi.unstubAllGlobals();
  });

  it('deletes a location and refreshes the list', async () => {
    const typer = userEvent.setup();
    renderWithClient(<GeofenceCard />);

    await screen.findByText('HQ Office');
    await typer.click(screen.getByTitle('Remove location'));

    await waitFor(() => expect(geofenceMocks.deleteLocation).toHaveBeenCalledWith('loc-1'));
    await waitFor(() => expect(geofenceMocks.listLocations).toHaveBeenCalledTimes(2));
  });
});
