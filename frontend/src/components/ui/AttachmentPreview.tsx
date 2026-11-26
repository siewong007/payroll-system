import { useState } from 'react';
import { ExternalLink, FileText } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { fetchUploadObjectUrl, isManagedUploadUrl } from '@/api/uploads';
import { toast } from '@/lib/toast';
import { useAuthorizedFile } from '@/lib/useAuthorizedFile';
import { isImageUrl } from '@/lib/utils';

/**
 * Shared rendering for an uploaded claim receipt or leave attachment.
 *
 * This previously existed as two byte-identical copies, in the approvals
 * workbench and the portal's leave page. They are one component now because
 * both had to change together when `/api/uploads/{filename}` stopped being
 * readable without a token: the file is fetched through the API client and
 * shown from an object URL, never linked to directly.
 */

/** A file the uploader never successfully stored, recorded as a `blob:` URL. */
function UnavailableAttachment({ displayName, detail }: { displayName: string; detail: string }) {
  return (
    <div className="mt-1 inline-flex items-center gap-2 px-4 py-3 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 rounded-lg">
      <div className="w-10 h-10 rounded-lg bg-red-100 dark:bg-red-500/20 flex items-center justify-center shrink-0">
        <FileText className="w-5 h-5 text-red-500 dark:text-red-400" aria-hidden="true" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-red-700 dark:text-red-300 truncate">{displayName}</div>
        <div className="text-xs text-red-500 dark:text-red-300/80">{detail}</div>
      </div>
    </div>
  );
}

export function AttachmentPreview({ url, name }: { url: string; name: string | null }) {
  const { t } = useTranslation();
  const displayName = name || t('common.attachment');
  // Called unconditionally — a `blob:` URL resolves straight through, and the
  // placeholder below is chosen from the raw URL rather than by skipping it.
  const file = useAuthorizedFile(url);

  if (url.startsWith('blob:')) {
    return (
      <UnavailableAttachment
        displayName={displayName}
        detail={t('common.fileUnavailableUpload')}
      />
    );
  }

  if (file.status === 'error') {
    return (
      <UnavailableAttachment
        displayName={displayName}
        detail={t('common.fileUnavailableAccess')}
      />
    );
  }

  if (file.status === 'loading') {
    return (
      <div
        role="status"
        className="mt-1 inline-flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-slate-800/80 border border-gray-200 dark:border-slate-700 rounded-lg"
      >
        <div className="w-10 h-10 rounded-lg bg-gray-100 dark:bg-slate-700 flex items-center justify-center shrink-0">
          <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-500 dark:border-slate-300" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-medium text-gray-600 dark:text-slate-200 truncate">{displayName}</div>
          <div className="text-xs text-gray-500 dark:text-slate-400">{t('common.loading')}</div>
        </div>
      </div>
    );
  }

  const { href } = file;

  return (
    <div className="mt-1">
      {isImageUrl(url) ? (
        <div className="space-y-2">
          <a href={href} target="_blank" rel="noopener noreferrer" className="block">
            <img
              src={href}
              alt={displayName}
              loading="lazy"
              decoding="async"
              className="max-w-full max-h-64 rounded-lg border border-gray-200 dark:border-slate-700 object-contain bg-gray-50 dark:bg-slate-800/60"
            />
          </a>
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex min-h-10 sm:min-h-0 items-center gap-1.5 text-sm font-medium text-gray-800 dark:text-slate-200 hover:text-black dark:hover:text-white transition-colors"
          >
            <ExternalLink className="w-3.5 h-3.5" aria-hidden="true" />
            {t('common.openFullSize')}
          </a>
        </div>
      ) : (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-slate-800/80 border border-gray-200 dark:border-slate-700 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 hover:border-gray-300 dark:hover:border-slate-600 transition-colors group"
        >
          <div className="w-10 h-10 rounded-lg bg-gray-100 dark:bg-slate-700 flex items-center justify-center shrink-0">
            <FileText className="w-5 h-5 text-gray-700 dark:text-slate-200" aria-hidden="true" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-gray-700 dark:text-slate-200 group-hover:text-gray-900 dark:group-hover:text-white truncate">{displayName}</div>
            <div className="text-xs text-gray-500 dark:text-slate-400">{t('common.clickToOpen')}</div>
          </div>
          <ExternalLink className="w-4 h-4 text-gray-500 dark:text-slate-400 group-hover:text-gray-700 dark:group-hover:text-slate-200 shrink-0" aria-hidden="true" />
        </a>
      )}
    </div>
  );
}

/**
 * An interactive trigger for an uploaded file inside compact list/table rows.
 *
 * Managed `/api/uploads/<name>` blobs are fetched strictly on click (never on
 * mount), avoiding dozens of background binary downloads when viewing a table.
 */
export function AttachmentLink({
  url,
  className,
  children,
}: {
  url: string;
  className?: string;
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);

  if (!url) {
    return <span className={className}>{children}</span>;
  }

  if (!isManagedUploadUrl(url)) {
    if (/^https?:\/\//i.test(url)) {
      return (
        <a
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
          className={className}
        >
          {children}
        </a>
      );
    }
    return <span className={className}>{children}</span>;
  }

  const handleOpen = async (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    if (loading) return;
    setLoading(true);
    // Open synchronously inside the click gesture so Safari/iOS don't block the tab.
    const tab = window.open('', '_blank');
    if (tab) {
      tab.opener = null;
    }
    try {
      const objectUrl = await fetchUploadObjectUrl(url);
      if (tab && !tab.closed) {
        tab.location.href = objectUrl;
      } else {
        const a = document.createElement('a');
        a.href = objectUrl;
        a.target = '_blank';
        a.rel = 'noopener noreferrer';
        document.body.appendChild(a);
        a.click();
        a.remove();
      }
      window.setTimeout(() => URL.revokeObjectURL(objectUrl), 60_000);
    } catch {
      if (tab && !tab.closed) {
        tab.close();
      }
      toast.error(t('common.fileUnavailableAccess'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      type="button"
      onClick={handleOpen}
      disabled={loading}
      aria-busy={loading}
      className={className}
    >
      {children}
    </button>
  );
}
