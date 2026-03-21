import { useState, useRef } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, ArrowLeft, Send, Trash2, Receipt, Upload, X, FileText, Image, Paperclip, ExternalLink } from 'lucide-react';
import { getClaims, createClaim, submitClaim, deleteClaim, uploadFile } from '@/api/portal';
import { formatMYR, formatDate } from '@/lib/utils';

const STATUS_TABS = [
  { key: null, label: 'All' },
  { key: 'draft', label: 'Draft' },
  { key: 'pending', label: 'Pending' },
  { key: 'approved', label: 'Approved' },
  { key: 'rejected', label: 'Rejected' },
  { key: 'processed', label: 'Processed' },
] as const;

const CATEGORIES = [
  'Transport', 'Meals', 'Accommodation', 'Office Expenses',
  'Medical', 'Training', 'Entertainment', 'Other',
];

export function Claims() {
  const queryClient = useQueryClient();
  const [statusFilter, setStatusFilter] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const { data: claims, isLoading } = useQuery({
    queryKey: ['my-claims', statusFilter],
    queryFn: () => getClaims(statusFilter ?? undefined),
  });

  const submitMutation = useMutation({
    mutationFn: submitClaim,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-claims'] }),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteClaim,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-claims'] }),
  });

  const statusBadge = (status: string) => {
    const cls: Record<string, string> = {
      draft: 'badge-draft', pending: 'badge-pending', approved: 'badge-approved',
      rejected: 'badge-rejected', processed: 'badge-processed',
    };
    return <span className={`badge ${cls[status] || 'badge-draft'}`}>{status}</span>;
  };

  const statusCount = (key: string) => claims?.filter((c) => c.status === key).length ?? 0;

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
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
          <h1 className="page-title">Claims</h1>
          <p className="page-subtitle">Submit and track your expense claims</p>
        </div>
        <button onClick={() => setShowCreate(true)} className="btn-primary w-full sm:w-auto">
          <Plus className="w-4 h-4" /> Add Expense
        </button>
      </div>

      {/* Status Filter Tabs */}
      <div className="flex gap-2 flex-wrap">
        {STATUS_TABS.map((t) => {
          const isActive = statusFilter === t.key;
          const count = t.key ? statusCount(t.key) : claims?.length ?? 0;
          return (
            <button
              key={t.key ?? 'all'}
              onClick={() => setStatusFilter(t.key)}
              className={`px-4 py-1.5 rounded-full text-sm font-medium transition-all-fast ${
                isActive
                  ? 'bg-black text-white shadow-sm'
                  : 'bg-white text-gray-500 border border-gray-200 hover:border-gray-300 hover:text-gray-700'
              }`}
            >
              {t.label} {count > 0 && <span className="ml-1 opacity-70">{count}</span>}
            </button>
          );
        })}
      </div>

      {/* Claims Table */}
      <div className="card p-0 overflow-hidden">
        {!claims || claims.length === 0 ? (
          <div className="text-center py-16 text-gray-400">
            <Receipt className="w-10 h-10 mx-auto mb-3 opacity-40" />
            <p>No expense claims found</p>
          </div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Expense / Transaction</th>
                <th className="text-right">Amount</th>
                <th>Category</th>
                <th>Date</th>
                <th>Receipt</th>
                <th className="text-center">Status</th>
                <th className="text-center">Actions</th>
              </tr>
            </thead>
            <tbody>
              {claims.map((c) => (
                <tr key={c.id}>
                  <td>
                    <span className="font-semibold text-gray-900">{c.title}</span>
                    {c.description && <p className="text-xs text-gray-400 mt-0.5 truncate max-w-xs">{c.description}</p>}
                  </td>
                  <td className="text-right"><span className="font-bold text-gray-900">{formatMYR(c.amount)}</span></td>
                  <td className="text-gray-500">{c.category || '—'}</td>
                  <td className="text-gray-500">{formatDate(c.expense_date)}</td>
                  <td>
                    {c.receipt_url ? (
                      c.receipt_url.startsWith('blob:') ? (
                        <span className="inline-flex items-center gap-1 text-red-400 text-sm">
                          <Paperclip className="w-3 h-3" />
                          <span className="truncate max-w-[100px]">Unavailable</span>
                        </span>
                      ) : (
                        <a href={c.receipt_url} target="_blank" rel="noopener noreferrer"
                          className="inline-flex items-center gap-1 text-gray-900 hover:text-black text-sm">
                          <Paperclip className="w-3 h-3" />
                          <span className="truncate max-w-[100px]">{c.receipt_file_name || 'View'}</span>
                          <ExternalLink className="w-3 h-3" />
                        </a>
                      )
                    ) : <span className="text-gray-300">—</span>}
                  </td>
                  <td className="text-center">{statusBadge(c.status)}</td>
                  <td className="text-center">
                    {c.status === 'draft' && (
                      <div className="flex items-center justify-center gap-2">
                        <button onClick={() => submitMutation.mutate(c.id)} title="Submit for approval" className="p-1.5 text-gray-900 hover:text-black hover:bg-gray-100 rounded-lg transition-all-fast">
                          <Send className="w-4 h-4" />
                        </button>
                        <button onClick={() => { if (confirm('Delete this claim?')) deleteMutation.mutate(c.id); }} title="Delete" className="p-1.5 text-red-500 hover:text-red-700 hover:bg-red-50 rounded-lg transition-all-fast">
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

/* ───────────── Full-page Create Claim Form ───────────── */
function CreateClaimForm({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [form, setForm] = useState({
    title: '', description: '', amount: '', category: '',
    expense_date: new Date().toISOString().split('T')[0],
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
    onError: (err: any) => {
      setError(err.response?.data?.error || 'Failed to create claim');
    },
  });

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    // Client-side size check (10 MB)
    if (file.size > 10 * 1024 * 1024) {
      setError('File too large. Maximum size is 10 MB.');
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
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to upload file');
      setUploadPreview(null);
    } finally {
      setUploading(false);
    }
  };

  const handleRemoveFile = () => {
    setForm((prev) => ({ ...prev, receipt_url: '', receipt_file_name: '' }));
    setUploadPreview(null);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const handleSubmit = () => {
    if (!form.title || !form.amount || !form.expense_date) {
      setError('Title, amount, and expense date are required');
      return;
    }
    if (!form.receipt_url) {
      setError('Please upload a receipt before submitting');
      return;
    }
    mutation.mutate({
      title: form.title,
      description: form.description || undefined,
      amount: Math.round(Number(form.amount) * 100),
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
        <button onClick={onClose} className="btn-secondary !px-3 !py-2">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="page-header">
          <h1 className="page-title">Add Expense Claim</h1>
          <p className="page-subtitle">Submit a new expense for reimbursement</p>
        </div>
      </div>

      {error && <div className="p-4 bg-red-50 text-red-700 text-sm rounded-xl border border-red-100">{error}</div>}

      <div className="bg-white rounded-2xl shadow divide-y divide-gray-100">
        {/* Section 1: Basic Info */}
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">1</span>
            <span className="section-title">Expense Details</span>
          </div>
          <div className="space-y-5 max-w-2xl">
            <div>
              <label className="form-label">Title *</label>
              <input
                value={form.title}
                onChange={(e) => setForm((prev) => ({ ...prev, title: e.target.value }))}
                className="form-input"
                placeholder="e.g., Medical Claim - Clinic Visit"
              />
            </div>
            <div>
              <label className="form-label">Description</label>
              <textarea
                value={form.description}
                onChange={(e) => setForm((prev) => ({ ...prev, description: e.target.value }))}
                rows={2}
                className="form-input"
                placeholder="Add details about this expense..."
              />
            </div>
          </div>
        </div>

        {/* Section 2: Amount & Category */}
        <div className="p-6 lg:p-8">
          <div className="section-header">
            <span className="section-number">2</span>
            <span className="section-title">Amount & Category</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-2xl">
            <div>
              <label className="form-label">Amount (RM) *</label>
              <input
                type="number" step="0.01" min="0"
                value={form.amount}
                onChange={(e) => setForm((prev) => ({ ...prev, amount: e.target.value }))}
                className="form-input" placeholder="0.00"
              />
            </div>
            <div>
              <label className="form-label">Category</label>
              <select
                value={form.category}
                onChange={(e) => setForm((prev) => ({ ...prev, category: e.target.value }))}
                className="form-input"
              >
                <option value="">Select...</option>
                {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            </div>
            <div>
              <label className="form-label">Expense Date *</label>
              <input
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
            <span className="section-title">Receipt / Proof *</span>
          </div>
          <div className="max-w-2xl">
            <label className="form-label">Upload Receipt *</label>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*,.pdf,.doc,.docx,.xls,.xlsx"
              onChange={handleFileSelect}
              className="hidden"
              id="receipt-upload"
            />

            {!form.receipt_url ? (
              /* Drop zone / upload button */
              <label
                htmlFor="receipt-upload"
                className={`flex flex-col items-center justify-center border-2 border-dashed rounded-xl p-8 cursor-pointer transition-colors ${
                  uploading
                    ? 'border-gray-400 bg-gray-50'
                    : 'border-gray-300 hover:border-gray-400 hover:bg-gray-50/50'
                }`}
              >
                {uploading ? (
                  <>
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900 mb-3" />
                    <p className="text-sm font-medium text-gray-700">Uploading...</p>
                  </>
                ) : (
                  <>
                    <Upload className="w-8 h-8 text-gray-400 mb-3" />
                    <p className="text-sm font-medium text-gray-700">Click to upload receipt</p>
                    <p className="text-xs text-gray-400 mt-1">
                      JPG, PNG, PDF, DOC, XLS up to 10 MB
                    </p>
                  </>
                )}
              </label>
            ) : (
              /* Uploaded file preview */
              <div className="border border-gray-200 rounded-xl p-4">
                <div className="flex items-start gap-4">
                  {/* Preview thumbnail or file icon */}
                  {uploadPreview ? (
                    <img
                      src={uploadPreview}
                      alt="Receipt preview"
                      className="w-20 h-20 object-cover rounded-lg border border-gray-200"
                    />
                  ) : (
                    <div className="w-20 h-20 bg-gray-100 rounded-lg flex items-center justify-center shrink-0">
                      {form.receipt_file_name?.toLowerCase().endsWith('.pdf') ? (
                        <FileText className="w-8 h-8 text-red-400" />
                      ) : isImageFile(form.receipt_file_name) ? (
                        <Image className="w-8 h-8 text-gray-500" />
                      ) : (
                        <FileText className="w-8 h-8 text-gray-400" />
                      )}
                    </div>
                  )}

                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-gray-900 truncate">
                      {form.receipt_file_name}
                    </p>
                    <p className="text-xs text-green-600 mt-1">Uploaded successfully</p>
                  </div>

                  <button
                    type="button"
                    onClick={handleRemoveFile}
                    className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors"
                    title="Remove file"
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
        <button onClick={onClose} className="btn-secondary">Cancel</button>
        <button onClick={handleSubmit} disabled={mutation.isPending || uploading} className="btn-primary">
          {mutation.isPending ? 'Creating...' : 'Create Claim'}
        </button>
      </div>
    </div>
  );
}

function isImageFile(filename: string | undefined): boolean {
  if (!filename) return false;
  return /\.(jpg|jpeg|png|gif|webp)$/i.test(filename);
}
