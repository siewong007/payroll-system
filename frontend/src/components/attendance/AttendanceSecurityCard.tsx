import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CheckCircle2, Camera, Loader2, Shield, ShieldAlert, Smartphone, XCircle } from 'lucide-react';
import {
  approveAttendanceDevice, getAttendanceSecuritySettings, listCompanyAttendanceDevices,
  revokeAttendanceDevice, updateAttendanceSecuritySettings,
  type AttendanceSecuritySettings, type DeviceBindingMode, type PhotoCaptureMode,
} from '@/api/attendance';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';

interface AttendanceSecurityCardProps {
  canManage: boolean;
}

export function AttendanceSecurityCard({ canManage }: AttendanceSecurityCardProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const queryClient = useQueryClient();

  const [deviceFilter, setDeviceFilter] = useState<'pending' | 'approved' | 'revoked' | 'all'>('pending');
  const [draft, setDraft] = useState<Partial<AttendanceSecuritySettings> | null>(null);

  const { data: settings, isLoading: loadingSettings } = useQuery({
    queryKey: ['attendance-security-settings'],
    queryFn: getAttendanceSecuritySettings,
  });

  const { data: devices = [], isLoading: loadingDevices } = useQuery({
    queryKey: ['attendance-company-devices', deviceFilter],
    queryFn: () => listCompanyAttendanceDevices(deviceFilter === 'all' ? undefined : deviceFilter),
  });

  const current: AttendanceSecuritySettings = {
    device_mode: draft?.device_mode ?? settings?.device_mode ?? 'none',
    device_auto_approve_first: draft?.device_auto_approve_first ?? settings?.device_auto_approve_first ?? true,
    photo_mode: draft?.photo_mode ?? settings?.photo_mode ?? 'none',
    photo_retention_days: draft?.photo_retention_days ?? settings?.photo_retention_days ?? 30,
  };

  const saveMut = useMutation({
    mutationFn: () => updateAttendanceSecuritySettings(current),
    onSuccess: () => {
      setDraft(null);
      toast.success(t('attendance.security.saved'));
      void queryClient.invalidateQueries({ queryKey: ['attendance-security-settings'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('attendance.security.saveFailed'))),
  });

  const approveMut = useMutation({
    mutationFn: (id: string) => approveAttendanceDevice(id),
    onSuccess: () => {
      toast.success(t('attendance.security.deviceApproved'));
      void queryClient.invalidateQueries({ queryKey: ['attendance-company-devices'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-anomalies'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('attendance.security.actionFailed'))),
  });

  const revokeMut = useMutation({
    mutationFn: (id: string) => revokeAttendanceDevice(id),
    onSuccess: () => {
      toast.success(t('attendance.security.deviceRevoked'));
      void queryClient.invalidateQueries({ queryKey: ['attendance-company-devices'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-anomalies'] });
    },
    onError: (err) => toast.error(getErrorMessage(err, t('attendance.security.actionFailed'))),
  });

  return (
    <div className="card p-5 sm:p-6 space-y-6 lg:col-span-2">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="w-10 h-10 rounded-xl bg-emerald-50 dark:bg-emerald-500/15 flex items-center justify-center shrink-0">
            <Shield className="w-5 h-5 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
          </div>
          <div>
            <h2 className="font-semibold text-gray-900 dark:text-slate-100">
              {t('attendance.security.title')}
            </h2>
            <p className="text-sm text-gray-500 dark:text-slate-400 mt-0.5">
              {t('attendance.security.subtitle')}
            </p>
          </div>
        </div>
      </div>

      {/* Policy controls */}
      {loadingSettings ? (
        <div className="h-24 rounded-xl bg-gray-100 dark:bg-slate-800 animate-pulse" />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 p-4 rounded-xl bg-[var(--surface-subtle)] border border-[var(--border)]">
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-semibold text-gray-600 dark:text-slate-300 flex items-center gap-1.5">
              <Smartphone className="w-3.5 h-3.5" aria-hidden="true" />
              {t('attendance.security.deviceModeLabel')}
            </span>
            <select
              value={current.device_mode}
              disabled={!canManage || saveMut.isPending}
              onChange={(e) => setDraft({ ...current, device_mode: e.target.value as DeviceBindingMode })}
              className="form-input min-h-10 !py-1.5 text-sm"
            >
              <option value="none">{t('attendance.security.deviceModes.none')}</option>
              <option value="warn">{t('attendance.security.deviceModes.warn')}</option>
              <option value="enforce">{t('attendance.security.deviceModes.enforce')}</option>
            </select>
          </label>

          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-semibold text-gray-600 dark:text-slate-300 flex items-center gap-1.5">
              <Camera className="w-3.5 h-3.5" aria-hidden="true" />
              {t('attendance.security.photoModeLabel')}
            </span>
            <select
              value={current.photo_mode}
              disabled={!canManage || saveMut.isPending}
              onChange={(e) => setDraft({ ...current, photo_mode: e.target.value as PhotoCaptureMode })}
              className="form-input min-h-10 !py-1.5 text-sm"
            >
              <option value="none">{t('attendance.security.photoModes.none')}</option>
              <option value="optional">{t('attendance.security.photoModes.optional')}</option>
              <option value="required">{t('attendance.security.photoModes.required')}</option>
            </select>
          </label>

          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-semibold text-gray-600 dark:text-slate-300">
              {t('attendance.security.retentionDaysLabel')}
            </span>
            <input
              type="number"
              min={7}
              max={90}
              value={current.photo_retention_days}
              disabled={!canManage || saveMut.isPending}
              onChange={(e) => {
                const v = Math.min(90, Math.max(7, Number(e.target.value) || 30));
                setDraft({ ...current, photo_retention_days: v });
              }}
              className="form-input min-h-10 !py-1.5 text-sm tabular-nums"
            />
          </label>

          <div className="flex flex-col justify-between gap-2">
            <label className="flex items-start gap-2 text-xs text-gray-700 dark:text-slate-300 cursor-pointer pt-1">
              <input
                type="checkbox"
                checked={current.device_auto_approve_first}
                disabled={!canManage || saveMut.isPending}
                onChange={(e) =>
                  setDraft({ ...current, device_auto_approve_first: e.target.checked })
                }
                className="mt-0.5 w-4 h-4 rounded border-gray-300 dark:border-slate-600 accent-[var(--accent-1)]"
              />
              <span>{t('attendance.security.autoApproveFirst')}</span>
            </label>
            {canManage && (
              <button
                type="button"
                disabled={saveMut.isPending}
                onClick={() => saveMut.mutate()}
                className="btn-primary min-h-9 justify-center text-xs"
              >
                {saveMut.isPending ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : null}
                {saveMut.isPending ? t('common.saving') : t('common.save')}
              </button>
            )}
          </div>
        </div>
      )}

      {/* Bound Devices Approval & Revocation Queue */}
      <div className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100">
            {t('attendance.security.devicesHeading')}
          </h3>
          <div className="inline-flex rounded-xl bg-gray-100 dark:bg-slate-800 p-1 gap-1">
            {(['pending', 'approved', 'revoked', 'all'] as const).map((status) => (
              <button
                key={status}
                type="button"
                onClick={() => setDeviceFilter(status)}
                className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors ${
                  deviceFilter === status
                    ? 'bg-white dark:bg-slate-700 text-gray-900 dark:text-white shadow-sm'
                    : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                }`}
              >
                {t(`attendance.security.deviceFilter.${status}`)}
              </button>
            ))}
          </div>
        </div>

        {loadingDevices ? (
          <div className="h-20 rounded-xl bg-gray-100 dark:bg-slate-800 animate-pulse" />
        ) : devices.length === 0 ? (
          <div className="rounded-xl border border-dashed border-gray-200 dark:border-slate-700 p-6 text-center text-sm text-gray-500 dark:text-slate-400">
            {t('attendance.security.noDevices')}
          </div>
        ) : (
          <div className="overflow-x-auto border border-gray-100 dark:border-slate-800 rounded-xl">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-100 dark:border-slate-800 bg-gray-50/70 dark:bg-slate-900/50 text-left text-xs font-semibold text-gray-500 dark:text-slate-400">
                  <th className="px-4 py-2.5">{t('common.employee')}</th>
                  <th className="px-4 py-2.5">{t('attendance.security.deviceCol')}</th>
                  <th className="px-4 py-2.5">{t('attendance.security.fingerprintCol')}</th>
                  <th className="px-4 py-2.5">{t('common.status')}</th>
                  {canManage && <th className="px-4 py-2.5 text-right">{t('common.actions')}</th>}
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                {devices.map((dev) => (
                  <tr key={dev.id}>
                    <td className="px-4 py-3">
                      <div className="font-medium text-gray-900 dark:text-slate-100">{dev.full_name}</div>
                      <div className="text-xs text-gray-500 dark:text-slate-400">{dev.employee_number}</div>
                    </td>
                    <td className="px-4 py-3 text-gray-700 dark:text-slate-300">
                      <div>{dev.device_label}</div>
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-gray-500 dark:text-slate-400">
                      {dev.public_key_fingerprint.slice(0, 12)}
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${
                          dev.status === 'approved'
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300'
                            : dev.status === 'pending'
                              ? 'bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300'
                              : 'bg-red-100 text-red-700 dark:bg-red-500/20 dark:text-red-300'
                        }`}
                      >
                        {dev.status === 'approved' ? (
                          <CheckCircle2 className="w-3 h-3" />
                        ) : dev.status === 'pending' ? (
                          <ShieldAlert className="w-3 h-3" />
                        ) : (
                          <XCircle className="w-3 h-3" />
                        )}
                        {t(`attendance.security.deviceStatus.${dev.status}`)}
                      </span>
                    </td>
                    {canManage && (
                      <td className="px-4 py-3 text-right space-x-2 whitespace-nowrap">
                        {dev.status === 'pending' && (
                          <button
                            type="button"
                            disabled={approveMut.isPending}
                            onClick={() => approveMut.mutate(dev.id)}
                            className="px-2.5 py-1 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-medium"
                          >
                            {t('attendance.security.approveDevice')}
                          </button>
                        )}
                        {dev.status !== 'revoked' && (
                          <button
                            type="button"
                            disabled={revokeMut.isPending}
                            onClick={() => revokeMut.mutate(dev.id)}
                            className="px-2.5 py-1 rounded-lg bg-red-50 hover:bg-red-100 dark:bg-red-500/15 dark:hover:bg-red-500/25 text-red-700 dark:text-red-300 text-xs font-medium"
                          >
                            {t('attendance.security.revokeDevice')}
                          </button>
                        )}
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
