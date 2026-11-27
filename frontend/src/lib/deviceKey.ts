import type { DeviceChallengePurpose, DeviceChallengeResponse, DeviceProofRequest } from '@/api/attendance';

const DB_NAME = 'payroll_attendance_device_v1';
const STORE_NAME = 'keys';
const RECORD_KEY = 'current';

interface StoredDeviceRecord {
  id: string;
  privateKey: CryptoKey;
  publicKey: CryptoKey;
  publicKeyRawB64Url: string;
  /** Maps `${companyId}:${employeeId}` (or `'default'`) to the backend device UUID. */
  enrolledByScope: Record<string, string>;
}

function toBase64Url(input: ArrayBuffer | Uint8Array): string {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function isDeviceKeySupported(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof indexedDB !== 'undefined' &&
    typeof window.crypto?.subtle?.generateKey === 'function'
  );
}

function openKeyDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'id' });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error ?? new Error('Failed to open device key store'));
  });
}

async function readStoredRecord(): Promise<StoredDeviceRecord | null> {
  if (!isDeviceKeySupported()) return null;
  const db = await openKeyDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const req = tx.objectStore(STORE_NAME).get(RECORD_KEY);
      req.onsuccess = () => resolve((req.result as StoredDeviceRecord | undefined) ?? null);
      req.onerror = () => reject(req.error ?? new Error('Failed to read device key'));
    });
  } finally {
    db.close();
  }
}

async function writeStoredRecord(record: StoredDeviceRecord): Promise<void> {
  if (!isDeviceKeySupported()) return;
  const db = await openKeyDb();
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      const req = tx.objectStore(STORE_NAME).put(record);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error ?? new Error('Failed to store device key'));
    });
  } finally {
    db.close();
  }
}

export async function getOrCreateDeviceKey(): Promise<{
  publicKeyRawB64Url: string;
  privateKey: CryptoKey;
  enrolledByScope: Record<string, string>;
}> {
  if (!isDeviceKeySupported()) {
    throw new Error('WebCrypto or IndexedDB is unavailable in this browser');
  }
  const existing = await readStoredRecord();
  if (existing?.privateKey && existing?.publicKeyRawB64Url) {
    return {
      publicKeyRawB64Url: existing.publicKeyRawB64Url,
      privateKey: existing.privateKey,
      enrolledByScope: existing.enrolledByScope ?? {},
    };
  }

  const pair = await window.crypto.subtle.generateKey(
    { name: 'ECDSA', namedCurve: 'P-256' },
    false,
    ['sign', 'verify'],
  );
  const rawPub = await window.crypto.subtle.exportKey('raw', pair.publicKey);
  const publicKeyRawB64Url = toBase64Url(rawPub);
  const record: StoredDeviceRecord = {
    id: RECORD_KEY,
    privateKey: pair.privateKey,
    publicKey: pair.publicKey,
    publicKeyRawB64Url,
    enrolledByScope: existing?.enrolledByScope ?? {},
  };
  await writeStoredRecord(record);
  return {
    publicKeyRawB64Url,
    privateKey: record.privateKey,
    enrolledByScope: record.enrolledByScope,
  };
}

export function deviceScopeKey(companyId?: string | null, employeeId?: string | null): string {
  if (companyId && employeeId) return `${companyId}:${employeeId}`;
  return 'default';
}

export async function getEnrolledDeviceId(scopeKey = 'default'): Promise<string | null> {
  try {
    const rec = await readStoredRecord();
    return rec?.enrolledByScope?.[scopeKey] ?? null;
  } catch {
    return null;
  }
}

export async function setEnrolledDeviceId(deviceId: string, scopeKey = 'default'): Promise<void> {
  const existing = await readStoredRecord();
  if (!existing) return;
  await writeStoredRecord({
    ...existing,
    enrolledByScope: {
      ...(existing.enrolledByScope ?? {}),
      [scopeKey]: deviceId,
    },
  });
}

export async function clearEnrolledDeviceId(scopeKey = 'default'): Promise<void> {
  try {
    const existing = await readStoredRecord();
    if (!existing) return;
    const next = { ...(existing.enrolledByScope ?? {}) };
    delete next[scopeKey];
    await writeStoredRecord({ ...existing, enrolledByScope: next });
  } catch {
    // Ignore storage cleanup errors
  }
}

export async function clearDeviceKey(): Promise<void> {
  if (!isDeviceKeySupported()) return;
  const db = await openKeyDb();
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      const req = tx.objectStore(STORE_NAME).delete(RECORD_KEY);
      req.onsuccess = () => resolve();
      req.onerror = () => reject(req.error ?? new Error('Failed to clear device key'));
    });
  } finally {
    db.close();
  }
}

export async function signDeviceChallenge(
  challenge: DeviceChallengeResponse,
  purpose: DeviceChallengePurpose,
  scopeKey = 'default',
  overrideDeviceId?: string | null,
): Promise<DeviceProofRequest | null> {
  if (!isDeviceKeySupported()) return null;
  const record = await readStoredRecord();
  if (!record?.privateKey) return null;
  const deviceId = overrideDeviceId ?? record.enrolledByScope?.[scopeKey];
  if (!deviceId) return null;

  const payload = new TextEncoder().encode(`payroll-att-device:v1:${purpose}:${challenge.nonce}`);
  const sig = await window.crypto.subtle.sign(
    { name: 'ECDSA', hash: 'SHA-256' },
    record.privateKey,
    payload,
  );
  return {
    device_id: deviceId,
    challenge_id: challenge.challenge_id,
    signature: toBase64Url(sig),
  };
}

export function detectDefaultDeviceLabel(): string {
  if (typeof navigator === 'undefined') return 'Browser device';
  const ua = navigator.userAgent;
  let os = 'Device';
  if (/iPhone/i.test(ua)) os = 'iPhone';
  else if (/iPad/i.test(ua)) os = 'iPad';
  else if (/Android/i.test(ua)) os = 'Android';
  else if (/Macintosh|Mac OS X/i.test(ua)) os = 'Mac';
  else if (/Windows/i.test(ua)) os = 'Windows';
  else if (/Linux/i.test(ua)) os = 'Linux';

  let browser = 'Browser';
  if (/Edg\//i.test(ua)) browser = 'Edge';
  else if (/Chrome|CriOS/i.test(ua)) browser = 'Chrome';
  else if (/Firefox|FxiOS/i.test(ua)) browser = 'Firefox';
  else if (/Safari/i.test(ua)) browser = 'Safari';

  return `${os} · ${browser}`;
}
