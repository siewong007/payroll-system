import { useState, useEffect, useRef } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  QrCode, Fingerprint, LogIn, LogOut, CheckCircle2, Clock,
  MapPin, Calendar, AlertCircle, ExternalLink,
} from 'lucide-react';
import { Html5Qrcode } from 'html5-qrcode';
import {
  getAttendanceMethod,
  getMyTodayAttendance,
  getMyAttendance,
  checkOut,
  checkInFaceId,
  type AttendanceRecord,
} from '@/api/attendance';

function formatTime(iso: string | null) {
  if (!iso) return '—';
  return new Date(iso).toLocaleTimeString('en-MY', { hour: '2-digit', minute: '2-digit' });
}
function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString('en-MY', { day: 'numeric', month: 'short', year: 'numeric' });
}

function getGeolocation(): Promise<GeolocationCoordinates | null> {
  return new Promise((resolve) => {
    if (!navigator.geolocation) { resolve(null); return; }
    navigator.geolocation.getCurrentPosition(
      (pos) => resolve(pos.coords),
      () => resolve(null),
      { timeout: 8000, maximumAge: 0 }
    );
  });
}

// ─── QR Scanner Modal ─────────────────────────────────────────────────────────

function QrScannerModal({ onClose, onScanned }: { onClose: () => void; onScanned: (token: string) => void }) {
  const [error, setError] = useState('');
  const [started, setStarted] = useState(false);
  const scannerRef = useRef<Html5Qrcode | null>(null);

  useEffect(() => {
    let stopped = false;
    
    const initScanner = async () => {
      try {
        // Wait a small bit to ensure DOM is ready and styled
        await new Promise(r => setTimeout(r, 300));
        
        if (stopped) return;

        if (!scannerRef.current) {
          scannerRef.current = new Html5Qrcode('qr-reader');
        }
        const qr = scannerRef.current;
        
        const config = {
          fps: 20,
          qrbox: { width: 280, height: 280 },
          aspectRatio: 1.0,
        };
        const successCb = (decodedText: string) => {
          if (stopped) return;
          stopped = true;
          qr.stop().then(() => {
            try {
              const url = new URL(decodedText);
              const token = url.searchParams.get('token');
              if (token) {
                onScanned(token);
              } else {
                setError('Invalid QR code. Please scan the attendance kiosk QR.');
              }
            } catch {
              setError('Invalid QR code format.');
            }
          }).catch(console.error);
        };
        const errorCb = (_err: any) => { /* scanning in progress */ };

        const constraints = {
          width: { min: 640, ideal: 1280 },
          height: { min: 480, ideal: 720 },
        };

        try {
          // Try 1: HD constraints + Environment camera
          await qr.start({ ...constraints, facingMode: 'environment' }, config, successCb, errorCb);
        } catch (err1) {
          console.warn('HD environment camera failed, trying basic environment camera', err1);
          try {
            // Try 2: Basic environment camera (no resolution constraints)
            await qr.start({ facingMode: 'environment' }, config, successCb, errorCb);
          } catch (err2) {
            console.warn('Environment camera failed, trying user camera', err2);
            try {
              // Try 3: User (front) camera
              await qr.start({ facingMode: 'user' }, config, successCb, errorCb);
            } catch (err3) {
              console.error('All camera attempts failed', err3);
              throw new Error('Could not access camera. Please ensure camera permissions are granted.');
            }
          }
        }
        
        if (!stopped) setStarted(true);
      } catch (err: any) {
        console.error('QR Scanner error:', err);
        setError(err?.message || 'Camera access denied or device not supported.');
      }
    };
    
    initScanner();

    return () => {
      stopped = true;
      if (scannerRef.current) {
        const qr = scannerRef.current;
        if (qr.isScanning) {
          qr.stop().then(() => qr.clear()).catch(console.error);
        } else {
          qr.clear();
        }
      }
    };
  }, [onScanned]);

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-black/70 p-4">
      <div className="bg-white rounded-3xl w-full max-w-sm overflow-hidden">
        <div className="p-5 border-b border-gray-100 flex items-center justify-between">
          <h3 className="font-semibold text-gray-900 flex items-center gap-2">
            <QrCode className="w-5 h-5 text-violet-600" />
            Scan Attendance QR
          </h3>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-700 text-xl leading-none transition-colors">&times;</button>
        </div>

        <div className="p-5">
          <div className="relative h-64 bg-gray-50 rounded-2xl overflow-hidden mb-4 border border-gray-100">
            {!started && !error && (
              <div className="absolute inset-0 flex items-center justify-center z-10">
                <div className="animate-spin rounded-full h-10 w-10 border-b-2 border-black" />
              </div>
            )}
            
            {error && (
              <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-50 gap-3 z-20">
                <AlertCircle className="w-10 h-10 text-red-400" />
                <p className="text-sm text-red-600 text-center px-4">{error}</p>
              </div>
            )}
            
            <div id="qr-reader" className="w-full h-full" />
          </div>

          <p className="text-xs text-gray-400 text-center">
            Point your camera at the QR code on the kiosk screen
          </p>
        </div>
      </div>
    </div>
  );
}

