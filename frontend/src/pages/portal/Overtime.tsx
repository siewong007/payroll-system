import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Clock, X } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { TimeSelector } from '@/components/ui/TimeSelector';
import { getOvertimeApplications, createOvertimeApplication, cancelOvertimeApplication } from '@/api/portal';
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
  const [showModal, setShowModal] = useState(false);
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
        <div className="px-6 py-4 border-b border-gray-100">
          <h3 className="text-sm font-semibold text-gray-900">My Applications</h3>
        </div>
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
                  {app.status === 'pending' && (
                    <button
                      onClick={() => handleCancel(app)}
                      disabled={cancelMutation.isPending}
                      className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50"
                      title="Cancel"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  )}
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
