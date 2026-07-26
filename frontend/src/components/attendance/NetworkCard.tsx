import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle, AlertTriangle, CheckCircle2, Plus, ShieldCheck, Sparkles, Trash2, Wifi,
} from 'lucide-react';
import {
  approveCandidate, createNetwork, deleteNetwork, dismissCandidate, getNetworkMode,
  getNetworkWhoami, listCandidates, listNetworks, setNetworkMode, toCidr,
  type CompanyNetwork, type NetworkCandidate, type NetworkMode,
} from '@/api/attendanceNetworks';

const MODE_OPTIONS: { value: NetworkMode; label: string; desc: string }[] = [
  { value: 'none', label: 'Off', desc: 'No network check on check-in' },
  { value: 'learn', label: 'Learn', desc: 'Watch which networks are used — never blocks' },
  { value: 'warn', label: 'Warn', desc: 'Allow check-in but flag it if off-network' },
  { value: 'enforce', label: 'Enforce', desc: 'Block check-in from outside approved networks' },
];

const QUERY_KEYS = {
  networks: ['attendance-networks'],
  mode: ['attendance-network-mode'],
  candidates: ['attendance-network-candidates'],
  whoami: ['attendance-network-whoami'],
};

function errorText(e: unknown, fallback: string): string {
  const err = e as { response?: { data?: { error?: string } } };
  return err.response?.data?.error || fallback;
}

/**
 * What the server sees this browser coming from.
 *
 * The whole feature rests on this one value, so it is shown rather than
 * assumed: an administrator sitting on the office Wi-Fi reads the address here
 * and approves it, and a deployment where the API is reached without its proxy
 * is visible immediately instead of silently trusting a private address.
 */
function WhoamiPanel() {
  const { data, isLoading } = useQuery({ queryKey: QUERY_KEYS.whoami, queryFn: getNetworkWhoami });
  const queryClient = useQueryClient();
  const [label, setLabel] = useState('This network');
  const [error, setError] = useState('');

  const approve = useMutation({
    mutationFn: () => createNetwork({ label, cidr: data?.suggested_cidr ?? '' }),
    onSuccess: () => {
      setError('');
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.networks });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.whoami });
    },
    onError: (e) => setError(errorText(e, 'Could not approve this network')),
  });

  if (isLoading || !data) {
    return <div className="h-16 rounded-xl bg-gray-50 animate-pulse" />;
  }

  // A private address while forwarded headers are trusted means the request did
  // not come through the proxy that is supposed to be the only way in. Every
  // client would resolve to the same address, so the control is meaningless
  // until it is fixed — say so loudly rather than let someone approve it.
  const looksMisconfigured =
    !!data.client_ip && /^(10\.|127\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|::1|f[cd])/i.test(data.client_ip);

  return (
    <div className="rounded-xl border border-gray-200 bg-gray-50 p-4 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-xs font-medium text-gray-500">This browser reaches the server as</p>
          <p className="text-sm font-mono font-semibold text-gray-900 truncate">
            {data.client_ip ?? 'unknown'}
          </p>
        </div>
        {data.is_approved ? (
          <span className="inline-flex items-center gap-1 rounded-full bg-emerald-100 px-2.5 py-1 text-[11px] font-medium text-emerald-700 shrink-0">
            <CheckCircle2 className="w-3 h-3" /> {data.matched_label ?? 'Approved'}
          </span>
        ) : (
          <span className="inline-flex items-center gap-1 rounded-full bg-gray-200 px-2.5 py-1 text-[11px] font-medium text-gray-600 shrink-0">
            Not approved
          </span>
        )}
      </div>

      {looksMisconfigured && (
        <p className="flex items-start gap-1.5 text-xs text-red-600">
          <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
          <span>
            That is a private address, so the server is seeing its own proxy rather than the
            employee. Approving it would let anyone check in from anywhere. Check that requests
            reach the API only through CloudFront and that <code>TRUST_PROXY_HEADERS</code> matches.
          </span>
        </p>
      )}

      {!data.is_approved && data.suggested_cidr && !looksMisconfigured && (
        <div className="space-y-2">
          <p className="text-xs text-gray-500">
            If you are on the office Wi-Fi right now, approve{' '}
            <span className="font-mono">{data.suggested_cidr}</span>.
          </p>
          <div className="flex gap-2">
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              placeholder="Name (e.g. HQ Wi-Fi)"
              className="flex-1 min-w-0 px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
            />
            <button
              onClick={() => approve.mutate()}
              disabled={!label.trim() || approve.isPending}
              className="px-3 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-40 shrink-0"
            >
              {approve.isPending ? 'Approving…' : 'Approve this'}
            </button>
          </div>
        </div>
      )}

      {error && (
        <p className="text-sm text-red-600 flex items-center gap-1">
          <AlertCircle className="w-3.5 h-3.5" />{error}
        </p>
      )}
    </div>
  );
}

