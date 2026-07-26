import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { User, UserWithCompanies } from '@/types';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const adminMocks = vi.hoisted(() => ({
  listUsers: vi.fn(),
  listCompanies: vi.fn(),
  createUser: vi.fn(),
  updateUser: vi.fn(),
  deleteUser: vi.fn(),
}));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/admin', () => adminMocks);

import { UserManagement } from '@/pages/admin/UserManagement';

const currentUser: User = {
  id: 'me',
  email: 'boss@example.com',
  full_name: 'Siti Boss',
  roles: ['super_admin'],
  company_id: 'company-1',
  employee_id: null,
};

const companies = [
  { id: 'company-1', name: 'Alpha Sdn Bhd' },
  { id: 'company-2', name: 'Beta Sdn Bhd' },
];

const target: UserWithCompanies = {
  id: 'user-1',
  email: 'target@example.com',
  full_name: 'Ahmad Target',
  roles: ['finance'],
  company_id: 'company-1',
  employee_id: null,
  is_active: true,
  created_at: '2026-01-01T00:00:00Z',
  companies: [companies[0]],
};

const self: UserWithCompanies = {
  ...target,
  id: 'me',
  email: currentUser.email,
  full_name: currentUser.full_name,
  roles: ['super_admin'],
};

function renderPage(rows: UserWithCompanies[] = [target, self]) {
  adminMocks.listUsers.mockResolvedValue({
    data: rows,
    total: rows.length,
    page: 1,
    per_page: 20,
  });
  adminMocks.listCompanies.mockResolvedValue(companies);
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <UserManagement />
    </QueryClientProvider> as ReactNode,
  );
}

/** DataTable renders the desktop <table> and the mobile card list side by side
 *  (CSS hides one, but jsdom applies no CSS), so row assertions are scoped to
 *  the table to avoid double-matching. */
function table() {
  return screen.getByRole('table');
}

async function findRow(name: string) {
  const grid = await screen.findByRole('table');
  return within(grid).findByText(name);
}

async function openCreateModal() {
  await userEvent.click(screen.getByRole('button', { name: 'Add User' }));
  return screen.findByRole('dialog');
}

beforeEach(() => {
  vi.clearAllMocks();
  authMocks.useAuth.mockReturnValue({ user: currentUser });
});

describe('UserManagement — listing', () => {
  it('renders the users returned by the server', async () => {
    renderPage();
    expect(await findRow('Ahmad Target')).toBeInTheDocument();
    expect(within(table()).getByText('target@example.com')).toBeInTheDocument();
  });

  it('requests a bounded page rather than the whole table', async () => {
    renderPage();
    await findRow('Ahmad Target');
    expect(adminMocks.listUsers).toHaveBeenCalledWith(
      expect.objectContaining({ page: 1, perPage: 20 }),
    );
  });

  it('sends the search term to the server and resets to the first page', async () => {
    renderPage();
    await findRow('Ahmad Target');

    await userEvent.type(screen.getByRole('searchbox', { name: /search users/i }), 'ahmad');

    await waitFor(() =>
      expect(adminMocks.listUsers).toHaveBeenLastCalledWith(
        expect.objectContaining({ search: 'ahmad', page: 1 }),
      ),
    );
  });

  it('narrows the visible rows with the role filter', async () => {
    renderPage();
    await findRow('Ahmad Target');

    await userEvent.selectOptions(screen.getByRole('combobox', { name: /filter by role/i }), 'finance');

    expect(within(table()).getByText('Ahmad Target')).toBeInTheDocument();
    expect(within(table()).queryByText('Siti Boss')).not.toBeInTheDocument();
  });

  it('gives every row action a unique accessible name', async () => {
    renderPage([target]);
    await findRow('Ahmad Target');

    expect(within(table()).getByRole('button', { name: 'Edit Ahmad Target' })).toBeInTheDocument();
    expect(within(table()).getByRole('button', { name: 'Delete Ahmad Target' })).toBeInTheDocument();
  });

  it('hides the delete action on the signed-in user’s own row', async () => {
    renderPage();
    await findRow('Siti Boss');

    expect(within(table()).queryByRole('button', { name: 'Delete Siti Boss' })).not.toBeInTheDocument();
    expect(within(table()).getByRole('button', { name: 'Delete Ahmad Target' })).toBeInTheDocument();
  });
});

