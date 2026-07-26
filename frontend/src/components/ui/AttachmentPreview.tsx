import { ExternalLink, FileText } from 'lucide-react';

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
    <div className="mt-1 inline-flex items-center gap-2 px-4 py-3 bg-red-50 border border-red-200 rounded-lg">
      <div className="w-10 h-10 rounded-lg bg-red-100 flex items-center justify-center shrink-0">
        <FileText className="w-5 h-5 text-red-400" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-red-700 truncate">{displayName}</div>
        <div className="text-xs text-red-400">{detail}</div>
      </div>
    </div>
  );
}

export function AttachmentPreview({ url, name }: { url: string; name: string | null }) {
  const displayName = name || 'Attachment';
  // Called unconditionally — a `blob:` URL resolves straight through, and the
  // placeholder below is chosen from the raw URL rather than by skipping it.
  const file = useAuthorizedFile(url);

  if (url.startsWith('blob:')) {
    return (
      <UnavailableAttachment
        displayName={displayName}
        detail="File unavailable — was not uploaded properly"
      />
    );
  }

  if (file.status === 'error') {
    return (
      <UnavailableAttachment
        displayName={displayName}
        detail="File unavailable — you may not have access to it"
      />
    );
  }

  if (file.status === 'loading') {
    return (
      <div className="mt-1 inline-flex items-center gap-2 px-4 py-3 bg-gray-50 border border-gray-200 rounded-lg">
        <div className="w-10 h-10 rounded-lg bg-gray-100 flex items-center justify-center shrink-0">
          <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-400" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-sm font-medium text-gray-500 truncate">{displayName}</div>
          <div className="text-xs text-gray-400">Loading…</div>
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
              className="max-w-full max-h-64 rounded-lg border border-gray-200 object-contain bg-gray-50"
            />
          </a>
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 text-sm text-gray-900 hover:text-black"
          >
            <ExternalLink className="w-3.5 h-3.5" />
            Open full size
          </a>
        </div>
      ) : (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 px-4 py-3 bg-gray-50 border border-gray-200 rounded-lg hover:bg-gray-100 hover:border-gray-300 transition-colors group"
        >
          <div className="w-10 h-10 rounded-lg bg-gray-100 flex items-center justify-center shrink-0">
            <FileText className="w-5 h-5 text-gray-700" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-gray-700 group-hover:text-gray-900 truncate">{displayName}</div>
            <div className="text-xs text-gray-400">Click to open</div>
          </div>
          <ExternalLink className="w-4 h-4 text-gray-400 group-hover:text-gray-700 shrink-0" />
        </a>
      )}
    </div>
  );
}

/**
 * An anchor to an uploaded file, for the compact list rows that show a link
 * rather than a preview.
 *
 * Renders as plain text while the file loads or if it is unreadable, so a row
 * never offers a link that would open a broken tab.
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
  const file = useAuthorizedFile(url);

  if (file.status !== 'ready') {
    return (
      <span className={className} aria-busy={file.status === 'loading'}>
        {children}
      </span>
    );
  }

  return (
    <a href={file.href} target="_blank" rel="noopener noreferrer" className={className}>
      {children}
    </a>
  );
}
