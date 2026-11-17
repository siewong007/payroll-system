import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle, AlertTriangle, CheckCircle2, Info, Plus, ShieldCheck, Sparkles, Trash2, Wifi,
} from 'lucide-react';
import {
  approveCandidate, createNetwork, deleteNetwork, dismissCandidate, getNetworkMode,
  getNetworkWhoami, listCandidates, listNetworks, setNetworkMode, toCidr,
  type CompanyNetwork, type NetworkCandidate, type NetworkMode,
} from '@/api/attendanceNetworks';

const QUERY_KEYS = {
  networks: ['attendance-networks'],
  mode: ['attendance-network-mode'],
  candidates: ['attendance-network-candidates'],
  whoami: ['attendance-network-whoami'],
};

import { getErrorMessage, translateServerMessage } from '@/lib/utils';
import type { TFunction } from 'i18next';

const MODE_VALUES: NetworkMode[] = ['none', 'learn', 'warn', 'enforce'];

function modeLabel(mode: NetworkMode, t: TFunction) {
  return t(`attendance.network.modes.${mode}.label`, { defaultValue: mode });
}
function modeDesc(mode: NetworkMode, t: TFunction) {
  return t(`attendance.network.modes.${mode}.desc`, { defaultValue: '' });
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
  const { t } = useTranslation();
  const { data, isLoading } = useQuery({ queryKey: QUERY_KEYS.whoami, queryFn: getNetworkWhoami });
  const queryClient = useQueryClient();
  const [label, setLabel] = useState(t('attendance.network.thisNetwork'));
  const [error, setError] = useState('');

  const approve = useMutation({
    mutationFn: () => createNetwork({ label, cidr: data?.suggested_cidr ?? '' }),
    onSuccess: () => {
      setError('');
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.networks });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.whoami });
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.network.approveFailed'))),
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
          <p className="text-xs font-medium text-gray-500">{t('attendance.network.reachesAs')}</p>
          <p className="text-sm font-mono font-semibold text-gray-900 truncate">
            {data.client_ip ?? t('attendance.network.unknown')}
          </p>
        </div>
        {data.is_approved ? (
          <span className="inline-flex items-center gap-1 rounded-full bg-emerald-100 px-2.5 py-1 text-[11px] font-medium text-emerald-700 shrink-0">
            <CheckCircle2 className="w-3 h-3" /> {data.matched_label ?? t('attendance.network.approved')}
          </span>
        ) : (
          <span className="inline-flex items-center gap-1 rounded-full bg-gray-200 px-2.5 py-1 text-[11px] font-medium text-gray-600 shrink-0">
            {t('attendance.network.notApproved')}
          </span>
        )}
      </div>

      {looksMisconfigured && (
        <p className="flex items-start gap-1.5 text-xs text-red-600">
          <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
          <span>
            <Trans
              i18nKey="attendance.network.privateWarning"
              components={{ code: <code /> }}
            />
          </span>
        </p>
      )}

      {!data.is_approved && data.suggested_cidr && !looksMisconfigured && (
        <div className="space-y-2">
          <p className="text-xs text-gray-500">
            {t('attendance.network.approveHint', { cidr: data.suggested_cidr })}
          </p>
          <div className="flex gap-2">
            <input
              value={label}
              onChange={e => setLabel(e.target.value)}
              placeholder={t('attendance.network.namePlaceholder')}
              aria-label={t('attendance.network.namePlaceholder')}
              className="form-input flex-1 min-w-0"
            />
            <button
              onClick={() => approve.mutate()}
              disabled={!label.trim() || approve.isPending}
              className="btn-primary !py-2 !min-h-9 shrink-0"
            >
              {approve.isPending ? t('attendance.network.approving') : t('attendance.network.approveThis')}
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
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ label: '', cidr: '' });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => createNetwork({ label: form.label, cidr: form.cidr }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.networks });
      onClose();
    },
    onError: (e) => setError(getErrorMessage(e, t('attendance.network.addFailed'))),
  });

  return (
    <div className="border border-gray-200 dark:border-slate-800 rounded-xl p-4 space-y-3 bg-gray-50/70 dark:bg-slate-900/60">
      <div className="flex justify-between items-center">
        <p className="text-sm font-semibold text-gray-700 dark:text-slate-200">{t('attendance.network.addTitle')}</p>
        <button onClick={onClose} aria-label={t('common.close')} className="text-gray-400 hover:text-gray-600 text-lg leading-none">&times;</button>
      </div>
      <input
        placeholder={t('attendance.network.namePlaceholder')}
        aria-label={t('attendance.network.namePlaceholder')}
        value={form.label}
        onChange={e => setForm(p => ({ ...p, label: e.target.value }))}
        className="form-input"
      />
      <input
        placeholder={t('attendance.network.cidrPlaceholder')}
        aria-label={t('attendance.network.cidrPlaceholder')}
        value={form.cidr}
        onChange={e => setForm(p => ({ ...p, cidr: e.target.value }))}
        className="form-input font-mono"
      />
      <p className="text-xs text-gray-500 dark:text-slate-400">
        <Trans
          i18nKey="attendance.network.publicHint"
          components={{ strong: <strong /> }}
        />
      </p>
      {error && (
        <p className="text-sm text-red-600 dark:text-red-400 flex items-center gap-1">
          <AlertCircle className="w-3.5 h-3.5" />{error}
        </p>
      )}
      <div className="flex gap-2">
        <button onClick={onClose}
          className="btn-secondary flex-1 !py-2">
          {t('common.cancel')}
        </button>
        <button
          onClick={() => mutation.mutate()}
          disabled={!form.label.trim() || !form.cidr.trim() || mutation.isPending}
          className="btn-primary flex-1 !py-2"
        >
          {mutation.isPending ? t('common.adding') : t('common.add')}
        </button>
      </div>
    </div>
  );
}

