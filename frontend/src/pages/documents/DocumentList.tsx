import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { keepPreviousData, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  Building2,
  ChevronLeft,
  ChevronRight,
  FileText,
  Link as LinkIcon,
  Loader2,
  Lock,
  Plus,
  Search,
  Trash2,
  Upload,
  User,
  X,
} from 'lucide-react';
import { getDocuments, getDocumentCategories, createDocument, deleteDocument } from '@/api/documents';
import { getEmployees } from '@/api/employees';
import { uploadFile } from '@/api/portal';
import { AttachmentLink } from '@/components/ui/AttachmentPreview';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { useAuth } from '@/context/AuthContext';
import { useConfirm } from '@/lib/confirm';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { formatDate, getErrorMessage, todayLocalDate } from '@/lib/utils';

/** Both `/employees` and `/documents` clamp `per_page` to 100 server-side, so a
 *  page is asked for and paged through rather than pretending 200 arrives. */
const PER_PAGE = 20;
const ALLOWED_UPLOAD_ACCEPT = '.jpg,.jpeg,.png,.gif,.webp,.pdf,.doc,.docx,.xls,.xlsx';
const MAX_UPLOAD_BYTES = 10 * 1024 * 1024;

type DocsTarget =
  | { type: 'company' }
  | { type: 'employee'; id: string; name: string; number: string };

function utcDayMillis(ymd: string): number | null {
  const parts = ymd.split('-');
  if (parts.length !== 3) return null;
  const [y, m, d] = parts.map(Number);
  if (!y || !m || !d) return null;
  return Date.UTC(y, m - 1, d);
}

function daysBetweenYmd(fromYmd: string, toYmd: string): number | null {
  const fromMs = utcDayMillis(fromYmd);
  const toMs = utcDayMillis(toYmd);
  if (fromMs === null || toMs === null) return null;
  return Math.round((toMs - fromMs) / 86_400_000);
}

function getStatusColor(status: string, expiryDate: string | null) {
  const today = todayLocalDate();
  if (expiryDate && expiryDate < today) {
    return 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300';
  }
  if (status === 'expired') {
    return 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300';
  }
  if (status === 'archived') {
    return 'bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-300';
  }
  if (expiryDate) {
    const daysUntil = daysBetweenYmd(today, expiryDate);
    if (daysUntil !== null && daysUntil <= 30) {
      return 'bg-amber-50 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300';
    }
  }
  return 'bg-green-50 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300';
}

function getStatusLabel(
  t: (key: string, opts?: Record<string, unknown>) => string,
  status: string,
  expiryDate: string | null,
) {
  const today = todayLocalDate();
  if (expiryDate && expiryDate < today) return t('enums.docStatus.expired');
  if (status === 'expired') return t('enums.docStatus.expired');
  if (status === 'archived') return t('enums.docStatus.archived');
  if (expiryDate) {
    const daysUntil = daysBetweenYmd(today, expiryDate);
    if (daysUntil !== null && daysUntil <= 30) {
      return t('enums.docStatus.expiring', { days: daysUntil });
    }
  }
  return t('enums.docStatus.active');
}

