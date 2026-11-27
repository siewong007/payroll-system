import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { UserGroupMember } from '@/api/userGroups';
import { userWithRoles } from './support/permissions';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const employeeMocks = vi.hoisted(() => ({ getEmployees: vi.fn() }));
const userGroupsMocks = vi.hoisted(() => ({
  list: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  remove: vi.fn(),
  members: vi.fn(),
  addMember: vi.fn(),
  removeMember: vi.fn(),
}));
const permissionsMocks = vi.hoisted(() => ({ matrix: vi.fn() }));
const adminMocks = vi.hoisted(() => ({
  listCompanies: vi.fn(),
  listUsers: vi.fn(),
}));
const backupMocks = vi.hoisted(() => ({
  exportCompanyBackup: vi.fn(),
  importCompanyBackup: vi.fn(),
}));
const auditMocks = vi.hoisted(() => ({
  getAuditLogs: vi.fn(),
  getAuditFilterOptions: vi.fn(),
}));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/employees', () => employeeMocks);
vi.mock('@/api/userGroups', () => ({ userGroupsApi: userGroupsMocks }));
vi.mock('@/api/permissions', () => ({ permissionsApi: permissionsMocks }));
vi.mock('@/api/admin', () => adminMocks);
vi.mock('@/api/backup', () => backupMocks);
vi.mock('@/api/audit', () => auditMocks);

import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { UserGroups } from '@/pages/admin/UserGroups';
import { BackupPage } from '@/pages/backup/BackupPage';
import { AuditTrailPage } from '@/pages/audit/AuditTrailPage';

