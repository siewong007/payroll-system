import { lazy, Suspense, useState, useMemo, useEffect } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { keepPreviousData, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Search, Pencil, Trash2, AlertTriangle, Upload, X, Users } from 'lucide-react';
import { useNavigate, useSearchParams } from 'react-router';
import { getEmployees, deleteEmployee } from '@/api/employees';
import { formatMYR, formatDate } from '@/lib/utils';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import type { Employee } from '@/types';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
const EmployeeProfile = lazy(() =>
  import('./EmployeeProfileDrawer').then((m) => ({ default: m.EmployeeProfile }))
);
import { EmployeeFormModal } from './EmployeeFormModal';

const getColumns = (t: (key: string) => string): Column<Employee>[] => [
  {
    key: 'employee',
    primary: true,
    header: t('employees.columns.employee'),
    render: (emp) => (
      <div className="min-w-0">
        <div className="font-medium text-gray-900 dark:text-slate-100 truncate">{emp.full_name}</div>
        <div className="text-xs text-gray-500 dark:text-slate-400">{emp.employee_number}</div>
      </div>
    ),
    summaryRender: (emp) => (
      <div>
        <div className="font-semibold text-base text-gray-900 dark:text-slate-100">{emp.full_name}</div>
        <div className="text-gray-500 dark:text-slate-400">{emp.employee_number}</div>
      </div>
    ),
  },
  {
    key: 'department',
    header: t('employees.columns.department'),
    render: (emp) => <span className="text-gray-600 dark:text-slate-300">{emp.department || '-'}</span>,
  },
  {
    key: 'designation',
    header: t('employees.columns.designation'),
    render: (emp) => <span className="text-gray-600 dark:text-slate-300">{emp.designation || '-'}</span>,
  },
  {
    key: 'joined',
    header: t('employees.columns.joined'),
    render: (emp) => <span className="text-gray-600 dark:text-slate-300 whitespace-nowrap">{formatDate(emp.date_joined)}</span>,
  },
  {
    key: 'salary',
    header: t('employees.columns.salary'),
    align: 'right',
    render: (emp) => <span className="font-medium text-gray-900 dark:text-slate-100 tabular-nums whitespace-nowrap">{formatMYR(emp.basic_salary)}</span>,
  },
  {
    key: 'status',
    header: t('employees.columns.status'),
    align: 'center',
    render: (emp) => (
      <span
        className={`inline-flex px-2.5 py-0.5 rounded-full text-xs font-medium whitespace-nowrap ${
          emp.is_active
            ? 'bg-green-50 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300'
            : 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300'
        }`}
      >
        {emp.is_active ? t('common.active') : t('common.inactive')}
      </span>
    ),
  },
];

