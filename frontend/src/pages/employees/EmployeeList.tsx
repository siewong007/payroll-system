import { useState, useMemo } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Search, Edit, DollarSign, Shield, MapPin, TrendingUp, TrendingDown, Pencil, Trash2, AlertTriangle, Upload } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { getEmployees, createEmployee, updateEmployee, deleteEmployee, getEmployee, getSalaryHistory } from '@/api/employees';
import { getPayrollGroups } from '@/api/payroll';
import { formatMYR, formatDate } from '@/lib/utils';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { Modal } from '@/components/ui/Modal';
import type { Employee, CreateEmployeeRequest } from '@/types';
import { useAuth } from '@/context/AuthContext';

const BANKS = [
  'Maybank', 'CIMB Bank', 'Public Bank', 'RHB Bank', 'Hong Leong Bank',
  'AmBank', 'Bank Islam', 'Bank Rakyat', 'Alliance Bank', 'Affin Bank',
  'BSN', 'OCBC Bank', 'Standard Chartered', 'HSBC', 'UOB', 'Agro Bank',
  'Bank Muamalat', 'MBSB Bank', 'Al Rajhi Bank',
];

const columns: Column<Employee>[] = [
  {
    key: 'employee',
    header: 'Employee',
    render: (emp) => (
      <div>
        <div className="font-medium">{emp.full_name}</div>
        <div className="text-xs text-gray-400">{emp.employee_number}</div>
      </div>
    ),
    summaryRender: (emp) => (
      <div>
        <div className="font-semibold text-base">{emp.full_name}</div>
        <div className="text-gray-500">{emp.employee_number}</div>
      </div>
    ),
  },
  {
    key: 'department',
    header: 'Department',
    render: (emp) => <span className="text-gray-600">{emp.department || '-'}</span>,
  },
  {
    key: 'designation',
    header: 'Designation',
    render: (emp) => <span className="text-gray-600">{emp.designation || '-'}</span>,
  },
  {
    key: 'joined',
    header: 'Joined',
    render: (emp) => <span className="text-gray-600">{formatDate(emp.date_joined)}</span>,
  },
  {
    key: 'salary',
    header: 'Basic Salary',
    align: 'right',
    render: (emp) => <span className="font-medium">{formatMYR(emp.basic_salary)}</span>,
  },
  {
    key: 'status',
    header: 'Status',
    align: 'center',
    render: (emp) => (
      <span
        className={`inline-flex px-2 py-1 rounded-full text-xs font-medium ${
          emp.is_active ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'
        }`}
      >
        {emp.is_active ? 'Active' : 'Inactive'}
      </span>
    ),
  },
];

