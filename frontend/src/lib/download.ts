function isIosTouchDevice(): boolean {
  if (typeof navigator === 'undefined') return false;
  return (
    /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1) ||
    Boolean((navigator as { standalone?: boolean }).standalone)
  );
}

/**
 * Trigger a browser download for a binary/CSV Blob and defer object-URL
 * revocation so Safari/Firefox/iOS never race the download stream to empty.
 */
export function triggerBlobDownload(
  data: BlobPart,
  filename: string,
  mimeType?: string,
): void {
  const blob =
    data instanceof Blob
      ? mimeType && !data.type
        ? new Blob([data], { type: mimeType })
        : data
      : new Blob([data], mimeType ? { type: mimeType } : undefined);
  const url = window.URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.rel = 'noopener noreferrer';
  const ios = isIosTouchDevice();
  if (ios) {
    a.target = '_blank';
  }
  a.style.display = 'none';
  document.body?.appendChild(a);
  a.click();
  if (typeof a.remove === 'function') {
    a.remove();
  }
  window.setTimeout(() => window.URL.revokeObjectURL(url), ios ? 10_000 : 1_000);
}