export function EmployeeList() {
  const { t } = useTranslation();
  const toast = useToast();
  const { can } = usePermissions();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const canViewPayroll = can('view_payroll');
  const canManageEmployees = can('manage_employees');
  const canImportEmployees = can('import_employees');
  const queryClient = useQueryClient();
  const [searchInput, setSearchInput] = useState(() => searchParams.get('q') ?? '');
  const [search, setDebouncedSearch] = useState(() => searchParams.get('q') ?? '');
  const [page, setPageState] = useState(() => Math.max(1, Number(searchParams.get('page')) || 1));
  const [showCreate, setShowCreate] = useState(false);
  const [editingEmployee, setEditingEmployee] = useState<Employee | null>(null);
  const [deletingEmployee, setDeletingEmployee] = useState<Employee | null>(null);
  const perPage = 20;

  useEffect(() => {
    if (searchInput === search) return;
    if (!searchInput) {
      setDebouncedSearch('');
      setPageState(1);
      const params = new URLSearchParams(searchParams);
      params.delete('q');
      params.delete('page');
      setSearchParams(params, { replace: true });
      return;
    }
    const timer = window.setTimeout(() => {
      setDebouncedSearch(searchInput);
      setPageState(1);
      const params = new URLSearchParams(searchParams);
      params.set('q', searchInput);
      params.delete('page');
      setSearchParams(params, { replace: true });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [search, searchInput, searchParams, setSearchParams]);

  const setSearch = (val: string) => {
    setSearchInput(val);
  };

  const setPage = (p: number) => {
    setPageState(p);
    const params = new URLSearchParams(searchParams);
    if (search) params.set('q', search);
    else params.delete('q');
    if (p > 1) params.set('page', String(p));
    else params.delete('page');
    setSearchParams(params, { replace: true });
  };

  const filteredColumns = useMemo(
    () => {
      const cols = getColumns(t);
      return canViewPayroll ? cols : cols.filter((c) => c.key !== 'salary');
    },
    [canViewPayroll, t],
  );

  const { data, isLoading, isFetching, isError, error, refetch } = useQuery({
    queryKey: ['employees', { search, page, perPage }],
    queryFn: () => getEmployees({ search: search || undefined, page, per_page: perPage }),
    placeholderData: keepPreviousData,
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteEmployee(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employees-picker'] });
      toast.success(t('employees.deleteSuccess'));
      setDeletingEmployee(null);
    },
    onError: (err) => {
      toast.error(getErrorMessage(err, t('employees.deleteFailed')));
    },
  });

  return (
    <div>
      <PageHeader
        title={t('employees.title')}
        actions={
          (canImportEmployees || canManageEmployees) ? (
            <>
              {canImportEmployees && (
                <button
                  type="button"
                  onClick={() => navigate('/employees/import')}
                  className="btn-secondary w-full sm:w-auto"
                >
                  <Upload className="w-4 h-4" aria-hidden="true" />
                  {t('common.import')}
                </button>
              )}
              {canManageEmployees && (
                <button
                  type="button"
                  onClick={() => setShowCreate(true)}
                  className="btn-primary w-full sm:w-auto"
                >
                  <Plus className="w-4 h-4" aria-hidden="true" />
                  {t('employees.add')}
                </button>
              )}
            </>
          ) : undefined
        }
      />

      {/* Search */}
      <div className="relative mb-6">
        <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-slate-500 pointer-events-none" aria-hidden="true" />
        <input
          type="text"
          placeholder={t('employees.pickerPlaceholder')}
          aria-label={t('employees.pickerPlaceholder')}
          value={searchInput}
          onChange={(e) => setSearch(e.target.value)}
          className="form-input pl-10 pr-11 py-2.5"
        />
        {searchInput && (
          <button
            type="button"
            onClick={() => setSearch('')}
            aria-label={t('common.clear')}
            title={t('common.clear')}
            className="absolute right-1.5 top-1/2 -translate-y-1/2 min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-md text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 transition-colors"
          >
            <X className="w-4 h-4" aria-hidden="true" />
          </button>
        )}
      </div>

      {isError ? (
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('employees.loadFailed')}
            description={getErrorMessage(error, t('employees.loadFailed'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      ) : (
        <div className={isFetching && !isLoading ? 'opacity-65 transition-opacity' : ''}>
          <DataTable
            columns={filteredColumns}
            data={data?.data ?? []}
            total={data?.total}
            page={page}
            onPageChange={setPage}
            perPage={perPage}
            isLoading={isLoading}
            emptyIcon={<Users className="w-8 h-8 text-gray-300 dark:text-slate-600" aria-hidden="true" />}
            emptyMessage={search ? t('employees.noMatch') : t('employees.empty')}
            emptyAction={
              search ? (
                <button type="button" onClick={() => setSearch('')} className="btn-secondary">
                  {t('reports.clearSearch')}
                </button>
              ) : canManageEmployees ? (
                <button type="button" onClick={() => setShowCreate(true)} className="btn-primary">
                  <Plus className="w-4 h-4" aria-hidden="true" />
                  {t('employees.add')}
                </button>
              ) : undefined
            }
            summaryTitle={(emp) => emp.full_name}
            renderSummary={(emp) => (
              <Suspense fallback={null}>
                <EmployeeProfile employeeId={emp.id} />
              </Suspense>
            )}
            renderSummaryFooter={
              canManageEmployees
                ? (emp, close) => (
                    <div className="flex flex-col-reverse sm:flex-row items-stretch sm:items-center justify-between gap-2">
                      <button
                        type="button"
                        onClick={() => {
                          close();
                          setDeletingEmployee(emp);
                        }}
                        className="btn-secondary text-red-600 dark:text-red-400 border-red-200 dark:border-red-500/30 hover:bg-red-50 dark:hover:bg-red-500/15"
                      >
                        <Trash2 className="w-4 h-4" aria-hidden="true" />
                        {t('common.delete')}
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          close();
                          setEditingEmployee(emp);
                        }}
                        className="btn-primary"
                      >
                        <Pencil className="w-4 h-4" aria-hidden="true" />
                        {t('employees.editEmployee')}
                      </button>
                    </div>
                  )
                : undefined
            }
          />
        </div>
      )}

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
        onClose={() => { if (!deleteMutation.isPending) setDeletingEmployee(null); }}
        title={t('employees.deleteTitle')}
        footer={
          deletingEmployee ? (
            <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
              <button
                type="button"
                onClick={() => setDeletingEmployee(null)}
                disabled={deleteMutation.isPending}
                className="btn-secondary"
              >
                {t('common.cancel')}
              </button>
              <button
                type="button"
                onClick={() => deleteMutation.mutate(deletingEmployee.id)}
                disabled={deleteMutation.isPending}
                className="btn-danger"
              >
                {deleteMutation.isPending ? t('common.deleting') : t('employees.deleteTitle')}
              </button>
            </div>
          ) : undefined
        }
      >
        {deletingEmployee && (
          <div className="flex items-start gap-4 p-5 bg-red-50 dark:bg-red-500/10 rounded-xl border border-red-100 dark:border-red-500/30">
            <div className="w-10 h-10 rounded-full bg-red-100 dark:bg-red-500/20 flex items-center justify-center shrink-0">
              <AlertTriangle className="w-5 h-5 text-red-600 dark:text-red-400" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold text-red-800 dark:text-red-300">{t('employees.deleteWarning')}</p>
              <p className="text-sm text-red-700 dark:text-red-300/90 mt-1.5 leading-relaxed break-words">
                <Trans
                  i18nKey="employees.deleteBody"
                  values={{ name: deletingEmployee.full_name, number: deletingEmployee.employee_number }}
                  components={{ b: <span className="font-semibold" /> }}
                />
              </p>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