export function EmployeeList() {
  const { user } = useAuth();
  const navigate = useNavigate();
  const isExec = user?.role === 'exec';
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const [showCreate, setShowCreate] = useState(false);
  const [editingEmployee, setEditingEmployee] = useState<Employee | null>(null);
  const [deletingEmployee, setDeletingEmployee] = useState<Employee | null>(null);
  const perPage = 20;

  const filteredColumns = useMemo(
    () => isExec ? columns.filter((c) => c.key !== 'salary') : columns,
    [isExec],
  );

  const { data, isLoading } = useQuery({
    queryKey: ['employees', { search, page, perPage }],
    queryFn: () => getEmployees({ search: search || undefined, page, per_page: perPage }),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteEmployee(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      setDeletingEmployee(null);
    },
  });

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <h1 className="text-xl sm:text-2xl font-bold text-gray-900">Employees</h1>
        <div className="flex gap-2">
          {!isExec && (
            <button
              onClick={() => navigate('/employees/import')}
              className="flex items-center justify-center gap-2 border border-gray-300 text-gray-700 px-4 py-2 rounded-lg hover:bg-gray-50 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
            >
              <Upload className="w-4 h-4" />
              Import
            </button>
          )}
          <button
            onClick={() => setShowCreate(true)}
            className="flex items-center justify-center gap-2 bg-black text-white px-4 py-2 rounded-lg hover:bg-gray-800 transition-colors text-sm font-medium w-full sm:w-auto min-h-[44px]"
          >
            <Plus className="w-4 h-4" />
            Add Employee
          </button>
        </div>
      </div>

      {/* Search */}
      <div className="relative mb-6">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
        <input
          type="text"
          placeholder="Search by name or employee number..."
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(1); }}
          className="w-full pl-10 pr-4 py-2.5 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
        />
      </div>

      <DataTable
        columns={filteredColumns}
        data={data?.data ?? []}
        total={data?.total}
        page={page}
        onPageChange={setPage}
        perPage={perPage}
        isLoading={isLoading}
        emptyMessage="No employees found"
        summaryTitle={(emp) => emp.full_name}
        renderSummary={(emp) => <EmployeeProfile employeeId={emp.id} />}
        renderSummaryFooter={(emp, close) => (
          <div className="flex items-center justify-between">
            <button
              onClick={() => {
                close();
                setDeletingEmployee(emp);
              }}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-red-600 hover:bg-red-50 rounded-lg transition-colors"
            >
              <Trash2 className="w-4 h-4" />
              Delete
            </button>
            <button
              onClick={() => {
                close();
                setEditingEmployee(emp);
              }}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-black hover:bg-gray-800 rounded-lg transition-colors"
            >
              <Pencil className="w-4 h-4" />
              Edit Employee
            </button>
          </div>
        )}
      />

      {showCreate && (
        <EmployeeFormModal
          mode="create"
          onClose={() => setShowCreate(false)}
        />
      )}

      {editingEmployee && (
        <EmployeeFormModal
          mode="edit"
          employeeId={editingEmployee.id}
          onClose={() => setEditingEmployee(null)}
        />
      )}

      {/* Delete confirmation modal */}
      <Modal
        open={deletingEmployee !== null}
        onClose={() => setDeletingEmployee(null)}
        title="Delete Employee"
      >
        {deletingEmployee && (
          <div className="space-y-5">
            <div className="flex items-start gap-4 p-5 bg-red-50 rounded-xl border border-red-100">
              <div className="w-10 h-10 rounded-full bg-red-100 flex items-center justify-center shrink-0">
                <AlertTriangle className="w-5 h-5 text-red-600" />
              </div>
              <div>
                <p className="text-sm font-semibold text-red-800">This action cannot be undone</p>
                <p className="text-sm text-red-600 mt-1.5 leading-relaxed">
                  You are about to permanently delete <span className="font-semibold">{deletingEmployee.full_name}</span> ({deletingEmployee.employee_number}).
                  All associated records may be affected.
                </p>
              </div>
            </div>

            <div className="flex justify-end gap-3 pt-1">
              <button
                onClick={() => setDeletingEmployee(null)}
                className="px-5 py-2.5 text-sm text-gray-600 hover:bg-gray-100 rounded-lg font-medium border border-gray-200 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => deleteMutation.mutate(deletingEmployee.id)}
                disabled={deleteMutation.isPending}
                className="px-5 py-2.5 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 font-medium shadow-sm transition-colors"
              >
                {deleteMutation.isPending ? 'Deleting...' : 'Delete Employee'}
              </button>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}

/* ───────────── Info Field (read-only display) ───────────── */

function InfoField({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <div>
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 mt-0.5">{value || '-'}</p>
    </div>
  );
}

/* ───────────── Employee Profile (read-only, shown in DataTable modal) ───────────── */

function EmployeeProfile({ employeeId }: { employeeId: string }) {
  const { user } = useAuth();
  const isExec = user?.role === 'exec';
  const { data: employee, isLoading } = useQuery({
    queryKey: ['employee', employeeId],
    queryFn: () => getEmployee(employeeId),
  });

  const { data: salaryHistory } = useQuery({
    queryKey: ['salaryHistory', employeeId],
    queryFn: () => getSalaryHistory(employeeId),
    enabled: !isExec,
  });

  if (isLoading || !employee) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-7">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-lg font-bold text-gray-900">{employee.full_name}</h3>
          <p className="text-sm text-gray-500">
            {employee.employee_number} &middot; {employee.department || 'No Department'} &middot; {employee.designation || 'No Designation'}
          </p>
        </div>
        <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-medium ${employee.is_active ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
          {employee.is_active ? 'Active' : 'Inactive'}
        </span>
      </div>

      {/* Personal */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
          <Edit className="w-3.5 h-3.5" /> Personal
        </h4>
        <div className="grid grid-cols-2 gap-x-8 gap-y-4">
          <InfoField label="NRIC" value={employee.ic_number} />
          <InfoField label="Date of Birth" value={employee.date_of_birth ? formatDate(employee.date_of_birth) : null} />
          <InfoField label="Gender" value={employee.gender} />
          <InfoField label="Race" value={employee.race} />
          <InfoField label="Nationality" value={employee.nationality} />
          <InfoField label="Marital Status" value={employee.marital_status} />
          <InfoField label="Email" value={employee.email} />
          <InfoField label="Phone" value={employee.phone} />
        </div>
      </section>

      {/* Address */}
      {(employee.address_line1 || employee.city || employee.state) && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
            <MapPin className="w-3.5 h-3.5" /> Address
          </h4>
          <p className="text-sm text-gray-900">
            {[employee.address_line1, employee.address_line2, employee.city, employee.state, employee.postcode]
              .filter(Boolean)
              .join(', ')}
          </p>
        </section>
      )}

      {/* Employment & Salary */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
          <DollarSign className="w-3.5 h-3.5" /> Employment & Salary
        </h4>
        <div className="grid grid-cols-2 gap-x-8 gap-y-4">
          <InfoField label="Employment Type" value={employee.employment_type?.replace('_', ' ')} />
          <InfoField label="Date Joined" value={formatDate(employee.date_joined)} />
          <InfoField label="Confirmation Date" value={employee.confirmation_date ? formatDate(employee.confirmation_date) : null} />
          {!isExec && <InfoField label="Basic Salary" value={formatMYR(employee.basic_salary)} />}
          <InfoField label="Bank" value={employee.bank_name} />
          <InfoField label="Account No" value={employee.bank_account_number} />
          <InfoField label="Cost Centre" value={employee.cost_centre} />
          <InfoField label="Branch" value={employee.branch} />
        </div>
      </section>

      {/* Statutory */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
          <Shield className="w-3.5 h-3.5" /> Statutory
        </h4>
        <div className="grid grid-cols-2 gap-x-8 gap-y-4">
          <InfoField label="TIN" value={employee.tax_identification_number} />
          <InfoField label="EPF Number" value={employee.epf_number} />
          <InfoField label="EPF Category" value={employee.epf_category} />
          <InfoField label="SOCSO Number" value={employee.socso_number} />
          <InfoField label="EIS Number" value={employee.eis_number} />
          <InfoField label="Residency" value={employee.residency_status?.replace('_', ' ')} />
          <InfoField label="Working Spouse" value={employee.working_spouse ? 'Yes' : 'No'} />
          <InfoField label="Children" value={String(employee.num_children ?? 0)} />
          <InfoField label="Muslim" value={employee.is_muslim ? 'Yes' : 'No'} />
          {employee.zakat_eligible && (
            <InfoField label="Zakat (Monthly)" value={formatMYR(employee.zakat_monthly_amount ?? 0)} />
          )}
          {(employee.ptptn_monthly_amount ?? 0) > 0 && (
            <InfoField label="PTPTN (Monthly)" value={formatMYR(employee.ptptn_monthly_amount!)} />
          )}
        </div>
      </section>

      {/* Salary History */}
      {!isExec && salaryHistory && salaryHistory.length > 0 && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
            <DollarSign className="w-3.5 h-3.5" /> Salary History
          </h4>
          <div className="rounded-2xl shadow overflow-hidden">
            <table className="w-full text-sm">
              <thead className="bg-gray-50">
                <tr>
                  <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">Date</th>
                  <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">Old</th>
                  <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">New</th>
                  <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">Change</th>
                  <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">Reason</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {salaryHistory.map((h) => {
                  const pctChange = ((h.new_salary - h.old_salary) / h.old_salary) * 100;
                  const isIncrease = pctChange >= 0;
                  return (
                    <tr key={h.id}>
                      <td className="px-4 py-2.5">{formatDate(h.effective_date)}</td>
                      <td className="px-4 py-2.5 text-right text-gray-500">{formatMYR(h.old_salary)}</td>
                      <td className="px-4 py-2.5 text-right font-medium">{formatMYR(h.new_salary)}</td>
                      <td className="px-4 py-2.5 text-right">
                        <span className={`inline-flex items-center gap-0.5 ${isIncrease ? 'text-green-600' : 'text-red-600'}`}>
                          {isIncrease ? <TrendingUp className="w-3 h-3" /> : <TrendingDown className="w-3 h-3" />}
                          {isIncrease ? '+' : ''}{pctChange.toFixed(1)}%
                        </span>
                      </td>
                      <td className="px-4 py-2.5 text-gray-500">{h.reason || '-'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}

/* ───────────── Shared Employee Form Modal (Create + Edit) ───────────── */

function employeeToForm(emp: Employee): CreateEmployeeRequest & { salaryDisplay: string } {
  return {
    employee_number: emp.employee_number,
    full_name: emp.full_name,
    ic_number: emp.ic_number ?? undefined,
    date_of_birth: emp.date_of_birth ?? undefined,
    gender: emp.gender ?? undefined,
    race: emp.race ?? undefined,
    nationality: emp.nationality ?? undefined,
    marital_status: emp.marital_status ?? undefined,
    email: emp.email ?? undefined,
    phone: emp.phone ?? undefined,
    address_line1: emp.address_line1 ?? undefined,
    address_line2: emp.address_line2 ?? undefined,
    city: emp.city ?? undefined,
    state: emp.state ?? undefined,
    postcode: emp.postcode ?? undefined,
    department: emp.department ?? undefined,
    designation: emp.designation ?? undefined,
    cost_centre: emp.cost_centre ?? undefined,
    branch: emp.branch ?? undefined,
    employment_type: emp.employment_type,
    date_joined: emp.date_joined,
    basic_salary: emp.basic_salary,
    bank_name: emp.bank_name ?? undefined,
    bank_account_number: emp.bank_account_number ?? undefined,
    tax_identification_number: emp.tax_identification_number ?? undefined,
    epf_number: emp.epf_number ?? undefined,
    socso_number: emp.socso_number ?? undefined,
    eis_number: emp.eis_number ?? undefined,
    epf_category: emp.epf_category ?? undefined,
    residency_status: emp.residency_status,
    working_spouse: emp.working_spouse ?? undefined,
    num_children: emp.num_children ?? undefined,
    is_muslim: emp.is_muslim ?? undefined,
    zakat_eligible: emp.zakat_eligible ?? undefined,
    zakat_monthly_amount: emp.zakat_monthly_amount ?? undefined,
    ptptn_monthly_amount: emp.ptptn_monthly_amount ?? undefined,
    payroll_group_id: emp.payroll_group_id ?? undefined,
    is_active: emp.is_active ?? undefined,
    salaryDisplay: (emp.basic_salary / 100).toFixed(2),
  };
}

function EmployeeFormModal({ mode, employeeId, onClose }: {
  mode: 'create' | 'edit';
  employeeId?: string;
  onClose: () => void;
}) {
  const { user } = useAuth();
  const isExec = user?.role === 'exec';
  const queryClient = useQueryClient();

  const { data: existingEmployee, isLoading: loadingEmployee } = useQuery({
    queryKey: ['employee', employeeId],
    queryFn: () => getEmployee(employeeId!),
    enabled: mode === 'edit' && !!employeeId,
  });

  const { data: payrollGroups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
  });

  const isReady = mode === 'create' || !!existingEmployee;

  return (
    <Modal open onClose={onClose} title={mode === 'create' ? 'Add New Employee' : 'Edit Employee'} maxWidth="max-w-3xl">
      {!isReady || loadingEmployee ? (
        <div className="flex items-center justify-center py-12">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900" />
        </div>
      ) : (
        <EmployeeFormContent
          mode={mode}
          employeeId={employeeId}
          initialData={existingEmployee}
          payrollGroups={payrollGroups}
          onClose={onClose}
          queryClient={queryClient}
          isExec={isExec}
        />
      )}
    </Modal>
  );
}

function EmployeeFormContent({ mode, employeeId, initialData, payrollGroups, onClose, queryClient, isExec }: {
  mode: 'create' | 'edit';
  employeeId?: string;
  initialData?: Employee;
  payrollGroups?: { id: string; name: string }[];
  onClose: () => void;
  queryClient: ReturnType<typeof useQueryClient>;
  isExec?: boolean;
}) {
  const defaults = initialData
    ? employeeToForm(initialData)
    : null;

  const [form, setForm] = useState<CreateEmployeeRequest>(defaults ?? {
    employee_number: '',
    full_name: '',
    date_joined: new Date().toISOString().split('T')[0],
    basic_salary: 0,
  });

  const [salaryDisplay, setSalaryDisplay] = useState(defaults?.salaryDisplay ?? '');

  const createMutation = useMutation({
    mutationFn: (req: CreateEmployeeRequest) => createEmployee(req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      onClose();
    },
  });

  const updateMutation = useMutation({
    mutationFn: (req: Partial<CreateEmployeeRequest>) => updateEmployee(employeeId!, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employee', employeeId] });
      onClose();
    },
  });

  const mutation = mode === 'create' ? createMutation : updateMutation;

  const updateField = (field: string, value: unknown) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  const handleSalaryChange = (value: string) => {
    setSalaryDisplay(value);
    const ringgit = parseFloat(value) || 0;
    updateField('basic_salary', Math.round(ringgit * 100));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate(form);
  };

  const inputClass = 'w-full px-4 py-2.5 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none text-sm bg-white transition-colors';
  const labelClass = 'block text-sm font-medium text-gray-600 mb-1.5';
  const sectionTitleClass = 'text-xs font-bold text-gray-500 uppercase tracking-wider mb-4';

  return (
    <form onSubmit={handleSubmit} className="space-y-8">
      {mutation.isError && (
        <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl border border-red-100">
          {(mutation.error as Error)?.message || `Failed to ${mode} employee`}
        </div>
      )}

      {/* Personal Information */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>Personal Information</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div>
            <label className={labelClass}>Employee Number *</label>
            <input
              type="text"
              value={form.employee_number}
              onChange={(e) => updateField('employee_number', e.target.value)}
              className={inputClass}
              required
              placeholder="e.g., EMP001"
            />
          </div>
          <div>
            <label className={labelClass}>Full Name (as per NRIC) *</label>
            <input
              type="text"
              value={form.full_name}
              onChange={(e) => updateField('full_name', e.target.value)}
              className={inputClass}
              required
            />
          </div>
          <div>
            <label className={labelClass}>NRIC / IC Number</label>
            <input
              type="text"
              value={form.ic_number || ''}
              onChange={(e) => updateField('ic_number', e.target.value)}
              className={inputClass}
              placeholder="e.g., 900101-14-5678"
            />
          </div>
          <div>
            <label className={labelClass}>Date of Birth</label>
            <input
              type="date"
              value={form.date_of_birth || ''}
              onChange={(e) => updateField('date_of_birth', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Gender</label>
            <select
              value={form.gender || ''}
              onChange={(e) => updateField('gender', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">Select</option>
              <option value="male">Male</option>
              <option value="female">Female</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Race</label>
            <select
              value={form.race || ''}
              onChange={(e) => updateField('race', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">Select</option>
              <option value="malay">Malay</option>
              <option value="chinese">Chinese</option>
              <option value="indian">Indian</option>
              <option value="other">Other</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Marital Status</label>
            <select
              value={form.marital_status || ''}
              onChange={(e) => updateField('marital_status', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">Select</option>
              <option value="single">Single</option>
              <option value="married">Married</option>
              <option value="divorced">Divorced</option>
              <option value="widowed">Widowed</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Email</label>
            <input
              type="email"
              value={form.email || ''}
              onChange={(e) => updateField('email', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Phone</label>
            <input
              type="tel"
              value={form.phone || ''}
              onChange={(e) => updateField('phone', e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </section>

      {/* Address */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>Address</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div className="md:col-span-2">
            <label className={labelClass}>Address Line 1</label>
            <input
              type="text"
              value={form.address_line1 || ''}
              onChange={(e) => updateField('address_line1', e.target.value)}
              className={inputClass}
            />
          </div>
          <div className="md:col-span-2">
            <label className={labelClass}>Address Line 2</label>
            <input
              type="text"
              value={form.address_line2 || ''}
              onChange={(e) => updateField('address_line2', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>City</label>
            <input
              type="text"
              value={form.city || ''}
              onChange={(e) => updateField('city', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>State</label>
            <input
              type="text"
              value={form.state || ''}
              onChange={(e) => updateField('state', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Postcode</label>
            <input
              type="text"
              value={form.postcode || ''}
              onChange={(e) => updateField('postcode', e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </section>

      {/* Employment Details */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>Employment Details</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div>
            <label className={labelClass}>Department</label>
            <input
              type="text"
              value={form.department || ''}
              onChange={(e) => updateField('department', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Designation</label>
            <input
              type="text"
              value={form.designation || ''}
              onChange={(e) => updateField('designation', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Employment Type</label>
            <select
              value={form.employment_type || 'permanent'}
              onChange={(e) => updateField('employment_type', e.target.value)}
              className={inputClass}
            >
              <option value="permanent">Permanent</option>
              <option value="contract">Contract</option>
              <option value="part_time">Part Time</option>
              <option value="intern">Intern</option>
              <option value="daily_rated">Daily Rated</option>
              <option value="hourly_rated">Hourly Rated</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Date Joined *</label>
            <input
              type="date"
              value={form.date_joined}
              onChange={(e) => updateField('date_joined', e.target.value)}
              className={inputClass}
              required
            />
          </div>
          {!isExec && (
            <>
              <div>
                <label className={labelClass}>Basic Salary (RM) *</label>
                <input
                  type="number"
                  step="0.01"
                  value={salaryDisplay}
                  onChange={(e) => handleSalaryChange(e.target.value)}
                  className={inputClass}
                  required
                  placeholder="e.g., 3000.00"
                />
              </div>
              <div>
                <label className={labelClass}>Payroll Group</label>
                <select
                  value={form.payroll_group_id || ''}
                  onChange={(e) => updateField('payroll_group_id', e.target.value || undefined)}
                  className={inputClass}
                >
                  <option value="">Select</option>
                  {payrollGroups?.map((g) => (
                    <option key={g.id} value={g.id}>{g.name}</option>
                  ))}
                </select>
              </div>
            </>
          )}
          <div>
            <label className={labelClass}>Cost Centre</label>
            <input
              type="text"
              value={form.cost_centre || ''}
              onChange={(e) => updateField('cost_centre', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Branch</label>
            <input
              type="text"
              value={form.branch || ''}
              onChange={(e) => updateField('branch', e.target.value)}
              className={inputClass}
            />
          </div>
          {mode === 'edit' && (
            <div>
              <label className={labelClass}>Status</label>
              <select
                value={form.is_active ? 'active' : 'inactive'}
                onChange={(e) => updateField('is_active', e.target.value === 'active')}
                className={inputClass}
              >
                <option value="active">Active</option>
                <option value="inactive">Inactive</option>
              </select>
            </div>
          )}
        </div>
      </section>

      {/* Banking Details */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>Banking Details</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div>
            <label className={labelClass}>Bank Name</label>
            <select
              value={form.bank_name || ''}
              onChange={(e) => updateField('bank_name', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">Select Bank</option>
              {BANKS.map((b) => (
                <option key={b} value={b}>{b}</option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelClass}>Account Number</label>
            <input
              type="text"
              value={form.bank_account_number || ''}
              onChange={(e) => updateField('bank_account_number', e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </section>

      {/* Statutory & Tax */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>Statutory & Tax</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div>
            <label className={labelClass}>TIN</label>
            <input
              type="text"
              value={form.tax_identification_number || ''}
              onChange={(e) => updateField('tax_identification_number', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>EPF Number</label>
            <input
              type="text"
              value={form.epf_number || ''}
              onChange={(e) => updateField('epf_number', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>SOCSO Number</label>
            <input
              type="text"
              value={form.socso_number || ''}
              onChange={(e) => updateField('socso_number', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>EIS Number</label>
            <input
              type="text"
              value={form.eis_number || ''}
              onChange={(e) => updateField('eis_number', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>EPF Category</label>
            <select
              value={form.epf_category || 'A'}
              onChange={(e) => updateField('epf_category', e.target.value)}
              className={inputClass}
            >
              <option value="A">A - Citizen/PR below 60</option>
              <option value="B">B - Elected 9% (KWSP 17A)</option>
              <option value="C">C - PR 60 and above</option>
              <option value="D">D - Citizen 60 and above</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Residency Status</label>
            <select
              value={form.residency_status || 'citizen'}
              onChange={(e) => updateField('residency_status', e.target.value)}
              className={inputClass}
            >
              <option value="citizen">Malaysian Citizen</option>
              <option value="permanent_resident">Permanent Resident</option>
              <option value="foreigner">Foreigner</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Working Spouse</label>
            <select
              value={form.working_spouse ? 'yes' : 'no'}
              onChange={(e) => updateField('working_spouse', e.target.value === 'yes')}
              className={inputClass}
            >
              <option value="no">No</option>
              <option value="yes">Yes</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>Number of Children</label>
            <input
              type="number"
              min="0"
              value={form.num_children ?? 0}
              onChange={(e) => updateField('num_children', parseInt(e.target.value) || 0)}
              className={inputClass}
            />
          </div>
          <div className="md:col-span-2 flex items-center gap-6 pt-2">
            <label className="flex items-center gap-2.5 cursor-pointer">
              <input
                type="checkbox"
                checked={form.is_muslim || false}
                onChange={(e) => {
                  updateField('is_muslim', e.target.checked);
                  if (!e.target.checked) {
                    updateField('zakat_eligible', false);
                    updateField('zakat_monthly_amount', undefined);
                  }
                }}
                className="rounded border-gray-200 w-4 h-4 text-gray-900 focus:ring-black"
              />
              <span className="text-sm font-medium text-gray-700">Muslim</span>
            </label>
            {form.is_muslim && (
              <label className="flex items-center gap-2.5 cursor-pointer">
                <input
                  type="checkbox"
                  checked={form.zakat_eligible || false}
                  onChange={(e) => updateField('zakat_eligible', e.target.checked)}
                  className="rounded border-gray-200 w-4 h-4 text-gray-900 focus:ring-black"
                />
                <span className="text-sm font-medium text-gray-700">Zakat Eligible</span>
              </label>
            )}
          </div>
        </div>
      </section>

      {/* Footer */}
      <div className="flex justify-end gap-3 pt-2">
        <button
          type="button"
          onClick={onClose}
          className="px-5 py-2.5 text-sm text-gray-600 hover:bg-gray-100 rounded-lg font-medium border border-gray-200 transition-colors"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={mutation.isPending}
          className="px-6 py-2.5 text-sm bg-black text-white rounded-lg hover:bg-gray-800 disabled:opacity-50 font-medium shadow-sm transition-colors"
        >
          {mutation.isPending
            ? (mode === 'create' ? 'Creating...' : 'Saving...')
            : (mode === 'create' ? 'Create Employee' : 'Save Changes')
          }
        </button>
      </div>
    </form>
  );
}