function AddNetworkForm({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ label: '', cidr: '' });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => createNetwork({ label: form.label, cidr: form.cidr }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.networks });
      onClose();
    },
    onError: (e) => setError(errorText(e, 'Failed to add network')),
  });

  return (
    <div className="border border-gray-200 rounded-xl p-4 space-y-3 bg-gray-50">
      <div className="flex justify-between items-center">
        <p className="text-sm font-semibold text-gray-700">Add Office Network</p>
        <button onClick={onClose} className="text-gray-400 hover:text-gray-600 text-lg leading-none">&times;</button>
      </div>
      <input
        placeholder="Name (e.g. HQ Wi-Fi)"
        value={form.label}
        onChange={e => setForm(p => ({ ...p, label: e.target.value }))}
        className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
      />
      <input
        placeholder="Public address or range (e.g. 203.0.113.5 or 203.0.113.0/24)"
        value={form.cidr}
        onChange={e => setForm(p => ({ ...p, cidr: e.target.value }))}
        className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono outline-none focus:ring-1 focus:ring-black"
      />
      <p className="text-xs text-gray-500">
        This is the office&rsquo;s <strong>public</strong> address as seen from the internet — not
        192.168.x.x. Ask your IT provider, or use the panel above while on the office Wi-Fi.
      </p>
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
          disabled={!form.label.trim() || !form.cidr.trim() || mutation.isPending}
          className="flex-1 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-40"
        >
          {mutation.isPending ? 'Adding...' : 'Add'}
        </button>
      </div>
    </div>
  );
}

function NetworkRow({ net }: { net: CompanyNetwork }) {
  const queryClient = useQueryClient();
  const deleteMut = useMutation({
    mutationFn: () => deleteNetwork(net.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.networks }),
  });

  return (
    <div className="flex items-center gap-3 py-2.5 px-1">
      <Wifi className={`w-4 h-4 shrink-0 ${net.is_active ? 'text-sky-500' : 'text-gray-300'}`} />
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-gray-900 truncate">
          {net.label}
          {net.learned_from_observation && (
            <span className="ml-1.5 text-[10px] font-normal text-gray-400">learned</span>
          )}
        </p>
        <p className="text-xs text-gray-400 font-mono">{toCidr(net)}</p>
      </div>
      {deleteMut.isError && (
        <span className="text-[11px] text-red-600 max-w-[45%] text-right">
          {errorText(deleteMut.error, 'Could not remove')}
        </span>
      )}
      <button
        onClick={() => deleteMut.mutate()}
        disabled={deleteMut.isPending}
        className="text-gray-300 hover:text-red-500 p-1 transition-colors"
        title="Remove network"
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </div>
  );
}

function CandidateRow({ candidate }: { candidate: NetworkCandidate }) {
  const queryClient = useQueryClient();
  const cidr = toCidr(candidate);
  const [label, setLabel] = useState('');
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState('');

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.candidates });
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.networks });
  };

  const approve = useMutation({
    mutationFn: () => approveCandidate({ cidr, label }),
    onSuccess: () => { setApproving(false); refresh(); },
    onError: (e) => setError(errorText(e, 'Could not approve')),
  });
  const dismiss = useMutation({
    mutationFn: () => dismissCandidate(cidr),
    onSuccess: refresh,
    onError: (e) => setError(errorText(e, 'Could not dismiss')),
  });

  return (
    <div className="py-3 px-1 space-y-2">
      <div className="flex items-start gap-3">
        <Sparkles className={`w-4 h-4 shrink-0 mt-0.5 ${candidate.is_anchored ? 'text-amber-500' : 'text-gray-300'}`} />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-mono font-medium text-gray-900 truncate">{cidr}</p>
          <p className="text-xs text-gray-500">
            {candidate.distinct_employees} employee{candidate.distinct_employees === 1 ? '' : 's'}
            {' · '}{candidate.observation_count} check-in{candidate.observation_count === 1 ? '' : 's'}
            {candidate.anchored_count > 0 && (
              <span className="text-emerald-600"> · {candidate.anchored_count} corroborated</span>
            )}
          </p>
          {candidate.blocked_reason && (
            <p className="text-[11px] text-gray-400 mt-0.5">{candidate.blocked_reason}</p>
          )}
        </div>
        {candidate.is_proposable && !approving && (
          <button
            onClick={() => setApproving(true)}
            className="text-xs font-medium text-gray-700 hover:text-black bg-gray-100 hover:bg-gray-200 px-2.5 py-1.5 rounded-lg shrink-0"
          >
            Approve
          </button>
        )}
        <button
          onClick={() => dismiss.mutate()}
          disabled={dismiss.isPending}
          className="text-xs text-gray-400 hover:text-gray-700 px-1.5 py-1.5 shrink-0"
          title="Never suggest this network again"
        >
          Dismiss
        </button>
      </div>

      {approving && (
        <div className="flex gap-2 pl-7">
          <input
            autoFocus
            value={label}
            onChange={e => setLabel(e.target.value)}
            placeholder="Name this network"
            className="flex-1 min-w-0 px-3 py-2 border border-gray-300 rounded-lg text-sm outline-none focus:ring-1 focus:ring-black"
          />
          <button
            onClick={() => approve.mutate()}
            disabled={!label.trim() || approve.isPending}
            className="px-3 py-2 bg-black text-white rounded-lg text-sm font-medium hover:bg-gray-800 disabled:opacity-40 shrink-0"
          >
            {approve.isPending ? 'Saving…' : 'Save'}
          </button>
        </div>
      )}

      {error && (
        <p className="text-xs text-red-600 flex items-center gap-1 pl-7">
          <AlertCircle className="w-3.5 h-3.5" />{error}
        </p>
      )}
    </div>
  );
}

