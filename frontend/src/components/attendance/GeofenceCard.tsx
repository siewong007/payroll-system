import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  MapPin, Plus, Trash2, CheckCircle2, AlertCircle, Shield,
} from 'lucide-react';
import {
  listLocations, createLocation, deleteLocation,
  getGeofenceMode, setGeofenceMode,
  type CompanyLocation,
} from '@/api/geofence';

const MODE_OPTIONS = [
  { value: 'none', label: 'Off', desc: 'No location check on check-in' },
  { value: 'warn', label: 'Warn', desc: 'Allow check-in but flag if outside office radius' },
  { value: 'enforce', label: 'Enforce', desc: 'Block check-in if outside office radius' },
];

function AddLocationForm({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', latitude: '', longitude: '', radius_meters: '200' });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => createLocation({
      name: form.name,
      latitude: parseFloat(form.latitude),
      longitude: parseFloat(form.longitude),
      radius_meters: parseInt(form.radius_meters) || 200,
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['geofence-locations'] });
      onClose();
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      setError(e.response?.data?.error || 'Failed to add location');
    },
  });

  const handleUseCurrentLocation = () => {
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setForm(p => ({
          ...p,
          latitude: pos.coords.latitude.toFixed(6),
          longitude: pos.coords.longitude.toFixed(6),
        }));
      },
      () => setError('Could not get current location'),
      { timeout: 8000 }
    );
  };

  return (
    <div className="border border-gray-200 rounded-xl p-4 space-y-3 bg-gray-50">
      <div className="flex justify-between items-center">
        <p className="text-sm font-semibold text-gray-700">Add Office Location</p>
        <button onClick={onClose} className="text-gray-400 hover:text-gray-600 text-lg leading-none">&times;</button>
      </div>
      <input
        placeholder="Location name (e.g. HQ Office)"
        value={form.name}
        onChange={e => setForm(p => ({ ...p, name: e.target.value }))}
        className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
      />
      <div className="grid grid-cols-2 gap-3">
        <input
          placeholder="Latitude"
          value={form.latitude}
          onChange={e => setForm(p => ({ ...p, latitude: e.target.value }))}
          className="px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
        <input
          placeholder="Longitude"
          value={form.longitude}
          onChange={e => setForm(p => ({ ...p, longitude: e.target.value }))}
          className="px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
      </div>
      <button
        type="button"
        onClick={handleUseCurrentLocation}
        className="text-xs text-sky-600 hover:text-sky-700 font-medium"
      >
        Use my current location
      </button>
      <div>
        <label className="block text-xs text-gray-500 mb-1">Radius (meters)</label>
        <input
          type="number"
          value={form.radius_meters}
          onChange={e => setForm(p => ({ ...p, radius_meters: e.target.value }))}
          className="w-32 px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
        />
      </div>
      {error && (
        <p className="text-sm text-red-600 flex items-center gap-1">
          <AlertCircle className="w-3.5 h-3.5" />{error}
        </p>
      )}
      <div className="flex gap-2">
        <button onClick={onClose}
          className="flex-1 py-2 border border-gray-300 rounded-lg text-sm text-gray-600 hover:bg-gray-100">
          Cancel
        </button>
        <button
          onClick={() => mutation.mutate()}
          disabled={!form.name || !form.latitude || !form.longitude || mutation.isPending}
          className="flex-1 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-40"
        >
          {mutation.isPending ? 'Adding...' : 'Add'}
        </button>
      </div>
    </div>
  );
}

function LocationRow({ loc }: { loc: CompanyLocation }) {
  const queryClient = useQueryClient();
  const deleteMut = useMutation({
    mutationFn: () => deleteLocation(loc.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['geofence-locations'] }),
  });

  return (
    <div className="flex items-center gap-3 py-2.5 px-1">
      <MapPin className="w-4 h-4 text-sky-500 shrink-0" />
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-gray-900 truncate">{loc.name}</p>
        <p className="text-xs text-gray-400">
          {loc.latitude.toFixed(4)}, {loc.longitude.toFixed(4)} &middot; {loc.radius_meters}m radius
        </p>
      </div>
      <button
        onClick={() => deleteMut.mutate()}
        disabled={deleteMut.isPending}
        className="text-gray-300 hover:text-red-500 p-1 transition-colors"
        title="Remove location"
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </div>
  );
}

export function GeofenceCard() {
  const queryClient = useQueryClient();
  const [showAdd, setShowAdd] = useState(false);

  const { data: locations = [] } = useQuery({
    queryKey: ['geofence-locations'],
    queryFn: listLocations,
  });

  const { data: modeData } = useQuery({
    queryKey: ['geofence-mode'],
    queryFn: getGeofenceMode,
  });

  const modeMut = useMutation({
    mutationFn: (mode: string) => setGeofenceMode(mode),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['geofence-mode'] }),
  });

  const currentMode = modeData?.mode ?? 'none';

  return (
    <div className="bg-white rounded-2xl shadow">
      <div className="p-6 border-b border-gray-100">
        <div className="flex items-center gap-2 mb-1">
          <Shield className="w-5 h-5 text-gray-700" />
          <h2 className="font-semibold text-gray-900">Geofencing</h2>
        </div>
        <p className="text-sm text-gray-500">
          Restrict or flag check-ins based on employee proximity to office locations.
        </p>
      </div>

      <div className="p-6 space-y-5">
        {/* Mode selector */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">Enforcement Mode</label>
          <div className="grid grid-cols-3 gap-2">
            {MODE_OPTIONS.map(opt => (
              <button
                key={opt.value}
                onClick={() => modeMut.mutate(opt.value)}
                className={`p-3 rounded-xl border-2 text-left transition-all ${
                  currentMode === opt.value
                    ? 'border-black bg-gray-50'
                    : 'border-gray-200 hover:border-gray-300'
                }`}
              >
                <p className="text-sm font-semibold text-gray-900">{opt.label}</p>
                <p className="text-xs text-gray-500 mt-0.5">{opt.desc}</p>
                {currentMode === opt.value && (
                  <CheckCircle2 className="w-3.5 h-3.5 text-black mt-1.5" />
                )}
              </button>
            ))}
          </div>
        </div>

        {/* Locations */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm font-medium text-gray-700">Office Locations</label>
            <button
              onClick={() => setShowAdd(true)}
              className="flex items-center gap-1 text-xs text-gray-600 hover:text-black font-medium bg-gray-100 hover:bg-gray-200 px-2.5 py-1.5 rounded-lg transition-colors"
            >
              <Plus className="w-3.5 h-3.5" /> Add
            </button>
          </div>

          {showAdd && <AddLocationForm onClose={() => setShowAdd(false)} />}

          {locations.length === 0 && !showAdd ? (
            <p className="text-xs text-gray-400 py-4 text-center">
              No office locations configured.{currentMode !== 'none' && ' Add one to enable geofencing.'}
            </p>
          ) : (
            <div className="divide-y divide-gray-100">
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
