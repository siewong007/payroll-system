import { useCallback, useId, useMemo, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from './Modal';
import { PromptContext, normalizePromptOptions, type PromptFn, type PromptOptions } from '@/lib/prompt';

export function PromptProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [active, setActive] = useState<PromptOptions | null>(null);
  const [value, setValue] = useState('');
  const [error, setError] = useState('');
  const resolverRef = useRef<((result: string | null) => void) | null>(null);
  const inputId = useId();

  const settle = useCallback((result: string | null) => {
    const resolve = resolverRef.current;
    resolverRef.current = null;
    setActive(null);
    setValue('');
    setError('');
    resolve?.(result);
  }, []);

  const prompt = useCallback<PromptFn>((input) => {
    const opts = normalizePromptOptions(input);
    if (resolverRef.current) {
      resolverRef.current(null);
    }
    setActive(opts);
    setValue(opts.initialValue ?? '');
    setError('');
    return new Promise<string | null>((resolve) => {
      resolverRef.current = resolve;
    });
  }, []);

  const handleConfirm = () => {
    const trimmed = value.trim();
    if (active?.required && !trimmed) {
      setError(t('ux.prompt.requiredError'));
      return;
    }
    settle(trimmed);
  };

  const ctxValue = useMemo(() => prompt, [prompt]);
  const variant = active?.variant ?? 'primary';

  const confirmButtonClass =
    variant === 'danger'
      ? 'btn-danger'
      : variant === 'warning'
        ? 'inline-flex items-center justify-center gap-2 rounded-md bg-amber-600 px-3.5 py-1.5 min-h-11 sm:min-h-9 text-[13px] font-medium text-white transition-colors hover:bg-amber-700'
        : 'btn-primary';

  return (
    <PromptContext.Provider value={ctxValue}>
      {children}
      <Modal
        open={active !== null}
        onClose={() => settle(null)}
        title={active?.title ?? t('ux.prompt.defaultTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={() => settle(null)} className="btn-secondary">
              {active?.cancelLabel ?? t('common.cancel')}
            </button>
            <button type="button" onClick={handleConfirm} className={confirmButtonClass}>
              {active?.confirmLabel ?? t('common.confirm')}
            </button>
          </div>
        }
      >
        <div className="space-y-2.5">
          <label htmlFor={inputId} className="block text-[13px] leading-relaxed text-[var(--muted-foreground)]">
            {active?.description}
          </label>
          <textarea
            id={inputId}
            rows={3}
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              if (error) setError('');
            }}
            onKeyDown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                e.preventDefault();
                handleConfirm();
              }
            }}
            placeholder={active?.placeholder ?? t('common.notes')}
            aria-invalid={error ? true : undefined}
            className="form-input resize-none"
          />
          {error && (
            <p role="alert" className="text-xs font-medium text-red-600 dark:text-red-400">
              {error}
            </p>
          )}
        </div>
      </Modal>
    </PromptContext.Provider>
  );
}
