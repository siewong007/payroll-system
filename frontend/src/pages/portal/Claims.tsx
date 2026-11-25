import { useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Send, Trash2, Receipt, Upload, X, FileText, Image, Paperclip, ExternalLink, AlertTriangle } from 'lucide-react';
import { listMyClaims, createClaim, cancelClaim, deleteClaim, submitClaim, uploadFile } from '@/api/claims';
import { useAuth } from '@/context/AuthContext';
import { AttachmentLink, AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { useConfirm } from '@/lib/confirm';
import { runBulk, summarizeBulkFailure } from '@/lib/bulk';
import { formatMYR, formatDate, getErrorMessage, todayLocalDate } from '@/lib/utils';
import type { Claim } from '@/types';
import i18n from '@/i18n';

const ALLOWED_CLAIM_EXT_RE = /\.(jpg|jpeg|png|gif|webp|pdf|doc|docx|xls|xlsx)$/i;

const claimCategoryLabel = (category: string) =>
  i18n.t(`enums.claimCategory.${category}`, { defaultValue: category });

const STATUS_TABS = [
  { key: null, labelKey: 'common.all' },
  { key: 'draft', labelKey: 'enums.requestStatus.draft' },
  { key: 'pending', labelKey: 'enums.requestStatus.pending' },
  { key: 'approved', labelKey: 'enums.requestStatus.approved' },
  { key: 'rejected', labelKey: 'enums.requestStatus.rejected' },
  { key: 'cancelled', labelKey: 'enums.requestStatus.cancelled' },
  { key: 'processed', labelKey: 'enums.requestStatus.processed' },
] as const;

const CATEGORIES = [
  'Transport', 'Meals', 'Accommodation', 'Office Expenses',
  'Medical', 'Training', 'Entertainment', 'Other',
];

const canCancelClaim = (c: Claim) => ['pending', 'approved', 'rejected'].includes(c.status);
const canDeleteClaim = (c: Claim) => ['draft', 'cancelled'].includes(c.status);

export function Claims() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const hasEmployee = Boolean(user?.employee_id);
  const confirmDialog = useConfirm();
  const queryClient = useQueryClient();
  const [statusFilter, setStatusFilter] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [selectedClaimIds, setSelectedClaimIds] = useState<string[]>([]);
  const [bulkError, setBulkError] = useState('');

  // Fetch all claims once so every status tab counter stays accurate across filters.
  const { data: claims = [], isLoading, isError, error, refetch } = useQuery({
    queryKey: ['my-claims'],
    queryFn: () => listMyClaims(),
    enabled: hasEmployee,
  });

  const filteredClaims = useMemo(
    () => (statusFilter ? claims.filter((c) => c.status === statusFilter) : claims),
    [claims, statusFilter],
  );

  const submitMutation = useMutation({
    mutationFn: submitClaim,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-claims'] }),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteClaim,
    onSuccess: (_data, deletedId) => {
      setSelectedClaimIds((prev) => prev.filter((id) => id !== deletedId));
      queryClient.invalidateQueries({ queryKey: ['my-claims'] });
    },
  });

  const cancelMutation = useMutation({
    mutationFn: cancelClaim,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-claims'] }),
  });

  // Partial failure is an outcome, not an exception: the refetch runs whatever
  // happens and only the claims that failed stay selected, so a retry hits
  // exactly those. `Promise.all` used to short-circuit on the first rejection
  // and leave the successful rows showing a stale status with no error anywhere.
  const bulkCancelMutation = useMutation({
    mutationFn: (ids: string[]) => runBulk(ids, cancelClaim),
    onSuccess: (outcome) => {
      setSelectedClaimIds(outcome.failed.map((failure) => failure.id));
      setBulkError(summarizeBulkFailure(outcome, 'cancelled'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('portal.claims.bulkCancelFailed'))),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['my-claims'] }),
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => runBulk(ids, deleteClaim),
    onSuccess: (outcome) => {
      setSelectedClaimIds(outcome.failed.map((failure) => failure.id));
      setBulkError(summarizeBulkFailure(outcome, 'deleted'));
    },
    onError: (err: unknown) => setBulkError(getErrorMessage(err, t('portal.claims.bulkDeleteFailed'))),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['my-claims'] }),
  });

  const statusBadge = (status: string) => {
    const cls: Record<string, string> = {
      draft: 'badge-draft', pending: 'badge-pending', approved: 'badge-approved',
      rejected: 'badge-rejected', processed: 'badge-processed', cancelled: 'badge-cancelled',
    };
    return (
      <span className={`badge ${cls[status] || 'badge-draft'}`}>
        {t(`enums.requestStatus.${status}`, { defaultValue: status })}
      </span>
    );
  };

  const statusCount = (key: string) => claims.filter((c) => c.status === key).length;
  const selectedClaims = filteredClaims.filter((claim) => selectedClaimIds.includes(claim.id));
  const selectedCancelableClaimIds = selectedClaims.filter(canCancelClaim).map((claim) => claim.id);
  const selectedDeletableClaimIds = selectedClaims.filter(canDeleteClaim).map((claim) => claim.id);

  const columns: Column<Claim>[] = [
    {
      key: 'title',
      header: t('portal.claims.cols.expense'),
      sortValue: (c) => c.title.toLowerCase(),
      render: (c) => (
        <div className="min-w-0">
          <span className="font-semibold text-gray-900 dark:text-slate-100">{c.title}</span>
          {c.description && <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5 truncate max-w-xs">{c.description}</p>}
          {c.review_notes && (
            <p className="text-xs text-amber-600 dark:text-amber-400 mt-0.5 truncate max-w-xs">
              {c.review_notes}
            </p>
          )}
        </div>
      ),
    },
    {
      key: 'amount',
      header: t('common.amount'),
      align: 'right',
      sortValue: (c) => c.amount,
      render: (c) => <span className="font-bold tabular-nums text-gray-900 dark:text-slate-100">{formatMYR(c.amount)}</span>,
    },
    {
      key: 'category',
      header: t('portal.claims.cols.category'),
      sortValue: (c) => c.category ?? '',
      render: (c) => <span className="text-gray-600 dark:text-slate-300">{c.category ? claimCategoryLabel(c.category) : '—'}</span>,
    },
    {
      key: 'expense_date',
      header: t('common.date'),
      sortValue: (c) => c.expense_date,
      render: (c) => <span className="text-gray-600 dark:text-slate-300 tabular-nums">{formatDate(c.expense_date)}</span>,
    },
    {
      key: 'receipt',
      header: t('portal.claims.cols.receipt'),
      render: (c) => (
        c.receipt_url ? (
          c.receipt_url.startsWith('blob:') ? (
            <span className="inline-flex items-center gap-1 text-red-500 dark:text-red-400 text-sm">
              <Paperclip className="w-3 h-3" />
              <span className="truncate max-w-[100px]">{t('common.unavailable')}</span>
            </span>
          ) : (
            <span onClick={(e) => e.stopPropagation()}>
              <AttachmentLink
                url={c.receipt_url}
                className="inline-flex items-center gap-1 text-gray-900 dark:text-slate-100 hover:underline text-sm"
              >
                <Paperclip className="w-3 h-3" />
                <span className="truncate max-w-[100px]">{c.receipt_file_name || t('common.view')}</span>
                <ExternalLink className="w-3 h-3" />
              </AttachmentLink>
            </span>
          )
        ) : <span className="text-gray-300 dark:text-slate-600">—</span>
      ),
    },
    {
      key: 'status',
      header: t('common.status'),
      align: 'center',
      sortValue: (c) => c.status,
      render: (c) => statusBadge(c.status),
    },
  ];

  if (!hasEmployee) {
    return (
      <div className="card">
        <EmptyState icon={Receipt} title={t('attendance.scan.noEmployeeProfile')} />
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 dark:border-slate-200" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="card">
        <EmptyState
          icon={AlertTriangle}
          title={t('portal.claims.loadFailed')}
          description={getErrorMessage(error, t('portal.claims.loadFailed'))}
          action={
            <button type="button" onClick={() => void refetch()} className="btn-primary">
              {t('common.retry')}
            </button>
          }
        />
      </div>
    );
  }

  // Full-page create form
  if (showCreate) {
    return <CreateClaimForm onClose={() => setShowCreate(false)} />;
  }

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">{t('portal.claims.title')}</h1>
          <p className="page-subtitle">{t('portal.claims.subtitle')}</p>
        </div>
        <button type="button" onClick={() => setShowCreate(true)} className="btn-primary w-full sm:w-auto">
          <Plus className="w-4 h-4" /> {t('portal.claims.addExpense')}
        </button>
      </div>

      {/* Status Filter Tabs */}
      <div className="flex gap-2 flex-wrap">
        {STATUS_TABS.map((tab) => {
          const isActive = statusFilter === tab.key;
          const count = tab.key ? statusCount(tab.key) : claims.length;
          return (
            <button
              key={tab.key ?? 'all'}
              type="button"
              onClick={() => {
                setSelectedClaimIds([]);
                setBulkError('');
                setStatusFilter(tab.key);
              }}
              aria-pressed={isActive}
              className={`min-h-10 px-4 py-1.5 rounded-full text-sm font-medium transition-all-fast ${
                isActive
                  ? 'bg-gradient-to-r from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-sm'
                  : 'bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-300 border border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700 hover:text-gray-900 dark:hover:text-white'
              }`}
            >
              {t(tab.labelKey)} {count > 0 && <span className="ml-1 opacity-75 tabular-nums">{count}</span>}
            </button>
          );
        })}
      </div>

      {selectedClaimIds.length > 0 && (
        <div className="rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
            <span className="text-sm font-medium text-gray-700 dark:text-slate-200">{t('common.selected', { count: selectedClaimIds.length })}</span>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    description: t('portal.claims.bulkCancelConfirm', { count: selectedCancelableClaimIds.length }),
                    confirmLabel: t('common.cancelSelected'),
                    variant: 'warning',
                  });
                  if (ok) {
                    setBulkError('');
                    bulkCancelMutation.mutate(selectedCancelableClaimIds);
                  }
                }}
                disabled={selectedCancelableClaimIds.length === 0 || bulkCancelMutation.isPending}
                className="btn-secondary !py-2 text-sm disabled:opacity-50"
              >
                <X className="w-4 h-4" />
                {bulkCancelMutation.isPending ? t('common.cancelling') : t('common.cancelSelected')}
              </button>
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    description: t('portal.claims.bulkDeleteConfirm', { count: selectedDeletableClaimIds.length }),
                    variant: 'danger',
                  });
                  if (ok) {
                    setBulkError('');
                    bulkDeleteMutation.mutate(selectedDeletableClaimIds);
                  }
                }}
                disabled={selectedDeletableClaimIds.length === 0 || bulkDeleteMutation.isPending}
                className="btn-secondary !py-2 text-sm text-red-600 dark:text-red-400 hover:!bg-red-50 dark:hover:!bg-red-500/10 disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" />
                {bulkDeleteMutation.isPending ? t('common.deleting') : t('common.deleteSelected')}
              </button>
            </div>
          </div>
          {bulkError && (
            <div role="alert" className="px-4 pb-3 text-sm text-red-700 dark:text-red-300">{bulkError}</div>
          )}
        </div>
      )}

      <DataTable
        columns={columns}
        data={filteredClaims}
        perPage={10}
        emptyMessage={t('portal.claims.empty')}
        emptyIcon={<Receipt className="w-10 h-10 opacity-40" />}
        selectable
        selectedRowKeys={selectedClaimIds}
        onSelectedRowKeysChange={setSelectedClaimIds}
        rowKey={(claim) => claim.id}
        summaryTitle={(c) => t('portal.claims.summaryTitle', { title: c.title })}
        renderSummary={(c) => (
          <div className="space-y-5">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h3 className="font-semibold text-lg text-gray-900 dark:text-slate-100">{c.title}</h3>
                {c.category && (
                  <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{claimCategoryLabel(c.category)}</p>
                )}
              </div>
              {statusBadge(c.status)}
            </div>

            <div className="grid grid-cols-2 gap-4 bg-gray-50 dark:bg-slate-800/70 rounded-lg p-4">
              <div>
                <p className="text-xs text-gray-400 dark:text-slate-400 uppercase tracking-wide">{t('common.amount')}</p>
                <p className="text-sm font-bold tabular-nums text-gray-900 dark:text-slate-100 mt-0.5">{formatMYR(c.amount)}</p>
              </div>
              <div>
                <p className="text-xs text-gray-400 dark:text-slate-400 uppercase tracking-wide">{t('common.date')}</p>
                <p className="text-sm font-medium text-gray-900 dark:text-slate-100 mt-0.5">{formatDate(c.expense_date)}</p>
              </div>
            </div>

            {c.description && (
              <div>
                <p className="text-xs text-gray-400 dark:text-slate-400 uppercase tracking-wide">{t('common.description')}</p>
                <p className="text-sm text-gray-700 dark:text-slate-200 mt-0.5">{c.description}</p>
              </div>
            )}

            {c.receipt_url && !c.receipt_url.startsWith('blob:') && (
              <div>
                <p className="text-xs text-gray-400 dark:text-slate-400 uppercase tracking-wide mb-1">{t('portal.claims.cols.receipt')}</p>
                <AttachmentPreview url={c.receipt_url} name={c.receipt_file_name} />
              </div>
            )}

            {c.review_notes && (
              <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-100 dark:border-amber-500/30 rounded-lg p-4">
                <p className="text-xs text-amber-700 dark:text-amber-300 uppercase tracking-wide">{t('portal.claims.reviewNotes')}</p>
                <p className="text-sm font-medium text-amber-900 dark:text-amber-100 mt-0.5">{c.review_notes}</p>
              </div>
            )}
          </div>
        )}
        renderSummaryFooter={(c, close) =>
          c.status === 'draft' || canCancelClaim(c) || canDeleteClaim(c) ? (
            <div className="flex flex-wrap items-center justify-end gap-2">
              {c.status === 'draft' && (
                <button
                  type="button"
                  onClick={() => submitMutation.mutate(c.id, { onSuccess: close })}
                  disabled={submitMutation.isPending}
                  className="btn-primary !py-2 text-sm"
                >
                  <Send className="w-4 h-4" />
                  {t('portal.claims.submitForApproval')}
                </button>
              )}
              {canCancelClaim(c) && (
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('portal.claims.cancelConfirm'),
                      confirmLabel: t('portal.claims.cancelClaim'),
                      variant: 'warning',
                    });
                    if (ok) cancelMutation.mutate(c.id, { onSuccess: close });
                  }}
                  disabled={cancelMutation.isPending}
                  className="btn-secondary !py-2 text-sm text-red-600 dark:text-red-400"
                >
                  <X className="w-4 h-4" />
                  {t('portal.claims.cancelClaim')}
                </button>
              )}
              {canDeleteClaim(c) && (
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: c.status === 'cancelled' ? t('portal.claims.deletePermanentlyConfirm') : t('portal.claims.deleteConfirm'),
                      variant: 'danger',
                    });
                    if (ok) deleteMutation.mutate(c.id, { onSuccess: close });
                  }}
                  disabled={deleteMutation.isPending}
                  className="btn-primary !py-2 text-sm !bg-none !bg-red-600 hover:!bg-red-700"
                >
                  <Trash2 className="w-4 h-4" />
                  {c.status === 'cancelled' ? t('common.deletePermanently') : t('common.delete')}
                </button>
              )}
            </div>
          ) : null
        }
        renderActions={(c) => (
          <div className="flex items-center justify-center gap-1">
            {c.status === 'draft' && (
              <>
                <button
                  type="button"
                  onClick={() => submitMutation.mutate(c.id)}
                  aria-label={t('portal.claims.submitForApproval')}
                  title={t('portal.claims.submitForApproval')}
                  className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-900 dark:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg transition-all-fast"
                >
                  <Send className="w-4 h-4" />
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    const ok = await confirmDialog({
                      description: t('portal.claims.deleteConfirm'),
                      variant: 'danger',
                    });
                    if (ok) deleteMutation.mutate(c.id);
                  }}
                  aria-label={t('common.delete')}
                  title={t('common.delete')}
                  className="min-h-10 min-w-10 inline-flex items-center justify-center text-red-500 dark:text-red-400 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-lg transition-all-fast"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </>
            )}
            {canCancelClaim(c) && (
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    description: t('portal.claims.cancelConfirm'),
                    confirmLabel: t('portal.claims.cancelClaim'),
                    variant: 'warning',
                  });
                  if (ok) cancelMutation.mutate(c.id);
                }}
                aria-label={t('portal.claims.cancelClaim')}
                title={t('portal.claims.cancelClaim')}
                className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-lg transition-all-fast"
              >
                <X className="w-4 h-4" />
              </button>
            )}
            {c.status === 'cancelled' && (
              <button
                type="button"
                onClick={async () => {
                  const ok = await confirmDialog({
                    description: t('portal.claims.deletePermanentlyConfirm'),
                    variant: 'danger',
                  });
                  if (ok) deleteMutation.mutate(c.id);
                }}
                aria-label={t('common.deletePermanently')}
                title={t('common.deletePermanently')}
                className="min-h-10 min-w-10 inline-flex items-center justify-center text-red-500 dark:text-red-400 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-lg transition-all-fast"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            )}
          </div>
        )}
      />
    </div>
  );
}

