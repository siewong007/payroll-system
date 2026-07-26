import { useId } from 'react';

interface FieldAria {
  id: string;
  'aria-describedby'?: string;
  'aria-invalid'?: true;
}

interface FormFieldProps {
  label: string;
  required?: boolean;
  hint?: string;
  error?: string;
  children: (aria: FieldAria) => React.ReactNode;
}

/**
 * Label + control pairing via a render prop, so the `htmlFor`/`id` link and the
 * `aria-describedby` wiring for hint and error text cannot be forgotten at a
 * call site. Controls rendered by hand previously had no accessible name at all.
 */
export function FormField({ label, required, hint, error, children }: FormFieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;

  const describedBy = [hint ? hintId : null, error ? errorId : null].filter(Boolean).join(' ');

  return (
    <div>
      <label htmlFor={id} className="form-label">
        {label}
        {required && <span aria-hidden="true"> *</span>}
        {required && <span className="sr-only"> (required)</span>}
      </label>
      {children({
        id,
        'aria-describedby': describedBy || undefined,
        'aria-invalid': error ? true : undefined,
      })}
      {hint && (
        <p id={hintId} className="mt-1 text-xs text-gray-500">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="mt-1 text-xs text-red-600">
          {error}
        </p>
      )}
    </div>
  );
}

interface FieldsetProps {
  legend: string;
  required?: boolean;
  note?: string;
  children: React.ReactNode;
}

/**
 * Grouping for sets of checkboxes/radios (roles, company assignment). A `<label>`
 * is wrong for a control *set* — `htmlFor` can only point at one input — so the
 * group gets a `<fieldset>`/`<legend>` instead.
 */
export function FieldGroup({ legend, required, note, children }: FieldsetProps) {
  return (
    <fieldset className="border-0 p-0 m-0">
      <legend className="form-label p-0">
        {legend}
        {required && <span aria-hidden="true"> *</span>}
        {required && <span className="sr-only"> (required)</span>}
        {note && <span className="text-gray-400 font-normal"> {note}</span>}
      </legend>
      {children}
    </fieldset>
  );
}
