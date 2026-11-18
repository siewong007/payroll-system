import { useState, useMemo } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Search, Pencil, Trash2, AlertTriangle, Upload, X } from 'lucide-react';
import { useNavigate, useSearchParams } from 'react-router';
import { getEmployees, deleteEmployee } from '@/api/employees';
import { formatMYR, formatDate } from '@/lib/utils';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import type { Employee } from '@/types';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';
import { EmployeeProfile } from './EmployeeProfileDrawer';
import { EmployeeFormModal } from './EmployeeFormModal';

const getColumns = (t: (key: string) => string): Column<Employee>[] => [
  {
    key: 'employee',
    header: t('employees.columns.employee'),
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
    header: t('employees.columns.department'),
    render: (emp) => <span className="text-gray-600">{emp.department || '-'}</span>,
  },
  {
    key: 'designation',
    header: t('employees.columns.designation'),
    render: (emp) => <span className="text-gray-600">{emp.designation || '-'}</span>,
  },
  {
    key: 'joined',
    header: t('employees.columns.joined'),
    render: (emp) => <span className="text-gray-600">{formatDate(emp.date_joined)}</span>,
  },
  {
    key: 'salary',
    header: t('employees.columns.salary'),
    align: 'right',
    render: (emp) => <span className="font-medium">{formatMYR(emp.basic_salary)}</span>,
  },
  {
    key: 'status',
    header: t('employees.columns.status'),
    align: 'center',
    render: (emp) => (
      <span
        className={`inline-flex px-2 py-1 rounded-full text-xs font-medium ${
          emp.is_active ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'
        }`}
      >
        {emp.is_active ? t('common.active') : t('common.inactive')}
      </span>
    ),
  },
];

export function EmployeeList() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const canViewPayroll = canAccessPayrollData(user);
  const queryClient = useQueryClient();
  const [search, setSearchState] = useState(() => searchParams.get('q') ?? '');
  const [page, setPageState] = useState(() => Math.max(1, Number(searchParams.get('page')) || 1));
  const [showCreate, setShowCreate] = useState(false);
  const [editingEmployee, setEditingEmployee] = useState<Employee | null>(null);
  const [deletingEmployee, setDeletingEmployee] = useState<Employee | null>(null);
  const perPage = 20;

  const syncUrl = (nextQuery: string, nextPage: number) => {
    const params = new URLSearchParams(searchParams);
    if (nextQuery) params.set('q', nextQuery);
    else params.delete('q');
    if (nextPage > 1) params.set('page', String(nextPage));
    else params.delete('page');
    setSearchParams(params, { replace: true });
  };

  const setSearch = (val: string) => {
    setSearchState(val);
    setPageState(1);
    syncUrl(val, 1);
  };

  const setPage = (p: number) => {
    setPageState(p);
    syncUrl(search, p);
  };

  const filteredColumns = useMemo(
    () => {
      const cols = getColumns(t);
      return canViewPayroll ? cols : cols.filter((c) => c.key !== 'salary');
    },
    [canViewPayroll, t],
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
      <PageHeader
        title={t('employees.title')}
        actions={
          <>
            {canViewPayroll && (
              <button
                onClick={() => navigate('/employees/import')}
                className="btn-secondary w-full sm:w-auto"
              >
                <Upload className="w-4 h-4" />
                {t('common.import')}
              </button>
            )}
            <button
              onClick={() => setShowCreate(true)}
              className="btn-primary w-full sm:w-auto"
            >
              <Plus className="w-4 h-4" />
              {t('employees.add')}
            </button>
          </>
        }
      />

      {/* Search */}
      <div className="relative mb-6">
        <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" />
        <input
          type="text"
          placeholder={t('employees.pickerPlaceholder')}
          aria-label={t('employees.pickerPlaceholder')}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="form-input !pl-10 !pr-9 !py-2.5"
        />
        {search && (
          <button
            type="button"
            onClick={() => setSearch('')}
            aria-label={t('common.clear')}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 p-1 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-slate-200"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      <DataTable
        columns={filteredColumns}
        data={data?.data ?? []}
        total={data?.total}
        page={page}
        onPageChange={setPage}
        perPage={perPage}
        isLoading={isLoading}
        emptyMessage={t('employees.empty')}
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
              {t('common.delete')}
            </button>
            <button
              onClick={() => {
                close();
                setEditingEmployee(emp);
              }}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-black hover:bg-gray-800 rounded-lg transition-colors"
            >
              <Pencil className="w-4 h-4" />
              {t('employees.editEmployee')}
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
        title={t('employees.deleteTitle')}
      >
        {deletingEmployee && (
          <div className="space-y-5">
            <div className="flex items-start gap-4 p-5 bg-red-50 rounded-xl border border-red-100">
              <div className="w-10 h-10 rounded-full bg-red-100 flex items-center justify-center shrink-0">
                <AlertTriangle className="w-5 h-5 text-red-600" />
              </div>
              <div>
                <p className="text-sm font-semibold text-red-800">{t('employees.deleteWarning')}</p>
                <p className="text-sm text-red-600 mt-1.5 leading-relaxed">
                  <Trans
                    i18nKey="employees.deleteBody"
                    values={{ name: deletingEmployee.full_name, number: deletingEmployee.employee_number }}
                    components={{ b: <span className="font-semibold" /> }}
                  />
                </p>
              </div>
            </div>

            <div className="flex justify-end gap-3 pt-1">
              <button
                onClick={() => setDeletingEmployee(null)}
                className="px-5 py-2.5 text-sm text-gray-600 hover:bg-gray-100 rounded-lg font-medium border border-gray-200 transition-colors"
              >
                {t('common.cancel')}
              </button>
              <button
                onClick={() => deleteMutation.mutate(deletingEmployee.id)}
                disabled={deleteMutation.isPending}
                className="px-5 py-2.5 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 font-medium shadow-sm transition-colors"
              >
                {deleteMutation.isPending ? t('common.deleting') : t('employees.deleteTitle')}
              </button>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