export function DocumentList() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const { can, canAny } = usePermissions();
  const canManageDocs = can('manage_documents');
  const canMarkConfidential = canAny(['manage_employees', 'view_payroll']);
  const companyId = user?.company_id ?? null;
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [page, setPage] = useState(1);
  const [selectedTarget, setSelectedTarget] = useState<DocsTarget | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  useEffect(() => {
    if (!search) {
      setDebouncedSearch('');
      return;
    }
    const timer = window.setTimeout(() => setDebouncedSearch(search), 250);
    return () => window.clearTimeout(timer);
  }, [search]);

  // `companyId` belongs in the key: the bare `['employees-select']` served the
  // previous tenant's roster after a company switch.
  const {
    data: employees,
    isLoading: loadingEmployees,
    isError: employeesError,
    error: employeesErrorObj,
    refetch: refetchEmployees,
  } = useQuery({
    queryKey: ['employees-select', companyId, debouncedSearch, page],
    queryFn: () => getEmployees({ search: debouncedSearch || undefined, page, per_page: PER_PAGE }),
    placeholderData: keepPreviousData,
  });

  const companyDocsSummary = useQuery({
    queryKey: ['documents', companyId, 'company-wide-summary'],
    queryFn: () => getDocuments({ page: 1, per_page: 100 }),
  });

  const companyWideCount = useMemo(
    () => (companyDocsSummary.data?.data ?? []).filter((d) => !d.employee_id).length,
    [companyDocsSummary.data],
  );

  const employeeList = employees?.data ?? [];
  const employeeTotal = employees?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(employeeTotal / PER_PAGE));

  return (
    <div>
      <PageHeader
        title={t('documents.title')}
        actions={
          canManageDocs ? (
            <button
              type="button"
              onClick={() => setShowCreate(true)}
              className="btn-primary w-full sm:w-auto"
            >
              <Plus className="w-4 h-4" />
              {t('documents.add')}
            </button>
          ) : undefined
        }
      />

      {/* Company-Wide Documents card */}
      <button
        type="button"
        onClick={() => setSelectedTarget({ type: 'company' })}
        className="w-full mb-6 bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 p-5 text-left hover:border-gray-300 dark:hover:border-slate-700 hover:shadow-md transition-all group"
      >
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3.5 min-w-0">
            <div className="w-10 h-10 rounded-xl bg-indigo-50 dark:bg-indigo-500/15 flex items-center justify-center text-indigo-600 dark:text-indigo-300 shrink-0">
              <Building2 className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="font-semibold text-sm text-gray-900 dark:text-slate-100">
                {t('documents.companyWideTitle')}
              </div>
              <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">
                {t('documents.companyWideSubtitle')}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <span className="text-xs font-medium px-2.5 py-1 rounded-full bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-200 tabular-nums">
              {t('documents.count', { count: companyWideCount })}
            </span>
            <ChevronRight className="w-4 h-4 text-gray-400 dark:text-slate-500 group-hover:text-gray-700 dark:group-hover:text-slate-200 transition-colors" />
          </div>
        </div>
      </button>

      {/* Search */}
      <div className="relative mb-6 max-w-md">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-slate-500 pointer-events-none" />
        <input
          type="text"
          placeholder={t('documents.searchPlaceholder')}
          aria-label={t('documents.searchPlaceholder')}
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(1); }}
          className="form-input !pl-10 !pr-11 !py-2.5"
        />
        {search && (
          <button
            type="button"
            onClick={() => { setSearch(''); setPage(1); }}
            aria-label={t('common.clear')}
            title={t('common.clear')}
            className="absolute right-1 top-1/2 -translate-y-1/2 min-h-10 min-w-10 flex items-center justify-center rounded-md text-gray-400 dark:text-slate-500 hover:text-gray-700 dark:hover:text-slate-200"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Employee List */}
      {loadingEmployees ? (
        <div className="flex items-center justify-center py-12">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black dark:border-slate-200" />
        </div>
      ) : employeesError ? (
        <EmptyState
          icon={AlertTriangle}
          title={t('documents.loadFailed')}
          description={getErrorMessage(employeesErrorObj, t('documents.loadFailed'))}
          action={
            <button type="button" onClick={() => refetchEmployees()} className="btn-primary">
              {t('common.retry')}
            </button>
          }
        />
      ) : employeeList.length === 0 ? (
        <div className="bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 py-12 text-center text-gray-400 dark:text-slate-500">
          <User className="w-8 h-8 mx-auto mb-2 text-gray-300 dark:text-slate-600" />
          <p className="text-sm">{search ? t('employees.noMatch') : t('employees.empty')}</p>
          {search && (
            <button
              type="button"
              onClick={() => { setSearch(''); setPage(1); }}
              className="btn-secondary mt-3"
            >
              {t('reports.clearSearch')}
            </button>
          )}
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {employeeList.map((emp) => (
            <button
              type="button"
              key={emp.id}
              onClick={() => setSelectedTarget({ type: 'employee', id: emp.id, name: emp.full_name, number: emp.employee_number })}
              className="bg-white dark:bg-slate-900 rounded-xl border border-gray-200 dark:border-slate-800 p-5 text-left hover:border-gray-300 dark:hover:border-slate-700 hover:shadow-md transition-all group"
            >
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-gray-900 dark:text-slate-100 font-semibold text-sm group-hover:bg-gray-200 dark:group-hover:bg-slate-700 transition-colors shrink-0">
                  {emp.full_name.split(' ').map(n => n[0]).slice(0, 2).join('')}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-sm text-gray-900 dark:text-slate-100 truncate">{emp.full_name}</div>
                  <div className="text-xs text-gray-500 dark:text-slate-400">{emp.employee_number}</div>
                </div>
                <FileText className="w-4 h-4 text-gray-300 dark:text-slate-600 group-hover:text-gray-500 dark:group-hover:text-slate-300 transition-colors shrink-0" />
              </div>
              {emp.department && (
                <div className="text-xs text-gray-500 dark:text-slate-400 mt-2 truncate">{emp.department}</div>
              )}
            </button>
          ))}
        </div>
      )}

      {/* Paging — the roster is served a page at a time, so the count and the
          controls are what tell an admin the tail exists at all. */}
      {employeeTotal > PER_PAGE && (
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mt-6 text-sm text-gray-500 dark:text-slate-400">
          <span>
            {t('common.showing', { from: (page - 1) * PER_PAGE + 1, to: Math.min(page * PER_PAGE, employeeTotal), total: employeeTotal })}
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page <= 1}
              className="min-h-10 flex items-center gap-1 px-3 py-1.5 rounded-lg border border-gray-200 dark:border-slate-700 disabled:opacity-40 hover:bg-gray-50 dark:hover:bg-slate-800"
            >
              <ChevronLeft className="w-4 h-4" /> {t('common.previous')}
            </button>
            <button
              type="button"
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              disabled={page >= totalPages}
              className="min-h-10 flex items-center gap-1 px-3 py-1.5 rounded-lg border border-gray-200 dark:border-slate-700 disabled:opacity-40 hover:bg-gray-50 dark:hover:bg-slate-800"
            >
              {t('common.next')} <ChevronRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Documents Modal (Company-Wide or Employee) */}
      {selectedTarget && (
        <EmployeeDocumentsModal
          target={selectedTarget}
          canManageDocs={canManageDocs}
          canMarkConfidential={canMarkConfidential}
          onClose={() => setSelectedTarget(null)}
        />
      )}

      {/* Create Document Modal */}
      {showCreate && canManageDocs && (
        <CreateDocumentModal
          canMarkConfidential={canMarkConfidential}
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            setShowCreate(false);
            queryClient.invalidateQueries({ queryKey: ['documents'] });
          }}
        />
      )}
    </div>
  );
}

