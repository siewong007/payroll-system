import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { AlertCircle, AlertTriangle, CheckCircle2, Info, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  ToastContext,
  allocateToastId,
  subscribeImperativeToast,
  type PushToastInput,
  type ToastContextValue,
  type ToastItem,
  type ToastTone,
} from '@/lib/toast';

const TONE_META: Record<ToastTone, { icon: typeof CheckCircle2; iconClass: string; ringClass: string }> = {
  success: {
    icon: CheckCircle2,
    iconClass: 'text-emerald-600 dark:text-emerald-400',
    ringClass: 'border-emerald-200 dark:border-emerald-500/30',
  },
  error: {
    icon: AlertCircle,
    iconClass: 'text-red-600 dark:text-red-400',
    ringClass: 'border-red-200 dark:border-red-500/30',
  },
  warning: {
    icon: AlertTriangle,
    iconClass: 'text-amber-600 dark:text-amber-400',
    ringClass: 'border-amber-200 dark:border-amber-500/30',
  },
  info: {
    icon: Info,
    iconClass: 'text-[var(--accent-1)]',
    ringClass: 'border-[var(--border)]',
  },
};

function ToastCard({
  item,
  onDismiss,
}: {
  item: ToastItem;
  onDismiss: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [paused, setPaused] = useState(false);
  const meta = TONE_META[item.tone];
  const Icon = meta.icon;

  useEffect(() => {
    const ms = item.durationMs ?? 4200;
    if (ms <= 0 || paused) return;
    const timer = window.setTimeout(() => onDismiss(item.id), ms);
    return () => window.clearTimeout(timer);
  }, [item.durationMs, item.id, onDismiss, paused]);

  return (
    <div
      role={item.tone === 'error' ? 'alert' : 'status'}
      aria-live={item.tone === 'error' ? 'assertive' : 'polite'}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      className={`animate-drop-in pointer-events-auto flex w-full max-w-sm items-start gap-2.5 rounded-lg border bg-[var(--card)] px-3.5 py-2.5 shadow-md ${meta.ringClass}`}
    >
      <Icon className={`mt-0.5 h-4 w-4 shrink-0 ${meta.iconClass}`} aria-hidden="true" />
      <div className="min-w-0 flex-1 text-[13px]">
        {item.title && <p className="font-semibold text-[var(--foreground)]">{item.title}</p>}
        <p className={`break-words text-[var(--foreground)] ${item.title ? 'mt-0.5 text-xs text-[var(--muted-foreground)]' : 'font-medium'}`}>
          {item.message}
        </p>
      </div>
      <button
        type="button"
        onClick={() => onDismiss(item.id)}
        aria-label={t('ux.toast.dismiss')}
        className="rounded-md p-1 text-[var(--muted-foreground)] transition-colors hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)]"
      >
        <X className="h-4 w-4" aria-hidden="true" />
      </button>
    </div>
  );
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [items, setItems] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: string) => {
    setItems((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const push = useCallback((input: PushToastInput) => {
    const id = allocateToastId();
    const next: ToastItem = {
      id,
      tone: input.tone ?? 'info',
      title: input.title,
      message: input.message,
      durationMs: input.durationMs ?? (input.tone === 'error' ? 6000 : 4200),
    };
    setItems((prev) => [...prev.slice(-4), next]);
    return id;
  }, []);

  useEffect(() => {
    return subscribeImperativeToast((item) => {
      setItems((prev) => [...prev.slice(-4), item]);
    });
  }, []);

  const value = useMemo<ToastContextValue>(
    () => ({
      push,
      dismiss,
      success: (message, title) => push({ tone: 'success', message, title }),
      error: (message, title) => push({ tone: 'error', message, title }),
      warning: (message, title) => push({ tone: 'warning', message, title }),
      info: (message, title) => push({ tone: 'info', message, title }),
    }),
    [dismiss, push],
  );

  return (
    <ToastContext.Provider value={value}>
      {children}
      <section
        aria-label={t('ux.toast.regionLabel')}
        className="pointer-events-none fixed inset-x-4 bottom-[calc(4.75rem+env(safe-area-inset-bottom))] z-[80] flex flex-col items-center gap-2 sm:inset-x-auto sm:right-4 sm:top-4 sm:bottom-auto sm:w-full sm:max-w-sm sm:items-end"
      >
        {items.map((item) => (
          <ToastCard key={item.id} item={item} onDismiss={dismiss} />
        ))}
      </section>
    </ToastContext.Provider>
  );
}
