import { useEffect, useRef, useId } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  maxWidth?: string;
}

const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Tracks open Modal instances in mount order so nested modals (e.g. ConfirmDialog
// opened over EmployeeDocumentsModal or DataTable summary) layer cleanly, keep
// body scroll locked until the last modal closes, and route Escape / Tab focus
// trapping exclusively to the topmost dialog.
const openModalStack: string[] = [];

function syncBodyScrollLock() {
  document.body.style.overflow = openModalStack.length > 0 ? 'hidden' : '';
}

export function Modal({ open, onClose, title, children, footer, maxWidth = 'max-w-2xl' }: ModalProps) {
  const { t } = useTranslation();
  const overlayRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const instanceId = useId();
  const titleId = `${instanceId}-title`;

  // Stack registration + ref-counted body scroll lock + focus restoration
  useEffect(() => {
    if (!open) return;

    previousFocusRef.current = document.activeElement as HTMLElement | null;
    if (!openModalStack.includes(instanceId)) {
      openModalStack.push(instanceId);
    }
    const mountedIdx = Math.max(0, openModalStack.indexOf(instanceId));
    const mountedZ = 50 + mountedIdx * 2;
    if (overlayRef.current) overlayRef.current.style.zIndex = String(mountedZ);
    if (containerRef.current) containerRef.current.style.zIndex = String(mountedZ + 1);
    syncBodyScrollLock();

    return () => {
      const idx = openModalStack.indexOf(instanceId);
      const wasTopmost = idx !== -1 && idx === openModalStack.length - 1;
      if (idx !== -1) {
        openModalStack.splice(idx, 1);
      }
      syncBodyScrollLock();
      if (wasTopmost) {
        previousFocusRef.current?.focus?.();
      }
    };
  }, [instanceId, open]);

  // Topmost-only Escape dismissal & Tab focus trapping
  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (openModalStack[openModalStack.length - 1] !== instanceId) {
        return;
      }

      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
        return;
      }

      if (e.key !== 'Tab') return;
      const dialog = dialogRef.current;
      if (!dialog) return;

      const focusableEls = dialog.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (focusableEls.length === 0) {
        e.preventDefault();
        return;
      }

      const first = focusableEls[0];
      const last = focusableEls[focusableEls.length - 1];

      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [instanceId, open, onClose]);

  useEffect(() => {
    if (!open) return;
    const raf = requestAnimationFrame(() => {
      if (
        openModalStack[openModalStack.length - 1] === instanceId &&
        dialogRef.current &&
        !dialogRef.current.contains(document.activeElement)
      ) {
        const firstFocusable = dialogRef.current.querySelector<HTMLElement>(FOCUSABLE);
        (firstFocusable ?? dialogRef.current).focus();
      }
    });
    return () => cancelAnimationFrame(raf);
  }, [instanceId, open]);

  const existingIndex = openModalStack.indexOf(instanceId);
  const stackIndex = open ? (existingIndex !== -1 ? existingIndex : openModalStack.length) : 0;
  const zIndex = 50 + stackIndex * 2;

  const content = open ? (
    <>
      {/* Overlay — covers viewport via w-screen h-screen */}
      <div
        ref={overlayRef}
        style={{ zIndex }}
        className="animate-fade-in fixed top-0 left-0 w-screen h-screen bg-black/45"
        onClick={onClose}
        role="presentation"
        aria-hidden="true"
      />

      {/* Centering container — grid places modal in center */}
      <div
        ref={containerRef}
        style={{ zIndex: zIndex + 1 }}
        className="fixed top-0 left-0 w-screen h-[100dvh] grid place-items-end sm:place-items-center p-0 sm:p-4 pointer-events-none"
      >
        {/* Modal panel */}
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby={title ? titleId : undefined}
          tabIndex={-1}
          className={`
            animate-scale-in relative bg-[var(--card)] border border-[var(--border)] w-full ${maxWidth} outline-none flex flex-col
            shadow-lg pointer-events-auto
            rounded-t-xl sm:rounded-xl
            max-h-[95dvh] sm:max-h-[85vh]
          `}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          {title && (
            <div className="flex items-center justify-between px-5 py-3.5 border-b border-[var(--border)] shrink-0">
              <div className="absolute top-2 left-1/2 -translate-x-1/2 w-10 h-1 rounded-full bg-[var(--border-strong)] sm:hidden" />
              <h2 id={titleId} className="text-base font-semibold tracking-[-0.01em] text-[var(--foreground)] pr-8">
                {title}
              </h2>
              <button
                type="button"
                onClick={onClose}
                aria-label={t('a11y.closeDialog')}
                className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center -mr-1.5 rounded-md text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          )}

          {/* Scrollable content */}
          <div
            className={`overflow-y-auto flex-1 px-5 overscroll-contain ${
              footer ? 'py-4' : 'pt-4 pb-[max(1.25rem,env(safe-area-inset-bottom))] sm:pb-4'
            }`}
          >
            {children}
          </div>

          {/* Sticky footer */}
          {footer && (
            <div className="px-5 py-3 pb-[max(0.875rem,env(safe-area-inset-bottom))] sm:pb-3 border-t border-[var(--border)] shrink-0 bg-[var(--surface-subtle)]/60 sm:rounded-b-xl">
              {footer}
            </div>
          )}
        </div>
      </div>
    </>
  ) : null;

  if (typeof document === 'undefined') {
    return content;
  }
  return createPortal(content, document.body);
}