// ─── Today Status Card ────────────────────────────────────────────────────────

function TodayCard({
  record,
  method,
  onCheckIn,
  onCheckOut,
  isCheckingIn,
  isCheckingOut,
}: {
  record: AttendanceRecord | null;
  method: string;
  onCheckIn: () => void;
  onCheckOut: () => void;
  isCheckingIn: boolean;
  isCheckingOut: boolean;
}) {
  const now = new Date();
  const dateStr = now.toLocaleDateString('en-MY', { weekday: 'long', day: 'numeric', month: 'long' });
  const isCheckedIn = !!record && !record.check_out_at;

  return (
    <div className="bg-gradient-to-br from-gray-900 to-gray-800 rounded-3xl p-6 text-white">
      <div className="flex items-start justify-between mb-6">
        <div>
          <p className="text-gray-400 text-sm">{dateStr}</p>
          <h2 className="text-xl font-bold mt-0.5">
            {!record ? 'Not Checked In' : !record.check_out_at ? 'Checked In' : 'Checked Out'}
          </h2>
        </div>
        <div className={`w-12 h-12 rounded-2xl flex items-center justify-center ${
          !record ? 'bg-gray-700' : !record.check_out_at ? 'bg-emerald-500' : 'bg-blue-500'
        }`}>
          {!record ? <LogIn className="w-6 h-6 text-gray-300" /> :
           !record.check_out_at ? <CheckCircle2 className="w-6 h-6 text-white" /> :
           <LogOut className="w-6 h-6 text-white" />}
        </div>
      </div>

      {record && (
        <div className="flex gap-4 mb-6">
          <div>
            <p className="text-xs text-gray-400 mb-0.5">Check In</p>
            <p className="text-lg font-bold tabular-nums">{formatTime(record.check_in_at)}</p>
          </div>
          {record.check_out_at && (
            <div>
              <p className="text-xs text-gray-400 mb-0.5">Check Out</p>
              <p className="text-lg font-bold tabular-nums">{formatTime(record.check_out_at)}</p>
            </div>
          )}
          {record.latitude && record.longitude && (
            <div className="ml-auto">
              <a
                href={`https://maps.google.com/?q=${record.latitude},${record.longitude}`}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-1 text-sky-400 text-xs mt-4"
              >
                <MapPin className="w-3.5 h-3.5" />
                Location
                <ExternalLink className="w-3 h-3" />
              </a>
            </div>
          )}
        </div>
      )}

      {/* Action Button */}
      {!record ? (
        <button
          onClick={onCheckIn}
          disabled={isCheckingIn}
          className={`w-full py-3 rounded-2xl font-semibold text-sm flex items-center justify-center gap-2.5 transition-all
            ${isCheckingIn ? 'bg-gray-600 text-gray-400 cursor-not-allowed' : 'bg-white text-gray-900 hover:bg-gray-100 active:scale-95'}`}
        >
          {isCheckingIn ? (
            <><div className="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-400" /> Checking in…</>
          ) : method === 'qr_code' ? (
            <><QrCode className="w-4 h-4" /> Scan QR Code to Check In</>
          ) : (
            <><Fingerprint className="w-4 h-4" /> Face ID Check In</>
          )}
        </button>
      ) : isCheckedIn ? (
        <button
          onClick={onCheckOut}
          disabled={isCheckingOut}
          className={`w-full py-3 rounded-2xl font-semibold text-sm flex items-center justify-center gap-2.5 transition-all
            ${isCheckingOut ? 'bg-gray-600 text-gray-400 cursor-not-allowed' : 'bg-white/10 text-white border border-white/20 hover:bg-white/20 active:scale-95'}`}
        >
          {isCheckingOut ? (
            <><div className="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-300" /> Checking out…</>
          ) : (
            <><LogOut className="w-4 h-4" /> Check Out</>
          )}
        </button>
      ) : (
        <div className="w-full py-3 rounded-2xl bg-blue-500/20 text-blue-300 text-sm text-center font-medium">
          ✓ Attendance complete for today
        </div>
      )}
    </div>
  );
}

// ─── History List ─────────────────────────────────────────────────────────────

const STATUS_STYLE: Record<string, string> = {
  present:  'bg-emerald-100 text-emerald-700',
  late:     'bg-amber-100 text-amber-700',
  absent:   'bg-red-100 text-red-700',
  half_day: 'bg-blue-100 text-blue-700',
};

