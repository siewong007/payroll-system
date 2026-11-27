import { useCallback, useMemo, useRef, useState, type ReactNode } from 'react';
import { AlertTriangle, HelpCircle, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Modal } from './Modal';
import {
  ConfirmContext,
  normalizeConfirmOptions,
  type ConfirmFn,
  type ConfirmOptions,
} from '@/lib/confirm';

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [active, setActive] = useState<ConfirmOptions | null>(null);
  const resolverRef = useRef<((confirmed: boolean) => void) | null>(null);

  const settle = useCallback((confirmed: boolean) => {
    const resolve = resolverRef.current;
    resolverRef.current = null;
    setActive(null);
    resolve?.(confirmed);
  }, []);

  const confirm = useCallback<ConfirmFn>((input) => {
    const opts = normalizeConfirmOptions(input);
    if (resolverRef.current) {
      resolverRef.current(false);
    }
    setActive(opts);
    return new Promise<boolean>((resolve) => {
      resolverRef.current = resolve;
    });
  }, []);

  const value = useMemo(() => confirm, [confirm]);

  const variant = active?.variant ?? 'danger';
  const resolvedTitle =
    active?.title ??
    (variant === 'danger' ? t('ux.confirm.destructiveTitle') : t('ux.confirm.defaultTitle'));
  const resolvedConfirm =
    active?.confirmLabel ??
    (variant === 'danger' ? t('common.delete') : t('ux.confirm.proceed'));
  const resolvedCancel = active?.cancelLabel ?? t('common.cancel');

  const Icon = variant === 'danger' ? Trash2 : variant === 'warning' ? AlertTriangle : HelpCircle;
  const badgeStyle =
    variant === 'danger'
      ? 'bg-red-50 text-red-600 border-red-200 dark:bg-red-500/12 dark:text-red-300 dark:border-red-500/25'
      : variant === 'warning'
        ? 'bg-amber-50 text-amber-600 border-amber-200 dark:bg-amber-500/12 dark:text-amber-300 dark:border-amber-500/25'
        : 'bg-[var(--surface-subtle)] text-[var(--accent-1)] border-[var(--border)]';

  const confirmButtonClass =
    variant === 'danger'
      ? 'btn-danger'
      : variant === 'warning'
        ? 'inline-flex items-center justify-center gap-2 rounded-md bg-amber-600 px-3.5 py-1.5 min-h-11 sm:min-h-9 text-[13px] font-medium text-white transition-colors hover:bg-amber-700'
        : 'btn-primary';

  return (
    <ConfirmContext.Provider value={value}>
      {children}
      <Modal
        open={active !== null}
        onClose={() => settle(false)}
        title={resolvedTitle}
        maxWidth="max-w-md"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={() => settle(false)} className="btn-secondary">
              {resolvedCancel}
            </button>
            <button type="button" onClick={() => settle(true)} className={confirmButtonClass}>
              {resolvedConfirm}
            </button>
          </div>
        }
      >
        <div className="flex items-start gap-3">
          <div className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border ${badgeStyle}`}>
            <Icon className="h-4 w-4" aria-hidden="true" />
          </div>
          <p className="pt-0.5 text-[13px] leading-relaxed text-[var(--muted-foreground)]">
            {active?.description}
          </p>
        </div>
      </Modal>
    </ConfirmContext.Provider>
  );
}