export function NetworkCard() {
  const queryClient = useQueryClient();
  const [showAdd, setShowAdd] = useState(false);
  const [modeError, setModeError] = useState('');

  const { data: networks = [] } = useQuery({ queryKey: QUERY_KEYS.networks, queryFn: listNetworks });
  const { data: modeData } = useQuery({ queryKey: QUERY_KEYS.mode, queryFn: getNetworkMode });
  const { data: candidates = [] } = useQuery({
    queryKey: QUERY_KEYS.candidates,
    queryFn: listCandidates,
  });

  const modeMut = useMutation({
    mutationFn: (mode: NetworkMode) => setNetworkMode(mode),
    onSuccess: () => {
      setModeError('');
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.mode });
    },
    // The server refuses Enforce with an empty allow-list. Surfacing that
    // verbatim is the whole point — silently leaving the toggle unmoved would
    // read as a UI glitch.
    onError: (e) => setModeError(errorText(e, 'Could not change the mode')),
  });

  // Unknown mode falls back to off: an unrecognised value must never start
  // blocking check-ins.
  const currentMode = modeData?.mode ?? 'none';
  const activeCount = networks.filter(n => n.is_active).length;

  return (
    <div className="bg-white rounded-2xl shadow">
      <div className="p-5 sm:p-6 border-b border-gray-100">
        <div className="flex items-center gap-2 mb-1">
          <ShieldCheck className="w-5 h-5 text-gray-700" />
          <h2 className="font-semibold text-gray-900">Office Network</h2>
        </div>
        <p className="text-sm text-gray-500">
          Require employees to be on the company network to check in. The check uses the address
          the server sees, which a phone cannot fake — not the Wi-Fi name, which it can.
        </p>
      </div>

      <div className="p-5 sm:p-6 space-y-5">
        <WhoamiPanel />

        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">Enforcement Mode</label>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {MODE_OPTIONS.map(opt => (
              <button
                key={opt.value}
                onClick={() => modeMut.mutate(opt.value)}
                className={`relative p-3 rounded-xl border-2 text-left transition-all ${
                  currentMode === opt.value ? 'border-black bg-gray-50' : 'border-gray-200 hover:border-gray-300'
                }`}
              >
                <p className="text-sm font-semibold text-gray-900 pr-5">{opt.label}</p>
                <p className="text-xs text-gray-500 mt-0.5">{opt.desc}</p>
                {currentMode === opt.value && (
                  <CheckCircle2 className="w-3.5 h-3.5 text-black absolute top-3 right-3" />
                )}
              </button>
            ))}
          </div>
          {modeError && (
            <p className="mt-2 text-sm text-red-600 flex items-start gap-1">
              <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />{modeError}
            </p>
          )}
          {currentMode === 'learn' && (
            <p className="mt-2 text-xs text-gray-500">
              Learning. Nobody is blocked or flagged — run this for a week or two, then approve
              what it finds below.
            </p>
          )}
        </div>

        {candidates.length > 0 && (
          <div>
            <label className="text-sm font-medium text-gray-700">Suggested Networks</label>
            <p className="text-xs text-gray-500 mb-1">
              Seen in use, not yet trusted. Approving one is always your decision.
            </p>
            <div className="divide-y divide-gray-100">
              {candidates.map(c => <CandidateRow key={toCidr(c)} candidate={c} />)}
            </div>
          </div>
        )}

        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm font-medium text-gray-700">Approved Networks</label>
            <button
              onClick={() => setShowAdd(true)}
              className="flex items-center gap-1 text-xs text-gray-600 hover:text-black font-medium bg-gray-100 hover:bg-gray-200 px-2.5 py-1.5 rounded-lg transition-colors"
            >
              <Plus className="w-3.5 h-3.5" /> Add
            </button>
          </div>

          {showAdd && <AddNetworkForm onClose={() => setShowAdd(false)} />}

          {networks.length === 0 && !showAdd ? (
            <p className="text-xs text-gray-400 py-4 text-center">
              No approved networks yet.
              {currentMode !== 'none' && ' Until you approve one, check-ins are not restricted.'}
            </p>
          ) : (
            <div className="divide-y divide-gray-100">
              {networks.map(net => <NetworkRow key={net.id} net={net} />)}
            </div>
          )}

          {currentMode === 'enforce' && activeCount === 1 && (
            <p className="mt-2 flex items-start gap-1.5 text-xs text-amber-700">
              <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
              Only one approved network while enforcing. If the office address changes, nobody will
              be able to check in until you add the new one.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