/* ───────────── Full-page Create Claim Form ───────────── */
function CreateClaimForm({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [form, setForm] = useState({
    title: '', description: '', amount: '', category: '',
    expense_date: todayLocalDate(),
    receipt_url: '', receipt_file_name: '',
  });
  const [error, setError] = useState('');
  const [uploading, setUploading] = useState(false);
  const [uploadPreview, setUploadPreview] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: createClaim,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['my-claims'] });
      onClose();
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('portal.claims.createFailed')));
    },
  });

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    if (!ALLOWED_CLAIM_EXT_RE.test(file.name)) {
      setError(t('portal.claims.invalidFileType'));
      if (fileInputRef.current) fileInputRef.current.value = '';
      return;
    }

    // Client-side size check (10 MB)
    if (file.size > 10 * 1024 * 1024) {
      setError(t('portal.claims.fileTooLarge'));
      if (fileInputRef.current) fileInputRef.current.value = '';
      return;
    }

    setError('');
    setUploading(true);

    // Show preview for images
    if (file.type.startsWith('image/')) {
      const reader = new FileReader();
      reader.onload = (ev) => setUploadPreview(ev.target?.result as string);
      reader.readAsDataURL(file);
    } else {
      setUploadPreview(null);
    }

    try {
      const result = await uploadFile(file);
      setForm((prev) => ({ ...prev, receipt_url: result.url, receipt_file_name: result.file_name }));
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('portal.claims.uploadFailed')));
      setUploadPreview(null);
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  const handleRemoveFile = () => {
    setForm((prev) => ({ ...prev, receipt_url: '', receipt_file_name: '' }));
    setUploadPreview(null);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleSubmit = () => {
    setError('');
    if (!form.title.trim() || !form.amount || !form.expense_date) {
      setError(t('portal.claims.requiredFields'));
      return;
    }
    const parsedAmount = Number(form.amount);
    if (!Number.isFinite(parsedAmount) || parsedAmount <= 0) {
      setError(t('portal.claims.amountPositive'));
      return;
    }
    if (!form.receipt_url) {
      setError(t('portal.claims.receiptRequired'));
      return;
    }
    mutation.mutate({
      title: form.title.trim(),
      description: form.description || undefined,
      amount: Math.round(parsedAmount * 100),
      category: form.category || undefined,
      expense_date: form.expense_date,
      receipt_url: form.receipt_url || undefined,
      receipt_file_name: form.receipt_file_name || undefined,
    });
  };

  return (
    <div className="space-y-6">
      {/* Back button + title */}
      <div className="flex items-center gap-4">
        <button type="button" onClick={onClose} aria-label={t('common.goBack')} className="btn-secondary !px-3 !py-2">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="page-header">
          <h1 className="page-title">{t('portal.claims.createTitle')}</h1>
          <p className="page-subtitle">{t('portal.claims.createSubtitle')}</p>
        </div>
      </div>

      {error && (
        <div role="alert" className="p-4 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/30">
          {error}
        </div>
      )}

      <div className="card !p-0 divide-y divide-gray-100 dark:divide-slate-800">
        {/* Section 1: Basic Info */}
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">{t('portal.claims.sections.details')}</span>
          </div>
          <div className="space-y-5 max-w-2xl">
            <div>
              <label className="form-label" htmlFor="claim-title">{t('common.title')} *</label>
              <input
                id="claim-title"
                value={form.title}
                onChange={(e) => setForm((prev) => ({ ...prev, title: e.target.value }))}
                className="form-input"
                placeholder={t('portal.claims.titlePlaceholder')}
              />
            </div>
            <div>
              <label className="form-label" htmlFor="claim-desc">{t('common.description')}</label>
              <textarea
                id="claim-desc"
                value={form.description}
                onChange={(e) => setForm((prev) => ({ ...prev, description: e.target.value }))}
                rows={2}
                className="form-input"
                placeholder={t('portal.claims.descriptionPlaceholder')}
              />
            </div>
          </div>
        </div>

        {/* Section 2: Amount & Category */}
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">{t('portal.claims.sections.amount')}</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-2xl">
            <div>
              <label className="form-label" htmlFor="claim-amount">{t('portal.claims.amountMyr')} *</label>
              <input
                id="claim-amount"
                type="number" step="0.01" min="0.01"
                value={form.amount}
                onChange={(e) => setForm((prev) => ({ ...prev, amount: e.target.value }))}
                className="form-input" placeholder="0.00"
              />
            </div>
            <div>
              <label className="form-label" htmlFor="claim-category">{t('portal.claims.cols.category')}</label>
              <select
                id="claim-category"
                value={form.category}
                onChange={(e) => setForm((prev) => ({ ...prev, category: e.target.value }))}
                className="form-input"
              >
                <option value="">{t('common.select')}</option>
                {CATEGORIES.map((c) => <option key={c} value={c}>{claimCategoryLabel(c)}</option>)}
              </select>
            </div>
            <div>
              <label className="form-label" htmlFor="claim-date">{t('portal.claims.expenseDate')} *</label>
              <input
                id="claim-date"
                type="date"
                value={form.expense_date}
                onChange={(e) => setForm((prev) => ({ ...prev, expense_date: e.target.value }))}
                className="form-input"
              />
            </div>
          </div>
        </div>

        {/* Section 3: Receipt */}
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">3</span>
            <span className="section-title">{t('portal.claims.sections.receipt')} *</span>
          </div>
          <div className="max-w-2xl">
            <label className="form-label" htmlFor="receipt-upload">{t('portal.claims.uploadReceipt')} *</label>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*,.pdf,.doc,.docx,.xls,.xlsx"
              disabled={uploading}
              onChange={handleFileSelect}
              className="sr-only"
              id="receipt-upload"
            />

            {!form.receipt_url ? (
              /* Drop zone / upload button */
              <label
                htmlFor="receipt-upload"
                className={`flex flex-col items-center justify-center border-2 border-dashed rounded-xl p-8 cursor-pointer transition-colors focus-within:ring-2 focus-within:ring-teal-500/40 ${
                  uploading
                    ? 'border-gray-400 dark:border-slate-600 bg-gray-50 dark:bg-slate-800/60'
                    : 'border-gray-300 dark:border-slate-700 hover:border-gray-400 dark:hover:border-slate-500 hover:bg-gray-50/50 dark:hover:bg-slate-800/40'
                }`}
              >
                {uploading ? (
                  <>
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 dark:border-slate-200 mb-3" />
                    <p className="text-sm font-medium text-gray-700 dark:text-slate-200">{t('common.uploading')}</p>
                  </>
                ) : (
                  <>
                    <Upload className="w-8 h-8 text-gray-400 dark:text-slate-400 mb-3" />
                    <p className="text-sm font-medium text-gray-700 dark:text-slate-200">{t('portal.claims.clickUpload')}</p>
                    <p className="text-xs text-gray-400 dark:text-slate-400 mt-1">
                      {t('portal.claims.uploadHint')}
                    </p>
                  </>
                )}
              </label>
            ) : (
              /* Uploaded file preview */
              <div className="border border-gray-200 dark:border-slate-700 rounded-xl p-4">
                <div className="flex items-start gap-4">
                  {/* Preview thumbnail or file icon */}
                  {uploadPreview ? (
                    <img
                      src={uploadPreview}
                      alt={t('portal.claims.receiptPreview')}
                      className="w-20 h-20 object-cover rounded-lg border border-gray-200 dark:border-slate-700"
                    />
                  ) : (
                    <div className="w-20 h-20 bg-gray-100 dark:bg-slate-800 rounded-lg flex items-center justify-center shrink-0">
                      {form.receipt_file_name?.toLowerCase().endsWith('.pdf') ? (
                        <FileText className="w-8 h-8 text-red-400" />
                      ) : isImageFile(form.receipt_file_name) ? (
                        <Image className="w-8 h-8 text-gray-500 dark:text-slate-300" />
                      ) : (
                        <FileText className="w-8 h-8 text-gray-400 dark:text-slate-400" />
                      )}
                    </div>
                  )}

                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">
                      {form.receipt_file_name}
                    </p>
                    <p className="text-xs text-green-600 dark:text-emerald-400 mt-1">{t('portal.claims.uploadedOk')}</p>
                  </div>

                  <button
                    type="button"
                    onClick={handleRemoveFile}
                    aria-label={t('common.removeFile')}
                    className="min-h-10 min-w-10 inline-flex items-center justify-center text-gray-400 dark:text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-lg transition-colors"
                    title={t('common.removeFile')}
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Actions */}
      <div className="flex justify-end gap-3">
        <button type="button" onClick={onClose} className="btn-secondary">{t('common.cancel')}</button>
        <button type="button" onClick={handleSubmit} disabled={mutation.isPending || uploading} className="btn-primary">
          {mutation.isPending ? t('common.creating') : t('portal.claims.createButton')}
        </button>
      </div>
    </div>
  );
}

function isImageFile(filename: string | undefined): boolean {
  if (!filename) return false;
  return /\.(jpg|jpeg|png|gif|webp)$/i.test(filename);
}