function NetworkRow({ net }: { net: CompanyNetwork }) {
  const { t } = useTranslation();
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
            <span className="ml-1.5 text-[10px] font-normal text-gray-400">{t('attendance.network.learned')}</span>
          )}
        </p>
        <p className="text-xs text-gray-400 font-mono">{toCidr(net)}</p>
      </div>
      {deleteMut.isError && (
        <span className="text-[11px] text-red-600 max-w-[45%] text-right">
          {getErrorMessage(deleteMut.error, t('attendance.network.removeFailed'))}
        </span>
      )}
      <button
        onClick={() => deleteMut.mutate()}
        disabled={deleteMut.isPending}
        className="text-gray-300 hover:text-red-500 p-1 transition-colors"
        title={t('attendance.network.remove')}
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </div>
  );
}

function CandidateRow({ candidate }: { candidate: NetworkCandidate }) {
  const { t } = useTranslation();
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
    onError: (e) => setError(getErrorMessage(e, t('attendance.network.approveFailedShort'))),
  });
  const dismiss = useMutation({
    mutationFn: () => dismissCandidate(cidr),
    onSuccess: refresh,
    onError: (e) => setError(getErrorMessage(e, t('attendance.network.dismissFailed'))),
  });

  return (
    <div className="py-3 px-1 space-y-2">
      <div className="flex items-start gap-3">
        <Sparkles className={`w-4 h-4 shrink-0 mt-0.5 ${candidate.is_anchored ? 'text-amber-500' : 'text-gray-300'}`} />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-mono font-medium text-gray-900 truncate">{cidr}</p>
          <p className="text-xs text-gray-500">
            {t('attendance.network.employees', { count: candidate.distinct_employees })}
            {' · '}
            {t('attendance.network.checkins', { count: candidate.observation_count })}
            {candidate.anchored_count > 0 && (
              <span className="text-emerald-600"> · {t('attendance.network.corroborated', { count: candidate.anchored_count })}</span>
            )}
          </p>
          {candidate.blocked_reason && (
            <p className="text-[11px] text-gray-400 mt-0.5">{translateServerMessage(candidate.blocked_reason)}</p>
          )}
        </div>
        {candidate.is_proposable && !approving && (
          <button
            onClick={() => setApproving(true)}
            className="text-xs font-medium text-gray-700 hover:text-black bg-gray-100 hover:bg-gray-200 px-2.5 py-1.5 rounded-lg shrink-0"
          >
            {t('attendance.network.approve')}
          </button>
        )}
        <button
          onClick={() => dismiss.mutate()}
          disabled={dismiss.isPending}
          className="text-xs text-gray-400 hover:text-gray-700 px-1.5 py-1.5 shrink-0"
          title={t('attendance.network.dismissTitle')}
        >
          {t('attendance.network.dismiss')}
        </button>
      </div>

      {approving && (
        <div className="flex gap-2 pl-7">
          <input
            autoFocus
            value={label}
            onChange={e => setLabel(e.target.value)}
            placeholder={t('attendance.network.nameThis')}
            aria-label={t('attendance.network.nameThis')}
            className="form-input flex-1 min-w-0"
          />
          <button
            onClick={() => approve.mutate()}
            disabled={!label.trim() || approve.isPending}
            className="btn-primary !py-2 !min-h-9 shrink-0"
          >
            {approve.isPending ? t('common.saving') : t('common.save')}
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
  const { t } = useTranslation();
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
    onError: (e) => setModeError(getErrorMessage(e, t('attendance.network.modeFailed'))),
  });

  // Unknown mode falls back to off: an unrecognised value must never start
  // blocking check-ins.
  const currentMode = modeData?.mode ?? 'none';
  const activeCount = networks.filter(n => n.is_active).length;

  return (
    <div className="card !p-0 overflow-hidden">
      <div className="p-5 sm:p-6 border-b border-gray-100 dark:border-slate-800">
        <div className="flex items-center gap-2 mb-1">
          <ShieldCheck className="w-5 h-5 text-[var(--accent-1)]" />
          <h2 className="font-semibold text-gray-900 dark:text-slate-100">{t('attendance.network.title')}</h2>
        </div>
        <p className="text-sm text-gray-500 dark:text-slate-400">
          {t('attendance.network.body')}
        </p>
      </div>

      <div className="p-5 sm:p-6 space-y-5">
        <WhoamiPanel />

        <div>
          <p className="block text-sm font-medium text-gray-700 dark:text-slate-300 mb-2">{t('attendance.geofence.modeLabel')}</p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {MODE_VALUES.map(mode => (
              <button
                key={mode}
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
                  <CheckCircle2 className="w-3.5 h-3.5 text-[var(--accent-1)] absolute top-3 right-3" />
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
              {t('attendance.network.learnHint')}
            </p>
          )}
          {/* Shown against Enforce specifically: this is the moment somebody
              decides to rely on the control, and it is the moment they should
              read what it does not prove. */}
          {currentMode === 'enforce' && (
            <div className="mt-2 flex items-start gap-1.5 rounded-lg bg-gray-50 px-3 py-2 text-xs text-gray-600">
              <Info className="w-3.5 h-3.5 mt-0.5 shrink-0 text-gray-400" />
              <span>
                <Trans
                  i18nKey="attendance.network.enforceProof"
                  components={{ strong: <strong className="font-medium text-gray-700" /> }}
                />
              </span>
            </div>
          )}
        </div>

        {candidates.length > 0 && (
          <div>
            <label className="text-sm font-medium text-gray-700">{t('attendance.network.suggestedTitle')}</label>
            <p className="text-xs text-gray-500 mb-1">
              {t('attendance.network.suggestedBody')}
            </p>
            <div className="divide-y divide-gray-100">
              {candidates.map(c => <CandidateRow key={toCidr(c)} candidate={c} />)}
            </div>
          </div>
        )}

        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm font-medium text-gray-700">{t('attendance.network.approvedTitle')}</label>
            <button
              onClick={() => setShowAdd(true)}
              className="flex items-center gap-1 text-xs text-gray-600 hover:text-black font-medium bg-gray-100 hover:bg-gray-200 px-2.5 py-1.5 rounded-lg transition-colors"
            >
              <Plus className="w-3.5 h-3.5" /> {t('common.add')}
            </button>
          </div>

          {showAdd && <AddNetworkForm onClose={() => setShowAdd(false)} />}

          {networks.length === 0 && !showAdd ? (
            <p className="text-xs text-gray-400 py-4 text-center">
              {t('attendance.network.empty')}
              {currentMode !== 'none' && ` ${t('attendance.network.emptyHint')}`}
            </p>
          ) : (
            <div className="divide-y divide-gray-100">
              {networks.map(net => <NetworkRow key={net.id} net={net} />)}
            </div>
          )}

          {currentMode === 'enforce' && activeCount === 1 && (
            <p className="mt-2 flex items-start gap-1.5 text-xs text-amber-700">
              <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
              {t('attendance.network.singleEnforceWarning')}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
