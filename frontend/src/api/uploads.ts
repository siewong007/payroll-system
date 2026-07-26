import api from '@/api/client';

/**
 * The prefix the backend stores for a file it serves itself, from
 * `claims.receipt_url`, `leave_requests.attachment_url` and
 * `documents.file_url`.
 */
const UPLOAD_URL_PREFIX = '/api/uploads/';

/**
 * Whether the API serves this URL, and therefore whether reading it needs the
 * access token.
 *
 * `documents.file_url` is free text an administrator can point anywhere, so a
 * document may just as well reference a link on another host. Those are fetched
 * by the browser exactly as before.
 */
export function isManagedUploadUrl(url: string): boolean {
  return url.startsWith(UPLOAD_URL_PREFIX);
}

/**
 * Reads an uploaded file through the shared axios client and wraps it in an
 * object URL the DOM can point at.
 *
 * `GET /api/uploads/{filename}` is authorized per caller, and the access token
 * lives in memory rather than in a cookie — so a plain `<a href>` or `<img src>`
 * navigation carries no credentials and gets a 404. Going through the client
 * attaches the bearer token and inherits its refresh-on-401 handling.
 *
 * The caller owns the returned URL and must `URL.revokeObjectURL` it.
 */
export async function fetchUploadObjectUrl(url: string): Promise<string> {
  // The client's baseURL already ends in `/api`, which may be absolute when
  // VITE_API_URL is set. Strip the stored prefix rather than concatenating.
  const path = url.replace(/^\/api/, '');
  const { data } = await api.get<Blob>(path, { responseType: 'blob' });
  return URL.createObjectURL(data);
}