describe('UserManagement — create form', () => {
  it('exposes the dialog with an accessible name and closes on Escape', async () => {
    renderPage();
    const dialog = await openCreateModal();

    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAccessibleName('Add User');

    await userEvent.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('gives every field an accessible name', async () => {
    renderPage();
    await openCreateModal();

    expect(screen.getByRole('textbox', { name: /full name/i })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: /email/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument();
  });

  it('does not offer the employee role, which the backend refuses', async () => {
    renderPage();
    await openCreateModal();

    const roles = screen.getByRole('group', { name: /roles/i });
    expect(within(roles).queryByRole('checkbox', { name: 'Employee' })).not.toBeInTheDocument();
    expect(within(roles).getByRole('checkbox', { name: 'Super Admin' })).toBeInTheDocument();
  });

  it('blocks submission when required fields are empty', async () => {
    renderPage();
    await openCreateModal();

    await userEvent.click(screen.getByRole('button', { name: 'Create User' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/full name is required/i);
    expect(adminMocks.createUser).not.toHaveBeenCalled();
  });

  it('rejects a password weaker than the backend policy before sending it', async () => {
    renderPage();
    await openCreateModal();

    await userEvent.type(screen.getByRole('textbox', { name: /full name/i }), 'New Person');
    await userEvent.type(screen.getByRole('textbox', { name: /email/i }), 'new@example.com');
    await userEvent.type(screen.getByLabelText(/password/i), 'short1A');
    await userEvent.click(screen.getByRole('checkbox', { name: 'Alpha Sdn Bhd' }));

    await userEvent.click(screen.getByRole('button', { name: 'Create User' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/at least 10 characters/i);
    expect(adminMocks.createUser).not.toHaveBeenCalled();
  });

  it('blocks submission when no company is selected', async () => {
    renderPage();
    await openCreateModal();

    await userEvent.type(screen.getByRole('textbox', { name: /full name/i }), 'New Person');
    await userEvent.type(screen.getByRole('textbox', { name: /email/i }), 'new@example.com');
    await userEvent.type(screen.getByLabelText(/password/i), 'Str0ngPassword');

    await userEvent.click(screen.getByRole('button', { name: 'Create User' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/at least one company/i);
    expect(adminMocks.createUser).not.toHaveBeenCalled();
  });

  it('truncates the company selection to one when exec is chosen', async () => {
    renderPage();
    await openCreateModal();

    await userEvent.click(screen.getByRole('checkbox', { name: 'Alpha Sdn Bhd' }));
    await userEvent.click(screen.getByRole('checkbox', { name: 'Beta Sdn Bhd' }));

    const roles = screen.getByRole('group', { name: /roles/i });
    await userEvent.click(within(roles).getByRole('checkbox', { name: 'Executive' }));

    // Radios now, and only the first selection survives.
    const alpha = screen.getByRole('radio', { name: 'Alpha Sdn Bhd' });
    const beta = screen.getByRole('radio', { name: 'Beta Sdn Bhd' });
    expect(alpha).toBeChecked();
    expect(beta).not.toBeChecked();
  });

  it('surfaces the server error when creation fails', async () => {
    renderPage();
    adminMocks.createUser.mockRejectedValue({
      response: { data: { error: 'A user with this email already exists' } },
    });
    await openCreateModal();

    await userEvent.type(screen.getByRole('textbox', { name: /full name/i }), 'New Person');
    await userEvent.type(screen.getByRole('textbox', { name: /email/i }), 'dupe@example.com');
    await userEvent.type(screen.getByLabelText(/password/i), 'Str0ngPassword');
    await userEvent.click(screen.getByRole('checkbox', { name: 'Alpha Sdn Bhd' }));
    await userEvent.click(screen.getByRole('button', { name: 'Create User' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/already exists/i);
  });
});

describe('UserManagement — edit form', () => {
  async function openEdit() {
    renderPage([target]);
    await findRow('Ahmad Target');
    await userEvent.click(within(table()).getByRole('button', { name: 'Edit Ahmad Target' }));
    return screen.findByRole('dialog');
  }

  it('omits company_ids when the selection is unchanged, so sessions survive a rename', async () => {
    adminMocks.updateUser.mockResolvedValue({});
    await openEdit();

    const name = screen.getByRole('textbox', { name: /full name/i });
    await userEvent.clear(name);
    await userEvent.type(name, 'Ahmad Renamed');
    await userEvent.click(screen.getByRole('button', { name: 'Save Changes' }));

    await waitFor(() => expect(adminMocks.updateUser).toHaveBeenCalled());
    const [, payload] = adminMocks.updateUser.mock.calls[0];
    expect(payload).not.toHaveProperty('company_ids');
    expect(payload).toMatchObject({ full_name: 'Ahmad Renamed' });
  });

  it('includes company_ids once the selection actually changes', async () => {
    adminMocks.updateUser.mockResolvedValue({});
    await openEdit();

    await userEvent.click(screen.getByRole('checkbox', { name: 'Beta Sdn Bhd' }));
    await userEvent.click(screen.getByRole('button', { name: 'Save Changes' }));

    await waitFor(() => expect(adminMocks.updateUser).toHaveBeenCalled());
    const [, payload] = adminMocks.updateUser.mock.calls[0];
    expect(payload.company_ids).toEqual(['company-1', 'company-2']);
  });

  it('warns that an access change will sign the user out of every device', async () => {
    await openEdit();

    expect(screen.queryByText(/signs this user out/i)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('checkbox', { name: 'Beta Sdn Bhd' }));

    expect(screen.getByText(/signs this user out of all devices/i)).toBeInTheDocument();
  });

  it('offers the employee role when editing an existing account', async () => {
    await openEdit();
    const roles = screen.getByRole('group', { name: /roles/i });
    expect(within(roles).getByRole('checkbox', { name: 'Employee' })).toBeInTheDocument();
  });
});

describe('UserManagement — delete', () => {
  it('shows the server error and keeps the dialog open when deletion fails', async () => {
    renderPage([target]);
    await findRow('Ahmad Target');
    adminMocks.deleteUser.mockRejectedValue({
      response: { data: { error: 'At least one active super admin must remain' } },
    });

    await userEvent.click(within(table()).getByRole('button', { name: 'Delete Ahmad Target' }));
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/super admin must remain/i);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    // Re-enabled, so the operator can correct and retry.
    expect(screen.getByRole('button', { name: 'Delete' })).toBeEnabled();
  });

  it('closes and refreshes the list on success', async () => {
    renderPage([target]);
    await findRow('Ahmad Target');
    adminMocks.deleteUser.mockResolvedValue({ ok: true });

    await userEvent.click(within(table()).getByRole('button', { name: 'Delete Ahmad Target' }));
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(adminMocks.deleteUser).toHaveBeenCalledWith('user-1'));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});
