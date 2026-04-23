import { useEffect, useRef, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Clock, X, Trash2 } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { getOvertimeApplications, createOvertimeApplication, cancelOvertimeApplication, deleteOvertimeApplication } from '@/api/portal';
import { formatDate } from '@/lib/utils';
import type { OvertimeApplication, CreateOvertimeRequest } from '@/types';

const statusBadge = (status: string) => {
  const cls: Record<string, string> = {
    pending: 'badge-pending',
    approved: 'badge-approved',
    rejected: 'badge-rejected',
    cancelled: 'badge-cancelled',
  };
  return <span className={`badge ${cls[status] || 'badge-draft'}`}>{status}</span>;
};

const otTypeLabel = (t: string) => {
  const labels: Record<string, string> = {
    normal: 'Normal Day',
    rest_day: 'Rest Day',
    public_holiday: 'Public Holiday',
  };
  return labels[t] || t;
};

const otTypeMultiplier = (t: string) => {
  const m: Record<string, string> = { normal: '1.5x', rest_day: '2.0x', public_holiday: '3.0x' };
  return m[t] || '';
};

export function Overtime() {
  const queryClient = useQueryClient();
  const selectAllRef = useRef<HTMLInputElement>(null);
  const [showModal, setShowModal] = useState(false);
  const [selectedOvertimeIds, setSelectedOvertimeIds] = useState<string[]>([]);
  const [form, setForm] = useState<CreateOvertimeRequest>({
    ot_date: '',
    start_time: '',
    end_time: '',
    hours: 0,
    ot_type: 'normal',
    reason: '',
  });

  const { data: applications = [], isLoading } = useQuery({
    queryKey: ['my-overtime'],
    queryFn: getOvertimeApplications,
  });

  const createMutation = useMutation({
    mutationFn: createOvertimeApplication,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['my-overtime'] });
      setShowModal(false);
      resetForm();
    },
  });

  const cancelMutation = useMutation({
    mutationFn: cancelOvertimeApplication,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-overtime'] }),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteOvertimeApplication,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-overtime'] }),
  });

  const bulkCancelMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map((id) => cancelOvertimeApplication(id)));
    },
    onSuccess: () => {
      setSelectedOvertimeIds([]);
      queryClient.invalidateQueries({ queryKey: ['my-overtime'] });
    },
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map((id) => deleteOvertimeApplication(id)));
    },
    onSuccess: () => {
      setSelectedOvertimeIds([]);
      queryClient.invalidateQueries({ queryKey: ['my-overtime'] });
    },
  });

  const resetForm = () => {
    setForm({ ot_date: '', start_time: '', end_time: '', hours: 0, ot_type: 'normal', reason: '' });
  };

  const calculateHours = (start: string, end: string) => {
    if (!start || !end) return 0;
    const [sh, sm] = start.split(':').map(Number);
    const [eh, em] = end.split(':').map(Number);
    let diff = (eh * 60 + em) - (sh * 60 + sm);
    if (diff <= 0) diff += 24 * 60; // overnight
    return Math.round(diff / 30) * 0.5; // round to nearest 0.5
  };

  const updateTime = (field: 'start_time' | 'end_time', value: string) => {
    const updated = { ...form, [field]: value };
    updated.hours = calculateHours(
      field === 'start_time' ? value : form.start_time,
      field === 'end_time' ? value : form.end_time,
    );
    setForm(updated);
  };

  const handleSubmit = () => {
    if (!form.ot_date || !form.start_time || !form.end_time || form.hours <= 0) return;
    createMutation.mutate({
      ...form,
      reason: form.reason || undefined,
    });
  };

  const handleCancel = (app: OvertimeApplication) => {
    if (confirm('Cancel this overtime application?')) {
      cancelMutation.mutate(app.id);
    }
  };

  const handleDelete = (app: OvertimeApplication) => {
    if (confirm('Permanently delete this cancelled overtime application?')) {
      deleteMutation.mutate(app.id);
    }
  };

  const canCancel = (app: OvertimeApplication) => ['pending', 'approved', 'rejected'].includes(app.status);
  const displayedOvertimeIds = applications.map((app) => app.id);
  const selectedDisplayedIds = selectedOvertimeIds.filter((id) => displayedOvertimeIds.includes(id));
  const allDisplayedSelected = displayedOvertimeIds.length > 0 && selectedDisplayedIds.length === displayedOvertimeIds.length;
  const someDisplayedSelected = selectedDisplayedIds.length > 0 && !allDisplayedSelected;
  const selectedApplications = applications.filter((app) => selectedOvertimeIds.includes(app.id));
  const selectedCancelableIds = selectedApplications.filter(canCancel).map((app) => app.id);
  const selectedDeletableIds = selectedApplications.filter((app) => app.status === 'cancelled').map((app) => app.id);

  useEffect(() => {
    if (selectAllRef.current) {
      selectAllRef.current.indeterminate = someDisplayedSelected;
    }
  }, [someDisplayedSelected]);

  const toggleOvertimeSelection = (id: string) => {
    setSelectedOvertimeIds((current) => (
      current.includes(id) ? current.filter((selectedId) => selectedId !== id) : [...current, id]
    ));
  };

  const toggleAllDisplayedOvertime = () => {
    setSelectedOvertimeIds((current) => (
      allDisplayedSelected
        ? current.filter((id) => !displayedOvertimeIds.includes(id))
        : Array.from(new Set([...current, ...displayedOvertimeIds]))
    ));
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">Overtime</h1>
          <p className="page-subtitle">Submit and track overtime applications</p>
        </div>
        <button
          onClick={() => { resetForm(); setShowModal(true); }}
          className="btn-primary flex items-center gap-2 w-full sm:w-auto"
        >
          <Plus className="w-4 h-4" />
          Apply OT
        </button>
      </div>

      {/* Applications List */}
      <div className="bg-white rounded-2xl border border-gray-200">
        <div className="flex items-center justify-between gap-3 px-6 py-4 border-b border-gray-100">
          <h3 className="text-sm font-semibold text-gray-900">My Applications</h3>
          {applications.length > 0 && (
            <label className="inline-flex items-center gap-2 text-xs font-medium text-gray-500">
              <input
                ref={selectAllRef}
                type="checkbox"
                checked={allDisplayedSelected}
                onChange={toggleAllDisplayedOvertime}
                className="h-4 w-4 rounded border-gray-300 text-gray-900 focus:ring-gray-900"
              />
              Select all
            </label>
          )}
        </div>
        {selectedOvertimeIds.length > 0 && (
          <div className="flex flex-col gap-2 border-b border-gray-100 bg-gray-50 px-6 py-3 sm:flex-row sm:items-center sm:justify-between">
            <span className="text-sm font-medium text-gray-700">{selectedOvertimeIds.length} selected</span>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => {
                  if (confirm(`Cancel ${selectedCancelableIds.length} selected overtime application(s)?`)) {
                    bulkCancelMutation.mutate(selectedCancelableIds);
                  }
                }}
                disabled={selectedCancelableIds.length === 0 || bulkCancelMutation.isPending}
                className="btn-secondary !py-2 text-sm disabled:opacity-50"
              >
                <X className="w-4 h-4" />
                {bulkCancelMutation.isPending ? 'Cancelling...' : 'Cancel Selected'}
              </button>
              <button
                type="button"
                onClick={() => {
                  if (confirm(`Permanently delete ${selectedDeletableIds.length} cancelled overtime application(s)?`)) {
                    bulkDeleteMutation.mutate(selectedDeletableIds);
                  }
                }}
                disabled={selectedDeletableIds.length === 0 || bulkDeleteMutation.isPending}
                className="btn-secondary !py-2 text-sm text-red-600 hover:!bg-red-50 disabled:opacity-50"
              >
                <Trash2 className="w-4 h-4" />
                {bulkDeleteMutation.isPending ? 'Deleting...' : 'Delete Selected'}
              </button>
            </div>
          </div>
        )}
        {isLoading ? (
          <div className="p-8 text-center text-sm text-gray-400">Loading...</div>
        ) : applications.length === 0 ? (
          <div className="p-12 text-center">
            <Clock className="w-10 h-10 text-gray-200 mx-auto mb-3" />
            <p className="text-sm text-gray-400">No overtime applications yet</p>
          </div>
        ) : (
          <div className="divide-y divide-gray-100">
            {applications.map((app) => (
              <div key={app.id} className="px-6 py-4">
                <div className="flex items-start justify-between">
                  <div className="flex items-start gap-3">
                    <input
                      type="checkbox"
                      checked={selectedOvertimeIds.includes(app.id)}
                      onChange={() => toggleOvertimeSelection(app.id)}
                      className="mt-1 h-4 w-4 rounded border-gray-300 text-gray-900 focus:ring-gray-900"
                      aria-label={`Select overtime application for ${formatDate(app.ot_date)}`}
                    />
                    <div className="space-y-1">
                    <div className="flex items-center gap-3">
                      <span className="text-sm font-semibold text-gray-900">{formatDate(app.ot_date)}</span>
                      {statusBadge(app.status)}
                      <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
                        app.ot_type === 'public_holiday' ? 'bg-red-100 text-red-700' :
                        app.ot_type === 'rest_day' ? 'bg-blue-100 text-blue-700' :
                        'bg-gray-100 text-gray-600'
                      }`}>
                        {otTypeLabel(app.ot_type)} ({otTypeMultiplier(app.ot_type)})
                      </span>
                    </div>
                    <p className="text-sm text-gray-500">
                      {app.start_time?.slice(0, 5)} — {app.end_time?.slice(0, 5)}
                      <span className="ml-2 font-medium text-gray-700">{app.hours}h</span>
                    </p>
                    {app.reason && <p className="text-sm text-gray-400">{app.reason}</p>}
                    {app.review_notes && (
                      <p className="text-sm text-amber-600 mt-1">Note: {app.review_notes}</p>
                    )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {canCancel(app) && (
                      <button
                        onClick={() => handleCancel(app)}
                        disabled={cancelMutation.isPending}
                        className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50"
                        title="Cancel"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    )}
                    {app.status === 'cancelled' && (
                      <button
                        onClick={() => handleDelete(app)}
                        disabled={deleteMutation.isPending}
                        className="p-1.5 rounded-lg text-red-500 hover:text-red-700 hover:bg-red-50"
                        title="Delete permanently"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Create Modal */}
      <Modal
        open={showModal}
        onClose={() => setShowModal(false)}
        title="Apply for Overtime"
        footer={
          <div className="flex justify-end gap-3">
            <button onClick={() => setShowModal(false)} className="btn-secondary">Cancel</button>
            <button
              onClick={handleSubmit}
              disabled={!form.ot_date || !form.start_time || !form.end_time || form.hours <= 0 || createMutation.isPending}
              className="btn-primary"
            >
              {createMutation.isPending ? 'Submitting...' : 'Submit Application'}
            </button>
          </div>
        }
      >
        <div className="space-y-4">
          <div>
            <label className="form-label">OT Date *</label>
            <input
              type="date"
              value={form.ot_date}
              onChange={(e) => setForm({ ...form, ot_date: e.target.value })}
              className="form-input"
            />
          </div>

          <div>
            <label className="form-label">OT Type *</label>
            <div className="grid grid-cols-3 gap-2">
              {(['normal', 'rest_day', 'public_holiday'] as const).map((t) => (
                <label
                  key={t}
                  className={`flex flex-col items-center gap-1 px-3 py-3 rounded-lg border-2 cursor-pointer transition-all text-center ${
                    form.ot_type === t
                      ? t === 'public_holiday' ? 'border-red-400 bg-red-50' : t === 'rest_day' ? 'border-blue-400 bg-blue-50' : 'border-gray-900 bg-gray-50'
                      : 'border-gray-200 hover:border-gray-300'
                  }`}
                >
                  <input
                    type="radio"
                    name="ot_type"
                    value={t}
                    checked={form.ot_type === t}
                    onChange={() => setForm({ ...form, ot_type: t })}
                    className="sr-only"
                  />
                  <span className="text-xs font-medium">{otTypeLabel(t)}</span>
                  <span className="text-[10px] text-gray-400">{otTypeMultiplier(t)}</span>
                </label>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <TimeSelector
              label="Start Time *"
              value={form.start_time || '18:00'}
              onChange={(value) => updateTime('start_time', value)}
            />
            <TimeSelector
              label="End Time *"
              value={form.end_time || '19:00'}
              onChange={(value) => updateTime('end_time', value)}
            />
          </div>

          {form.hours > 0 && (
            <div className="bg-gray-50 rounded-lg p-3 text-center">
              <span className="text-2xl font-bold text-gray-900">{form.hours}</span>
              <span className="text-sm text-gray-400 ml-1">hours</span>
            </div>
          )}

          <div>
            <label className="form-label">Reason</label>
            <textarea
              value={form.reason}
              onChange={(e) => setForm({ ...form, reason: e.target.value })}
              className="form-input"
              rows={2}
              placeholder="Describe the work performed..."
            />
          </div>

          {createMutation.isError && (
            <p className="text-sm text-red-600">{(createMutation.error as Error).message || 'Failed to submit'}</p>
          )}
        </div>
      </Modal>
    </div>
  );
}
