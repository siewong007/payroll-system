import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  MapPin, Plus, Trash2, CheckCircle2, AlertCircle, Shield, Pencil, RefreshCw,
} from 'lucide-react';
import {
  listLocations, createLocation, updateLocation, deleteLocation,
  getGeofenceMode, setGeofenceMode,
  type CompanyLocation,
} from '@/api/geofence';
import { useConfirm } from '@/lib/confirm';
import { getErrorMessage } from '@/lib/utils';
import type { TFunction } from 'i18next';

const MODE_VALUES = ['none', 'warn', 'enforce'] as const;

function modeLabel(mode: string, t: TFunction) {
  return t(`attendance.geofence.modes.${mode}.label`, { defaultValue: mode });
}
function modeDesc(mode: string, t: TFunction) {
  return t(`attendance.geofence.modes.${mode}.desc`, { defaultValue: '' });
}

function validateLocationCoords(
  latStr: string,
  lngStr: string,
  radiusStr: string,
  t: TFunction,
): { latitude: number; longitude: number; radius_meters: number } | { error: string } {
  const latitude = Number(latStr.trim());
  const longitude = Number(lngStr.trim());
  if (
    !latStr.trim() ||
    !lngStr.trim() ||
    !Number.isFinite(latitude) ||
    !Number.isFinite(longitude) ||
    latitude < -90 ||
    latitude > 90 ||
    longitude < -180 ||
    longitude > 180
  ) {
    return { error: t('attendance.geofence.invalidCoords') };
  }
  const radius = Number(radiusStr.trim());
  if (!radiusStr.trim() || !Number.isInteger(radius) || radius < 10 || radius > 10000) {
    return { error: t('attendance.geofence.invalidRadius') };
  }
  return { latitude, longitude, radius_meters: radius };
}

function AddLocationForm({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', latitude: '', longitude: '', radius_meters: '200' });
  const [locating, setLocating] = useState(false);
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: (payload: { name: string; latitude: number; longitude: number; radius_meters: number }) =>
      createLocation(payload),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['geofence-locations'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
      onClose();
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      setError(getErrorMessage(e, t('attendance.geofence.addFailed')));
    },
  });

  const handleAdd = () => {
    const validated = validateLocationCoords(form.latitude, form.longitude, form.radius_meters, t);
    if ('error' in validated) {
      setError(validated.error);
      return;
    }
    setError('');
    mutation.mutate({
      name: form.name.trim(),
      ...validated,
    });
  };

  const handleUseCurrentLocation = () => {
    if (typeof navigator === 'undefined' || !navigator.geolocation) {
      setError(t('attendance.geofence.noLocation'));
      return;
    }
    setError('');
    setLocating(true);
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setLocating(false);
        setForm(p => ({
          ...p,
          latitude: pos.coords.latitude.toFixed(6),
          longitude: pos.coords.longitude.toFixed(6),
        }));
      },
      () => {
        setLocating(false);
        setError(t('attendance.geofence.noLocation'));
      },
      { timeout: 8000 }
    );
  };

  return (
    <div className="border border-gray-200 dark:border-slate-700 rounded-xl p-4 space-y-3 bg-gray-50 dark:bg-slate-800/60">
      <div className="flex justify-between items-center">
        <p className="text-sm font-semibold text-gray-700 dark:text-slate-200">{t('attendance.geofence.addTitle')}</p>
        <button
          type="button"
          onClick={onClose}
          aria-label={t('common.close')}
          className="text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 text-lg leading-none px-1.5"
        >
          &times;
        </button>
      </div>
      <input
        placeholder={t('attendance.geofence.namePlaceholder')}
        aria-label={t('attendance.geofence.namePlaceholder')}
        value={form.name}
        onChange={e => setForm(p => ({ ...p, name: e.target.value }))}
        className="form-input"
      />
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <input
          placeholder={t('attendance.geofence.latitude')}
          aria-label={t('attendance.geofence.latitude')}
          inputMode="decimal"
          value={form.latitude}
          onChange={e => setForm(p => ({ ...p, latitude: e.target.value }))}
          className="form-input"
        />
        <input
          placeholder={t('attendance.geofence.longitude')}
          aria-label={t('attendance.geofence.longitude')}
          inputMode="decimal"
          value={form.longitude}
          onChange={e => setForm(p => ({ ...p, longitude: e.target.value }))}
          className="form-input"
        />
      </div>
      <button
        type="button"
        onClick={handleUseCurrentLocation}
        disabled={locating}
        className="text-xs text-sky-600 dark:text-sky-400 hover:text-sky-700 dark:hover:text-sky-300 font-medium disabled:opacity-50"
      >
        {locating ? t('attendance.geofence.locating') : t('attendance.geofence.useCurrent')}
      </button>
      <div>
        <label className="block text-xs text-gray-500 dark:text-slate-400 mb-1">{t('attendance.geofence.radius')}</label>
        <input
          type="number"
          min={10}
          max={10000}
          value={form.radius_meters}
          onChange={e => setForm(p => ({ ...p, radius_meters: e.target.value }))}
          className="form-input !w-32"
        />
      </div>
      {error && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
          <AlertCircle className="w-3.5 h-3.5 shrink-0" />{error}
        </p>
      )}
      <div className="flex gap-2">
        <button type="button" onClick={onClose} className="btn-secondary flex-1 !py-2">
          {t('common.cancel')}
        </button>
        <button
          type="button"
          onClick={handleAdd}
          disabled={!form.name.trim() || !form.latitude.trim() || !form.longitude.trim() || mutation.isPending}
          className="btn-primary flex-1 !py-2"
        >
          {mutation.isPending ? t('common.adding') : t('common.add')}
        </button>
      </div>
    </div>
  );
}

