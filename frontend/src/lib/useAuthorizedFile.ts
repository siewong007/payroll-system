import { useEffect, useState } from 'react';

import { fetchUploadObjectUrl, isManagedUploadUrl } from '@/api/uploads';

export type AuthorizedFile =
  | { status: 'loading' }
  | { status: 'ready'; href: string }
  | { status: 'error' };

/**
 * Resolves a stored attachment URL to something the DOM can load.
 *
 * Files the API serves are authorized per caller and need the bearer token, so
 * they are fetched through the axios client and handed back as an object URL.
 * Anything else — an externally hosted document, or the `blob:` placeholder a
 * failed upload leaves behind — passes through untouched.
 *
 * The object URL is revoked when the URL changes or the component unmounts; a
 * response that lands after either is revoked immediately rather than leaked.
 */
export function useAuthorizedFile(url: string | null | undefined): AuthorizedFile {
  const [state, setState] = useState<AuthorizedFile>({ status: 'loading' });

  useEffect(() => {
    if (!url) {
      setState({ status: 'error' });
      return;
    }

    if (!isManagedUploadUrl(url)) {
      setState({ status: 'ready', href: url });
      return;
    }

    let cancelled = false;
    let objectUrl: string | null = null;
    setState({ status: 'loading' });

    fetchUploadObjectUrl(url)
      .then((created) => {
        if (cancelled) {
          URL.revokeObjectURL(created);
          return;
        }
        objectUrl = created;
        setState({ status: 'ready', href: created });
      })
      .catch(() => {
        if (!cancelled) setState({ status: 'error' });
      });

    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [url]);

  return state;
}