function renderWithQuery(ui: ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

beforeEach(() => {
  vi.clearAllMocks();
  authMocks.useAuth.mockReturnValue({
    user: userWithRoles(['super_admin'], { company_id: 'comp-1', full_name: 'Siti Boss' }),
  });
});

describe('EmployeePicker', () => {
  it('supports keyboard navigation (ArrowDown, Enter, Escape)', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    employeeMocks.getEmployees.mockResolvedValue({
      data: [
        {
          id: 'emp-1',
          full_name: 'Aisyah Rahman',
          employee_number: 'EMP001',
          position: 'Engineer',
          department: 'Tech',
          is_active: true,
        },
        {
          id: 'emp-2',
          full_name: 'Bala Subra',
          employee_number: 'EMP002',
          position: 'Analyst',
          department: 'Finance',
          is_active: true,
        },
      ],
      total: 2,
      page: 1,
      per_page: 20,
    });

    renderWithQuery(<EmployeePicker value="" onChange={onChange} />);

    const input = screen.getByRole('combobox');
    await user.click(input);
    expect(await screen.findByRole('option', { name: /Aisyah Rahman/i })).toBeInTheDocument();

    await user.keyboard('{ArrowDown}{ArrowDown}{Enter}');
    expect(onChange).toHaveBeenCalledWith('emp-2', 'Bala Subra (EMP002)');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();

    await user.click(input);
    expect(await screen.findByRole('listbox')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('shows an error message with a Retry button and recovers when retried', async () => {
    const user = userEvent.setup();
    employeeMocks.getEmployees
      .mockRejectedValueOnce(new Error('Service unavailable'))
      .mockResolvedValueOnce({
        data: [
          {
            id: 'emp-1',
            full_name: 'Aisyah Rahman',
            employee_number: 'EMP001',
            position: 'Engineer',
            department: 'Tech',
            is_active: true,
          },
        ],
        total: 1,
        page: 1,
        per_page: 20,
      });

    renderWithQuery(<EmployeePicker value="" onChange={vi.fn()} />);

    await user.click(screen.getByRole('combobox'));
    const retryBtn = await screen.findByRole('button', { name: /retry/i });
    await user.click(retryBtn);

    expect(await screen.findByRole('option', { name: /Aisyah Rahman/i })).toBeInTheDocument();
  });
});

describe('UserGroups member management', () => {
  it('lists current members and allows adding and removing a member', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);

    let currentMembers: UserGroupMember[] = [
      {
        group_id: 'g-1',
        user_id: 'u-1',
        added_at: '2026-01-01T00:00:00Z',
        full_name: 'Farah Ali',
        email: 'farah@example.com',
      },
    ];

    permissionsMocks.matrix.mockResolvedValue({
      permissions: [{ key: 'view_calendar', label: 'View calendar', group: 'Calendar' }],
      roles: [],
    });
    userGroupsMocks.list.mockImplementation(async () => [
      {
        id: 'g-1',
        company_id: 'comp-1',
        name: 'Calendar Editors',
        description: 'Holiday maintainers',
        is_active: true,
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:00Z',
        permissions: ['view_calendar'],
        member_count: currentMembers.length,
      },
    ]);
    userGroupsMocks.members.mockImplementation(async () => currentMembers);
    adminMocks.listUsers.mockResolvedValue({
      data: [
        {
          id: 'u-1',
          full_name: 'Farah Ali',
          email: 'farah@example.com',
          roles: ['hr_manager'],
          company_id: 'comp-1',
          employee_id: null,
          is_active: true,
          created_at: '2026-01-01T00:00:00Z',
          companies: [],
        },
        {
          id: 'u-2',
          full_name: 'Hafiz Noor',
          email: 'hafiz@example.com',
          roles: ['hr_manager'],
          company_id: 'comp-1',
          employee_id: null,
          is_active: true,
          created_at: '2026-01-01T00:00:00Z',
          companies: [],
        },
      ],
      total: 2,
      page: 1,
      per_page: 100,
    });
    userGroupsMocks.addMember.mockImplementation(async (_groupId: string, userId: string) => {
      if (userId === 'u-2') {
        currentMembers = [
          ...currentMembers,
          {
            group_id: 'g-1',
            user_id: 'u-2',
            added_at: '2026-01-02T00:00:00Z',
            full_name: 'Hafiz Noor',
            email: 'hafiz@example.com',
          },
        ];
      }
    });
    userGroupsMocks.removeMember.mockImplementation(async (_groupId: string, userId: string) => {
      currentMembers = currentMembers.filter((m) => m.user_id !== userId);
    });

    renderWithQuery(<UserGroups />);

    const openBtn = await screen.findByRole('button', {
      name: 'Manage members for Calendar Editors',
    });
    await user.click(openBtn);

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Farah Ali')).toBeInTheDocument();

    await user.click(within(dialog).getByRole('button', { name: 'Add Hafiz Noor' }));
    const stepUpDialog = await screen.findByRole('dialog', { name: 'Confirm Add Group Member' });
    await user.type(within(stepUpDialog).getByLabelText(/confirm with your password/i), 'SuperSecret123!');
    await user.click(within(stepUpDialog).getByRole('button', { name: 'Add' }));
    await waitFor(() =>
      expect(userGroupsMocks.addMember).toHaveBeenCalledWith('g-1', 'u-2', {
        actor_password: 'SuperSecret123!',
      }),
    );

    const removeBtn = await within(dialog).findByRole('button', { name: 'Remove Hafiz Noor' });
    await user.click(removeBtn);
    await waitFor(() => expect(userGroupsMocks.removeMember).toHaveBeenCalledWith('g-1', 'u-2'));
    await waitFor(() =>
      expect(within(dialog).queryByRole('button', { name: 'Remove Hafiz Noor' })).not.toBeInTheDocument(),
    );
  });
});

describe('BackupPage preview and overwrite guard', () => {
  it('previews backup metadata and requires typing the target company name to overwrite', async () => {
    const user = userEvent.setup();
    adminMocks.listCompanies.mockResolvedValue([
      { id: 'comp-1', name: 'Acme Sdn Bhd' },
      { id: 'comp-2', name: 'Beta Sdn Bhd' },
    ]);
    backupMocks.importCompanyBackup.mockResolvedValue({
      new_company_id: 'comp-1',
      new_company_name: 'Acme Sdn Bhd',
      is_overwrite: true,
      records_imported: { employees: 12, payroll_runs: 3 },
      warnings: [],
    });

    const { container } = renderWithQuery(<BackupPage />);

    const targetSelect = await screen.findByLabelText(/choose a target company/i);
    await waitFor(() =>
      expect(within(targetSelect).getByRole('option', { name: 'Acme Sdn Bhd' })).toBeInTheDocument(),
    );
    await user.selectOptions(targetSelect, 'comp-1');

    const payload = JSON.stringify({
      metadata: {
        format_version: '2.0',
        exported_at: '2026-05-01T08:00:00Z',
        source_company_id: 'comp-legacy',
        source_company_name: 'Legacy Co',
        record_counts: { employees: 12, payroll_runs: 3 },
      },
    });
    const file = new File([payload], 'backup.json', { type: 'application/json' });
    const fileInput = container.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(fileInput, file);

    expect(await screen.findByText('Legacy Co')).toBeInTheDocument();
    expect(screen.getByText('2.0')).toBeInTheDocument();
    expect(screen.getByText('15')).toBeInTheDocument();

    const importBtn = screen.getByRole('button', { name: 'Import Backup' });
    expect(importBtn).toBeDisabled();

    const confirmInput = screen.getByLabelText(/confirm overwrite/i);
    await user.type(confirmInput, 'Wrong Co');
    expect(importBtn).toBeDisabled();

    await user.clear(confirmInput);
    await user.type(confirmInput, 'Acme Sdn Bhd');
    expect(importBtn).toBeEnabled();

    await user.click(importBtn);
    const stepUpDialog = await screen.findByRole('dialog', { name: 'Confirm Backup Import' });
    await user.type(within(stepUpDialog).getByLabelText(/confirm with your password/i), 'SuperSecret123!');
    await user.click(within(stepUpDialog).getByRole('button', { name: 'Import Backup' }));
    await waitFor(() =>
      expect(backupMocks.importCompanyBackup).toHaveBeenCalledWith(file, 'comp-1', false, {
        actor_password: 'SuperSecret123!',
      }),
    );
  });
});

describe('AuditTrailPage structured diff modal', () => {
  it('renders Field | Previous | New diff table highlighting changed keys', async () => {
    const user = userEvent.setup();
    auditMocks.getAuditFilterOptions.mockResolvedValue({
      entity_types: [{ value: 'employee', label: 'Employee' }],
      actions: [{ value: 'update_employee', label: 'Update employee' }],
    });
    auditMocks.getAuditLogs.mockResolvedValue({
      data: [
        {
          id: 'log-1',
          user_id: 'u-1',
          user_full_name: 'Siti Boss',
          user_email: 'boss@example.com',
          action: 'update_employee',
          entity_type: 'employee',
          entity_id: 'emp-12345678',
          description: 'Updated salary and department',
          old_values: { department: 'Ops', basic_salary: '3000.00', status: 'active' },
          new_values: { department: 'Engineering', basic_salary: '3500.00', status: 'active' },
          ip_address: '127.0.0.1',
          user_agent: 'Vitest',
          created_at: '2026-05-01T10:00:00Z',
        },
      ],
      total: 1,
      page: 1,
      per_page: 25,
    });

    renderWithQuery(<AuditTrailPage />);

    const descriptionCell = await screen.findByText('Updated salary and department');
    await user.click(descriptionCell);

    const dialog = await screen.findByRole('dialog');
    const diffTable = within(dialog).getByRole('table');
    expect(within(diffTable).getByRole('columnheader', { name: 'Field' })).toBeInTheDocument();
    expect(within(diffTable).getByRole('columnheader', { name: 'Previous Values' })).toBeInTheDocument();
    expect(within(diffTable).getByRole('columnheader', { name: 'New Values' })).toBeInTheDocument();

    const salaryRow = within(diffTable).getByText('basic_salary').closest('tr');
    const deptRow = within(diffTable).getByText('department').closest('tr');
    const statusRow = within(diffTable).getByText('status').closest('tr');

    expect(salaryRow).toHaveAttribute('data-changed', 'true');
    expect(deptRow).toHaveAttribute('data-changed', 'true');
    expect(statusRow).toHaveAttribute('data-changed', 'false');
    expect(within(diffTable).getByText('Ops')).toBeInTheDocument();
    expect(within(diffTable).getByText('Engineering')).toBeInTheDocument();
    expect(within(diffTable).getByText('3000.00')).toBeInTheDocument();
    expect(within(diffTable).getByText('3500.00')).toBeInTheDocument();
  });
});