function LocationRow({ loc }: { loc: CompanyLocation }) {
  const { t } = useTranslation();
  const confirmDialog = useConfirm();
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState({
    name: loc.name,
    latitude: String(loc.latitude),
    longitude: String(loc.longitude),
    radius_meters: String(loc.radius_meters),
  });
  const [error, setError] = useState('');

  const updateMut = useMutation({
    mutationFn: (payload: { name: string; latitude: number; longitude: number; radius_meters: number }) =>
      updateLocation(loc.id, payload),
    onSuccess: () => {
      setError('');
      setEditing(false);
      void queryClient.invalidateQueries({ queryKey: ['geofence-locations'] });
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.geofence.updateFailed'))),
  });

  const handleSave = () => {
    const validated = validateLocationCoords(form.latitude, form.longitude, form.radius_meters, t);
    if ('error' in validated) {
      setError(validated.error);
      return;
    }
    setError('');
    updateMut.mutate({
      name: form.name.trim(),
      ...validated,
    });
  };

  const deleteMut = useMutation({
    mutationFn: () => deleteLocation(loc.id),
    onSuccess: () => {
      setError('');
      void queryClient.invalidateQueries({ queryKey: ['geofence-locations'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.geofence.removeFailed'))),
  });

  if (editing) {
    return (
      <div className="py-3 px-1 space-y-2.5">
        <input
          value={form.name}
          onChange={e => setForm(p => ({ ...p, name: e.target.value }))}
          aria-label={t('attendance.geofence.namePlaceholder')}
          className="form-input"
        />
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
          <input
            value={form.latitude}
            onChange={e => setForm(p => ({ ...p, latitude: e.target.value }))}
            aria-label={t('attendance.geofence.latitude')}
            inputMode="decimal"
            className="form-input"
          />
          <input
            value={form.longitude}
            onChange={e => setForm(p => ({ ...p, longitude: e.target.value }))}
            aria-label={t('attendance.geofence.longitude')}
            inputMode="decimal"
            className="form-input"
          />
          <input
            type="number"
            min={10}
            max={10000}
            value={form.radius_meters}
            onChange={e => setForm(p => ({ ...p, radius_meters: e.target.value }))}
            aria-label={t('attendance.geofence.radius')}
            className="form-input"
          />
        </div>
        {error && (
          <p role="alert" className="text-xs text-red-600 dark:text-red-400 flex items-center gap-1">
            <AlertCircle className="w-3.5 h-3.5 shrink-0" />{error}
          </p>
        )}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => {
              setEditing(false);
              setError('');
              setForm({
                name: loc.name,
                latitude: String(loc.latitude),
                longitude: String(loc.longitude),
                radius_meters: String(loc.radius_meters),
              });
            }}
            className="btn-secondary !py-1.5 !min-h-9"
          >
            {t('common.cancel')}
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={!form.name.trim() || !form.latitude.trim() || !form.longitude.trim() || updateMut.isPending}
            className="btn-primary !py-1.5 !min-h-9"
          >
            {updateMut.isPending ? t('common.saving') : t('common.save')}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="py-2.5 px-1 space-y-1">
      <div className="flex items-center gap-3">
        <MapPin className="w-4 h-4 text-sky-500 shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{loc.name}</p>
          <p className="text-xs text-gray-500 dark:text-slate-400">
            {t('attendance.geofence.coords', { lat: loc.latitude.toFixed(4), lng: loc.longitude.toFixed(4), radius: loc.radius_meters })}
          </p>
        </div>
        <button
          type="button"
          onClick={() => {
            setForm({
              name: loc.name,
              latitude: String(loc.latitude),
              longitude: String(loc.longitude),
              radius_meters: String(loc.radius_meters),
            });
            setError('');
            setEditing(true);
          }}
          className="text-gray-400 dark:text-slate-500 hover:text-gray-700 dark:hover:text-slate-200 p-2 rounded-lg transition-colors"
          title={t('attendance.geofence.editLocation')}
          aria-label={t('attendance.geofence.editLocation')}
        >
          <Pencil className="w-4 h-4" />
        </button>
        <button
          type="button"
          onClick={async () => {
            const ok = await confirmDialog({
              description: t('attendance.geofence.removeConfirm', { name: loc.name }),
              variant: 'danger',
            });
            if (ok) {
              deleteMut.mutate();
            }
          }}
          disabled={deleteMut.isPending}
          className="text-gray-400 dark:text-slate-500 hover:text-red-500 dark:hover:text-red-400 p-2 rounded-lg transition-colors disabled:opacity-50"
          title={t('attendance.geofence.remove')}
          aria-label={t('attendance.geofence.remove')}
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
      {error && (
        <p role="alert" className="text-xs text-red-600 dark:text-red-400 pl-7 flex items-center gap-1">
          <AlertCircle className="w-3.5 h-3.5 shrink-0" />{error}
        </p>
      )}
    </div>
  );
}

export function GeofenceCard() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [showAdd, setShowAdd] = useState(false);
  const [modeError, setModeError] = useState('');

  const {
    data: locations = [],
    isLoading: locationsLoading,
    isError: locationsError,
    error: locationsErrObj,
    refetch: refetchLocations,
  } = useQuery({
    queryKey: ['geofence-locations'],
    queryFn: listLocations,
  });

  const { data: modeData, isError: modeLoadError, refetch: refetchMode } = useQuery({
    queryKey: ['geofence-mode'],
    queryFn: getGeofenceMode,
  });

  const modeMut = useMutation({
    mutationFn: (mode: string) => setGeofenceMode(mode),
    onSuccess: () => {
      setModeError('');
      void queryClient.invalidateQueries({ queryKey: ['geofence-mode'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
    },
    onError: (e) => setModeError(getErrorMessage(e, t('attendance.geofence.modeFailed'))),
  });

  const currentMode = modeData?.mode ?? 'none';

  return (
    <div className="card !p-0 overflow-hidden">
      <div className="p-5 sm:p-6 border-b border-gray-100 dark:border-slate-800">
        <div className="flex items-center gap-2 mb-1">
          <Shield className="w-5 h-5 text-[var(--accent-1)]" />
          <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('attendance.geofence.title')}</h2>
        </div>
        <p className="text-sm text-gray-500 dark:text-slate-400">
          {t('attendance.geofence.body')}
        </p>
      </div>

      <div className="p-5 sm:p-6 space-y-5">
        <div>
          <p className="block text-sm font-medium text-gray-700 dark:text-slate-300 mb-2">{t('attendance.geofence.modeLabel')}</p>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
            {MODE_VALUES.map(mode => (
              <button
                key={mode}
                type="button"
                aria-pressed={currentMode === mode}
                onClick={() => modeMut.mutate(mode)}
                className={`relative p-3 rounded-xl border-2 text-left transition-all ${
                  currentMode === mode
                    ? 'border-[var(--accent-1)] bg-[var(--accent-soft)]'
                    : 'border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700'
                }`}
              >
                <p className="text-sm font-semibold text-gray-900 dark:text-slate-100 pr-5">{modeLabel(mode, t)}</p>
                <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{modeDesc(mode, t)}</p>
                {currentMode === mode && (
                  <CheckCircle2 className="w-3.5 h-3.5 text-[var(--accent-1)] absolute top-3 right-3 sm:static sm:mt-1.5" />
                )}
              </button>
            ))}
          </div>
          {modeError && (
            <p role="alert" className="mt-2 text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
              <AlertCircle className="w-3.5 h-3.5 shrink-0" />{modeError}
            </p>
          )}
        </div>

        {/* Locations */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-gray-700 dark:text-slate-300">{t('attendance.geofence.locationsLabel')}</span>
            <button
              type="button"
              onClick={() => setShowAdd(true)}
              className="btn-secondary !py-1.5 !px-2.5 !min-h-8 text-xs"
            >
              <Plus className="w-3.5 h-3.5" /> {t('common.add')}
            </button>
          </div>

          {showAdd && <AddLocationForm onClose={() => setShowAdd(false)} />}

          {locationsLoading ? (
            <div className="flex items-center justify-center py-6">
              <div className="spinner" />
            </div>
          ) : locationsError || modeLoadError ? (
            <div className="py-4 text-center space-y-2">
              <p className="text-xs text-red-600 dark:text-red-400">
                {getErrorMessage(locationsErrObj, t('attendance.geofence.loadFailed'))}
              </p>
              <button
                type="button"
                onClick={() => { void refetchLocations(); void refetchMode(); }}
                className="inline-flex items-center gap-1 text-xs font-medium text-gray-700 dark:text-slate-300 hover:underline"
              >
                <RefreshCw className="w-3 h-3" /> {t('common.retry')}
              </button>
            </div>
          ) : locations.length === 0 && !showAdd ? (
            <p className="text-xs text-gray-400 dark:text-slate-500 py-4 text-center">
              {t('attendance.geofence.empty')}{currentMode !== 'none' && ` ${t('attendance.geofence.emptyHint')}`}
            </p>
          ) : (
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              {locations.map(loc => (
                <LocationRow key={loc.id} loc={loc} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