function HistoryList({ records }: { records: AttendanceRecord[] }) {
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
            <p className="text-sm font-medium text-gray-900">{formatDate(r.check_in_at)}</p>
            <div className="flex items-center gap-3 mt-0.5">
              <span className="text-xs text-gray-500 flex items-center gap-1">
                <LogIn className="w-3 h-3 text-emerald-400" />
                {formatTime(r.check_in_at)}
              </span>
              {r.check_out_at && (
                <span className="text-xs text-gray-500 flex items-center gap-1">
                  <LogOut className="w-3 h-3 text-gray-400" />
                  {formatTime(r.check_out_at)}
                </span>
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

// ─── Main ─────────────────────────────────────────────────────────────────────

export function MyAttendance() {
  const queryClient = useQueryClient();
  const [showScanner, setShowScanner] = useState(false);
  const [toast, setToast] = useState('');
  const [toastType, setToastType] = useState<'success' | 'error'>('success');

  const showToast = (msg: string, type: 'success' | 'error' = 'success') => {
    setToast(msg);
    setToastType(type);
    setTimeout(() => setToast(''), 3000);
  };

  const { data: methodData } = useQuery({
    queryKey: ['attendance-method'],
    queryFn: getAttendanceMethod,
  });

  const { data: todayData } = useQuery({
    queryKey: ['attendance-today'],
    queryFn: getMyTodayAttendance,
    refetchInterval: 60_000,
  });

  const { data: historyResult } = useQuery({
    queryKey: ['attendance-my'],
    queryFn: () => getMyAttendance({ per_page: 100 }),
  });
  const history = historyResult?.data ?? [];

  const checkOutMut = useMutation({
    mutationFn: async () => {
      const coords = await getGeolocation();
      return checkOut(coords?.latitude, coords?.longitude);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
      showToast('Checked out successfully!');
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      showToast(e.response?.data?.error || 'Check-out failed', 'error');
    },
  });

  const faceIdCheckInMut = useMutation({
    mutationFn: async () => {
      // Use the existing passkey authentication flow (discoverable)
      const beginResp = await fetch('/api/auth/passkey/discoverable/begin', {
        method: 'POST',
        credentials: 'include',
      });
      if (!beginResp.ok) throw new Error('Face ID not available');
      const { challenge, allowCredentials } = await beginResp.json();

      // WebAuthn get
      const cred = await navigator.credentials.get({
        publicKey: {
          challenge: Uint8Array.from(atob(challenge.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0)),
          allowCredentials: allowCredentials || [],
          timeout: 60000,
          userVerification: 'required',
        },
      }) as PublicKeyCredential;

      const coords = await getGeolocation();
      return checkInFaceId(
        cred.id,
        { id: cred.id },
        coords?.latitude,
        coords?.longitude
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attendance-today'] });
      queryClient.invalidateQueries({ queryKey: ['attendance-my'] });
      showToast('Checked in successfully with Face ID! ✓');
    },
    onError: (e: Error & { response?: { data?: { error?: string } } }) => {
      showToast(e.response?.data?.error || 'Face ID check-in failed', 'error');
    },
  });

  const handleCheckIn = () => {
    if (methodData?.method === 'qr_code') {
      setShowScanner(true);
    } else {
      faceIdCheckInMut.mutate();
    }
  };

  const handleQrScanned = async (token: string) => {
    setShowScanner(false);
    // Navigate to the scan page which handles the actual check-in
    window.location.href = `/attendance/scan?token=${token}`;
  };

  const method = methodData?.method ?? 'qr_code';
  const today = todayData?.record ?? null;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="page-title">My Attendance</h1>
        <p className="page-subtitle">Check in and track your daily attendance</p>
      </div>

      {/* Toast */}
      {toast && (
        <div className={`fixed top-4 left-1/2 -translate-x-1/2 z-50 px-5 py-3 rounded-2xl text-sm font-medium shadow-lg flex items-center gap-2 ${
          toastType === 'success' ? 'bg-emerald-600 text-white' : 'bg-red-600 text-white'
        }`}>
          {toastType === 'success' ? <CheckCircle2 className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
          {toast}
        </div>
      )}

      {/* Today's Card */}
      <TodayCard
        record={today}
        method={method}
        onCheckIn={handleCheckIn}
        onCheckOut={() => checkOutMut.mutate()}
        isCheckingIn={faceIdCheckInMut.isPending}
        isCheckingOut={checkOutMut.isPending}
      />

      {/* Method info */}
      <div className="bg-white rounded-2xl p-4 flex items-center gap-3 text-sm border border-gray-100">
        {method === 'qr_code' ? (
          <QrCode className="w-5 h-5 text-violet-600 shrink-0" />
        ) : (
          <Fingerprint className="w-5 h-5 text-sky-600 shrink-0" />
        )}
        <span className="text-gray-600">
          Your company uses <strong className="text-gray-800">{method === 'qr_code' ? 'QR Code' : 'Face ID'}</strong> for attendance.
          {method === 'qr_code' && ' Scan the QR code at the kiosk to check in.'}
        </span>
      </div>

      {/* History */}
      <div className="bg-white rounded-2xl shadow overflow-hidden">
        <div className="px-5 py-4 border-b border-gray-100 flex items-center gap-2">
          <Clock className="w-4 h-4 text-gray-400" />
          <h3 className="font-semibold text-gray-900 text-sm">Attendance History</h3>
        </div>
        <HistoryList records={history} />
      </div>

      {showScanner && (
        <QrScannerModal
          onClose={() => setShowScanner(false)}
          onScanned={handleQrScanned}
        />
      )}
    </div>
  );
}