function EmployeeDocumentsModal({
  target,
  canManageDocs,
  canMarkConfidential,
  onClose,
}: {
  target: DocsTarget;
  canManageDocs: boolean;
  canMarkConfidential: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [page, setPage] = useState(1);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const isCompanyWide = target.type === 'company';

  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['documents', isCompanyWide ? { companyWide: true } : { employee_id: target.id, page }],
    queryFn: () =>
      isCompanyWide
        ? getDocuments({ page: 1, per_page: 100 })
        : getDocuments({ employee_id: target.id, page, per_page: PER_PAGE }),
    placeholderData: keepPreviousData,
  });

  const rows = useMemo(() => {
    const raw = data?.data ?? [];
    return isCompanyWide ? raw.filter((d) => !d.employee_id) : raw;
  }, [data?.data, isCompanyWide]);

  const totalCount = isCompanyWide ? rows.length : (data?.total ?? 0);
  const totalPages = isCompanyWide ? 1 : Math.max(1, Math.ceil(totalCount / PER_PAGE));

  const { data: categories } = useQuery({
    queryKey: ['document-categories'],
    queryFn: getDocumentCategories,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteDocument,
    onMutate: (id) => setDeletingId(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['documents'] });
      toast.success(t('documents.deleteSuccess'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('documents.deleteFailed'))),
    onSettled: () => setDeletingId(null),
  });

  const categoryMap = new Map(categories?.map(c => [c.id, c.name]) ?? []);

  const dialogTitle = isCompanyWide
    ? t('documents.companyWideTitle')
    : `${target.name} (${target.number})`;

  const today = todayLocalDate();

  return (
    <>
      <Modal
        open
        onClose={onClose}
        title={dialogTitle}
        maxWidth="max-w-4xl"
        footer={
          <div className="flex flex-wrap items-center justify-between gap-3">
            <span className="text-sm text-gray-500 dark:text-slate-400">
              {totalCount > 0 ? t('documents.count', { count: totalCount }) : ''}
            </span>
            <div className="flex items-center gap-2">
              {!isCompanyWide && totalCount > PER_PAGE && (
                <div className="flex items-center gap-2 mr-2">
                  <button
                    type="button"
                    onClick={() => setPage((p) => Math.max(1, p - 1))}
                    disabled={page <= 1}
                    className="min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg border border-gray-200 dark:border-slate-700 disabled:opacity-40 hover:bg-white dark:hover:bg-slate-800"
                    aria-label={t('common.previousPage')}
                  >
                    <ChevronLeft className="w-4 h-4" />
                  </button>
                  <span className="text-sm text-gray-500 dark:text-slate-400">{t('common.pageOf', { page, total: totalPages })}</span>
                  <button
                    type="button"
                    onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                    disabled={page >= totalPages}
                    className="min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg border border-gray-200 dark:border-slate-700 disabled:opacity-40 hover:bg-white dark:hover:bg-slate-800"
                    aria-label={t('common.nextPage')}
                  >
                    <ChevronRight className="w-4 h-4" />
                  </button>
                </div>
              )}
              {canManageDocs && (
                <button
                  type="button"
                  onClick={() => setShowCreate(true)}
                  className="btn-primary !py-2 !min-h-9"
                >
                  <Plus className="w-3.5 h-3.5" />
                  {t('documents.add')}
                </button>
              )}
            </div>
          </div>
        }
      >
        <div>
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <div className="spinner" />
            </div>
          ) : isError ? (
            <EmptyState
              icon={AlertTriangle}
              title={t('documents.loadFailed')}
              description={getErrorMessage(error, t('documents.loadFailed'))}
              action={
                <button type="button" onClick={() => refetch()} className="btn-primary">
                  {t('common.retry')}
                </button>
              }
            />
          ) : rows.length === 0 ? (
            <div className="text-center py-12 text-gray-400 dark:text-slate-500">
              <FileText className="w-8 h-8 mx-auto mb-2 opacity-40" />
              <p className="text-sm">{isCompanyWide ? t('documents.emptyCompanyWide') : t('documents.emptyForEmployee')}</p>
            </div>
          ) : (
            <div className="overflow-x-auto -mx-6">
              <table className="w-full">
                <thead className="bg-gray-50 dark:bg-slate-900/60 border-b border-gray-200 dark:border-slate-800">
                  <tr>
                    <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase">{t('documents.cols.title')}</th>
                    <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase">{t('documents.cols.category')}</th>
                    <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase">{t('documents.cols.file')}</th>
                    <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('documents.cols.issueDate')}</th>
                    <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('documents.cols.expiry')}</th>
                    <th className="text-center px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase">{t('common.status')}</th>
                    {canManageDocs && <th className="text-center px-6 py-3 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase w-16" />}
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                  {rows.map((doc) => (
                    <tr key={doc.id} className="hover:bg-gray-50 dark:hover:bg-slate-800/50 transition-colors">
                      <td className="px-6 py-3">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm text-gray-900 dark:text-slate-100">{doc.title}</span>
                          {doc.is_confidential && (
                            <Lock className="w-3.5 h-3.5 text-amber-500 shrink-0" aria-label={t('documents.confidential')} />
                          )}
                        </div>
                        {doc.description && (
                          <div className="text-xs text-gray-500 dark:text-slate-400 mt-0.5 truncate max-w-xs">{doc.description}</div>
                        )}
                      </td>
                      <td className="px-6 py-3 text-sm text-gray-600 dark:text-slate-300">
                        {doc.category_id ? categoryMap.get(doc.category_id) ?? '-' : '-'}
                      </td>
                      <td className="px-6 py-3">
                        <AttachmentLink
                          url={doc.file_url}
                          className="text-sm text-gray-900 dark:text-slate-100 hover:text-black dark:hover:text-white hover:underline"
                        >
                          {doc.file_name}
                        </AttachmentLink>
                        {doc.file_size && (
                          <div className="text-xs text-gray-500 dark:text-slate-400">{(doc.file_size / 1024).toFixed(0)} KB</div>
                        )}
                      </td>
                      <td className="px-6 py-3 text-sm text-gray-600 dark:text-slate-300 whitespace-nowrap">
                        {doc.issue_date ? formatDate(doc.issue_date) : '-'}
                      </td>
                      <td className="px-6 py-3 text-sm text-gray-600 dark:text-slate-300 whitespace-nowrap">
                        {doc.expiry_date ? (
                          <span className="inline-flex items-center gap-1">
                            {doc.expiry_date < today && <AlertTriangle className="w-3.5 h-3.5 text-red-500 dark:text-red-400 shrink-0" />}
                            {formatDate(doc.expiry_date)}
                          </span>
                        ) : '-'}
                      </td>
                      <td className="px-6 py-3 text-center">
                        <span className={`inline-flex px-2 py-1 rounded-full text-xs font-medium whitespace-nowrap ${getStatusColor(doc.status, doc.expiry_date)}`}>
                          {getStatusLabel(t, doc.status, doc.expiry_date)}
                        </span>
                      </td>
                      {canManageDocs && (
                        <td className="px-6 py-3 text-center">
                          <button
                            type="button"
                            disabled={deletingId === doc.id && deleteMutation.isPending}
                            onClick={async () => {
                              const ok = await confirmDialog({
                                description: t('documents.deleteConfirm'),
                                variant: 'danger',
                              });
                              if (ok) deleteMutation.mutate(doc.id);
                            }}
                            aria-label={t('common.deleteName', { name: doc.title })}
                            title={t('common.deleteName', { name: doc.title })}
                            className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center text-gray-400 dark:text-slate-500 hover:text-red-600 dark:hover:text-red-400 rounded-lg hover:bg-red-50 dark:hover:bg-red-500/15 disabled:opacity-50 transition-colors"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </Modal>

      {/* Nested Create Modal */}
      {showCreate && canManageDocs && (
        <CreateDocumentModal
          defaultEmployeeId={isCompanyWide ? '' : target.id}
          defaultEmployeeLabel={isCompanyWide ? undefined : `${target.name} (${target.number})`}
          canMarkConfidential={canMarkConfidential}
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            setShowCreate(false);
            queryClient.invalidateQueries({ queryKey: ['documents'] });
          }}
        />
      )}
    </>
  );
}

function CreateDocumentModal({
  defaultEmployeeId,
  defaultEmployeeLabel,
  canMarkConfidential,
  onClose,
  onCreated,
}: {
  defaultEmployeeId?: string;
  defaultEmployeeLabel?: string;
  canMarkConfidential: boolean;
  onClose: () => void;
  onCreated: () => void;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const formId = useId();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data: categories } = useQuery({
    queryKey: ['document-categories'],
    queryFn: getDocumentCategories,
  });

  const [form, setForm] = useState({
    title: '',
    description: '',
    file_name: '',
    file_url: '',
    category_id: '',
    issue_date: '',
    expiry_date: '',
    is_confidential: false,
  });
  const [employeeId, setEmployeeId] = useState(defaultEmployeeId ?? '');
  const [inputMode, setInputMode] = useState<'upload' | 'url'>('upload');
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: createDocument,
    onSuccess: () => {
      toast.success(t('documents.createSuccess'));
      onCreated();
    },
    onError: (err) => setError(getErrorMessage(err, t('documents.createFailed'))),
  });

  const handleFileUpload = async (file: File) => {
    setError('');
    if (file.size > MAX_UPLOAD_BYTES) {
      setError(t('approvals.fileTooLarge'));
      return;
    }
    setUploading(true);
    try {
      const uploaded = await uploadFile(file);
      setForm((f) => ({
        ...f,
        file_name: uploaded.file_name || f.file_name,
        file_url: uploaded.url,
        title: f.title || uploaded.file_name.replace(/\.[^.]+$/, ''),
      }));
    } catch (err) {
      setError(getErrorMessage(err, t('documents.uploadFailed')));
    } finally {
      setUploading(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const title = form.title.trim();
    const fileName = form.file_name.trim();
    const fileUrl = form.file_url.trim();
    if (!title || !fileName || !fileUrl) {
      setError(t('documents.requiredFields'));
      return;
    }
    if (form.issue_date && form.expiry_date && form.expiry_date < form.issue_date) {
      setError(t('documents.expiryBeforeIssue'));
      return;
    }
    setError('');
    mutation.mutate({
      ...form,
      title,
      description: form.description.trim() || undefined,
      file_name: fileName,
      file_url: fileUrl,
      is_confidential: canMarkConfidential ? form.is_confidential : false,
      employee_id: employeeId || undefined,
      category_id: form.category_id || undefined,
      issue_date: form.issue_date || undefined,
      expiry_date: form.expiry_date || undefined,
    });
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={t('documents.add')}
      maxWidth="max-w-xl"
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={mutation.isPending || uploading}
            className="btn-primary"
          >
            {mutation.isPending ? t('common.creating') : t('documents.create')}
          </button>
        </div>
      }
    >
      <form id={formId} onSubmit={handleSubmit} className="space-y-5">
        {error && (
          <div role="alert" className="text-sm text-red-600 dark:text-red-300 bg-red-50 dark:bg-red-500/10 p-4 rounded-xl border border-red-100 dark:border-red-500/25">
            {error}
          </div>
        )}

        <section className="rounded-xl border border-gray-100 dark:border-slate-800 p-4 space-y-4">
          <h3 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider">{t('documents.details')}</h3>
          <FormField label={t('documents.cols.title')} required>
            {(aria) => (
              <input
                {...aria}
                type="text"
                value={form.title}
                onChange={(e) => setForm(f => ({ ...f, title: e.target.value }))}
                className="form-input"
                placeholder={t('documents.titlePlaceholder')}
              />
            )}
          </FormField>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <FormField label={t('common.employee')}>
              {(aria) => (
                <EmployeePicker
                  id={aria.id}
                  value={employeeId}
                  onChange={(id) => setEmployeeId(id)}
                  isActive={null}
                  initialLabel={defaultEmployeeLabel}
                  placeholder={t('documents.companyWide')}
                />
              )}
            </FormField>
            <FormField label={t('documents.cols.category')}>
              {(aria) => (
                <select
                  {...aria}
                  value={form.category_id}
                  onChange={(e) => setForm(f => ({ ...f, category_id: e.target.value }))}
                  className="form-input"
                >
                  <option value="">{t('documents.noCategory')}</option>
                  {categories?.map(c => (
                    <option key={c.id} value={c.id}>{c.name}</option>
                  ))}
                </select>
              )}
            </FormField>
          </div>

          <FormField label={t('common.description')}>
            {(aria) => (
              <textarea
                {...aria}
                value={form.description}
                onChange={(e) => setForm(f => ({ ...f, description: e.target.value }))}
                className="form-input"
                rows={2}
              />
            )}
          </FormField>
        </section>

        <section className="rounded-xl border border-gray-100 dark:border-slate-800 p-4 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider">{t('documents.fileInfo')}</h3>
            <div className="flex gap-1 bg-gray-100 dark:bg-slate-800 p-0.5 rounded-lg">
              <button
                type="button"
                onClick={() => setInputMode('upload')}
                className={`flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-medium transition-colors ${
                  inputMode === 'upload'
                    ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                    : 'text-gray-500 dark:text-slate-400'
                }`}
              >
                <Upload className="w-3 h-3" />
                {t('common.upload')}
              </button>
              <button
                type="button"
                onClick={() => setInputMode('url')}
                className={`flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-medium transition-colors ${
                  inputMode === 'url'
                    ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                    : 'text-gray-500 dark:text-slate-400'
                }`}
              >
                <LinkIcon className="w-3 h-3" />
                {t('documents.enterUrl')}
              </button>
            </div>
          </div>

          {inputMode === 'upload' ? (
            <div className="space-y-3">
              <input
                ref={fileInputRef}
                type="file"
                accept={ALLOWED_UPLOAD_ACCEPT}
                className="sr-only"
                onChange={(e) => {
                  const picked = e.target.files?.[0];
                  e.currentTarget.value = '';
                  if (picked) void handleFileUpload(picked);
                }}
              />
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  disabled={uploading}
                  className="btn-secondary"
                >
                  {uploading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Upload className="w-4 h-4" />}
                  {uploading ? t('common.uploading') : t('approvals.uploadFile')}
                </button>
                {form.file_url && (
                  <button
                    type="button"
                    onClick={() => setForm((f) => ({ ...f, file_url: '', file_name: '' }))}
                    aria-label={t('common.remove')}
                    className="min-h-10 min-w-10 flex items-center justify-center rounded-lg text-gray-400 dark:text-slate-500 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15"
                  >
                    <X className="w-4 h-4" />
                  </button>
                )}
              </div>
              {form.file_name ? (
                <p className="text-sm text-gray-700 dark:text-slate-300 break-all">{form.file_name}</p>
              ) : (
                <p className="text-xs text-gray-500 dark:text-slate-400">{t('approvals.uploadHint')}</p>
              )}
              {form.file_url && (
                <FormField label={t('documents.fileName')} required>
                  {(aria) => (
                    <input
                      {...aria}
                      type="text"
                      value={form.file_name}
                      onChange={(e) => setForm(f => ({ ...f, file_name: e.target.value }))}
                      className="form-input"
                    />
                  )}
                </FormField>
              )}
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <FormField label={t('documents.fileName')} required>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.file_name}
                    onChange={(e) => setForm(f => ({ ...f, file_name: e.target.value }))}
                    className="form-input"
                    // i18n-ok: filename example, not prose copy
                    placeholder="document.pdf"
                  />
                )}
              </FormField>
              <FormField label={t('documents.fileUrl')} required hint={t('documents.fileUrlHint')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.file_url}
                    onChange={(e) => setForm(f => ({ ...f, file_url: e.target.value }))}
                    className="form-input"
                    placeholder="/api/uploads/document.pdf"
                  />
                )}
              </FormField>
            </div>
          )}

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <FormField label={t('documents.cols.issueDate')}>
              {(aria) => (
                <input
                  {...aria}
                  type="date"
                  value={form.issue_date}
                  onChange={(e) => setForm(f => ({ ...f, issue_date: e.target.value }))}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('documents.expiryDate')}>
              {(aria) => (
                <input
                  {...aria}
                  type="date"
                  value={form.expiry_date}
                  min={form.issue_date || undefined}
                  onChange={(e) => setForm(f => ({ ...f, expiry_date: e.target.value }))}
                  className="form-input"
                />
              )}
            </FormField>
          </div>

          {canMarkConfidential && (
            <label className="flex items-center gap-2.5 cursor-pointer pt-1">
              <input
                type="checkbox"
                checked={form.is_confidential}
                onChange={(e) => setForm(f => ({ ...f, is_confidential: e.target.checked }))}
                className="rounded border-gray-300 dark:border-slate-700 w-4 h-4 text-[var(--accent-1)] focus:ring-[var(--ring)]"
              />
              <span className="text-sm font-medium text-gray-700 dark:text-slate-200">{t('documents.confidentialDoc')}</span>
            </label>
          )}
        </section>
      </form>
    </Modal>
  );
}
