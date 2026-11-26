/** Builds the sandboxed iframe `srcDoc` HTML document for letter previews. */
export function buildLetterPreviewSrcDoc(bodyHtml: string): string {
  return `<!doctype html><html><head><meta name="color-scheme" content="light"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src https: data:;"><style>body{margin:16px;font-family:Inter,system-ui,-apple-system,sans-serif;font-size:14px;line-height:1.6;color:#111827;background:#ffffff;}</style></head><body>${bodyHtml}</body></html>`;
}
