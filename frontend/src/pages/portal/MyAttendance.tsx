import { useQuery } from '@tanstack/react-query';
import { LogIn, LogOut, CheckCircle2, Clock, MapPin, Calendar, AlertCircle, RefreshCw } from 'lucide-react';
import { getAttendanceMethod, getMyAttendance, type AttendanceRecord } from '@/api/attendance';
import { CheckInCard } from '@/components/attendance/CheckInCard';
import { formatZonedTime } from '@/lib/attendance';

const FALLBACK_TZ = 'Asia/Kuala_Lumpur';

function formatDate(iso: string, timeZone: string) {
  try {
    return new Date(iso).toLocaleDateString('en-MY', { timeZone, day: 'numeric', month: 'short', year: 'numeric' });
  } catch {
    return new Date(iso).toLocaleDateString('en-MY', { day: 'numeric', month: 'short', year: 'numeric' });
  }
}

const STATUS_STYLE: Record<string, string> = {
  present:  'bg-emerald-100 text-emerald-700',
  late:     'bg-amber-100 text-amber-700',
  absent:   'bg-red-100 text-red-700',
  half_day: 'bg-blue-100 text-blue-700',
};

function HistoryList({
  records,
  timeZone,
  isError,
  onRetry,
}: {
  records: AttendanceRecord[];
  timeZone: string;
  isError: boolean;
  onRetry: () => void;
}) {
  // A failed fetch must not look like an empty history.
  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center h-40 text-gray-500 gap-2">
        <AlertCircle className="w-10 h-10 text-red-300" />
        <p className="text-sm">Couldn't load your attendance history.</p>
        <button
          onClick={onRetry}
          className="flex items-center gap-1.5 text-sm font-medium text-gray-900 hover:underline"
        >
          <RefreshCw className="w-3.5 h-3.5" /> Try again
        </button>
      </div>
    );
  }

  if (records.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-40 text-gray-400">
        <Calendar className="w-10 h-10 mb-2 opacity-40" />
        <p className="text-sm">No attendance records yet</p>
      </div>
    );
  }

  return (
    <div className="divide-y divide-gray-50">
      {records.map(r => (
        <div key={r.id} className="flex items-center gap-4 py-3.5 px-5">
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-gray-900">{formatDate(r.check_in_at, timeZone)}</p>
            <div className="flex items-center gap-3 mt-0.5">
              {/* An auto-absent placeholder carries a midnight timestamp that
                  never happened — showing "12:00 AM" reads as a real check-in. */}
              {r.status === 'absent' ? (
                <span className="text-xs text-gray-400">No attendance recorded</span>
              ) : (
                <>
                  <span className="text-xs text-gray-500 flex items-center gap-1">
                    <LogIn className="w-3 h-3 text-emerald-400" />
                    {formatZonedTime(r.check_in_at, timeZone)}
                  </span>
                  {r.check_out_at ? (
                    <span className="text-xs text-gray-500 flex items-center gap-1">
                      <LogOut className="w-3 h-3 text-gray-400" />
                      {formatZonedTime(r.check_out_at, timeZone)}
                    </span>
                  ) : (
                    <span className="text-xs text-amber-600 flex items-center gap-1">
                      <Clock className="w-3 h-3" /> Never checked out
                    </span>
                  )}
                  {r.hours_worked && (
                    <span className="text-xs text-gray-400 tabular-nums">{r.hours_worked}h</span>
                  )}
                </>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {r.latitude && r.longitude && (
              <a
                href={`https://maps.google.com/?q=${r.latitude},${r.longitude}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sky-500 hover:text-sky-700"
                title="View location"
              >
                <MapPin className="w-3.5 h-3.5" />
              </a>
            )}
            <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_STYLE[r.status] ?? 'bg-gray-100 text-gray-600'}`}>
              {r.status.replace('_', ' ')}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

export function MyAttendance() {
  const { data: method } = useQuery({ queryKey: ['attendance-method'], queryFn: getAttendanceMethod });
  const {
    data: historyResult,
    isError: historyError,
    refetch: refetchHistory,
  } = useQuery({
    queryKey: ['attendance-my'],
    queryFn: () => getMyAttendance({ per_page: 100 }),
  });

  const history = historyResult?.data ?? [];
  const timeZone = method?.timezone || FALLBACK_TZ;
  const thisMonth = history.filter(r => r.status !== 'absent' && r.check_out_at).length;

  return (
    <div className="space-y-6">
      <div className="page-header">
        <h1 className="page-title">My Attendance</h1>
        <p className="page-subtitle">Check in and track your daily attendance</p>
      </div>

      {/* The same card as the portal home — one implementation of check-in,
          so the two surfaces cannot drift apart. */}
      <CheckInCard />

      <div className="bg-white rounded-2xl shadow overflow-hidden">
        <div className="px-5 py-4 border-b border-gray-100 flex items-center gap-2">
          <Clock className="w-4 h-4 text-gray-400" />
          <h3 className="font-semibold text-gray-900 text-sm">Attendance History</h3>
          {!historyError && history.length > 0 && (
            <span className="ml-auto flex items-center gap-1.5 text-xs text-gray-400">
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
              {thisMonth} completed
            </span>
          )}
        </div>
        <HistoryList
          records={history}
          timeZone={timeZone}
          isError={historyError}
          onRetry={() => void refetchHistory()}
        />
      </div>
    </div>
  );
}
