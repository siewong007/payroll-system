import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { keepPreviousData, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  AtSign,
  CheckCircle,
  Clock,
  Eye,
  FileText,
  Mail,
  Pencil,
  Plus,
  Send,
  Trash2,
  Users,
  XCircle,
} from 'lucide-react';
import {
  getEmailTemplates,
  createEmailTemplate,
  updateEmailTemplate,
  deleteEmailTemplate,
  sendLetter,
  previewLetter,
  getEmailLogs,
} from '@/api/email';
import { getEmployee } from '@/api/employees';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { useAuth } from '@/context/AuthContext';
import { useConfirm } from '@/lib/confirm';
import { SUPER_ADMIN_ROLES, hasAnyRole } from '@/lib/roles';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { useUnsavedChanges } from '@/lib/useUnsavedChanges';
import { formatDate, getErrorMessage } from '@/lib/utils';
import type { EmailLog, EmailTemplate, LetterType, PreviewLetterResponse } from '@/types';

const LETTER_TYPE_VALUES: LetterType[] = [
  'general', 'offer', 'appointment', 'warning', 'termination', 'promotion',
];

// i18n-ok: merge-field identifiers are API placeholders, not prose copy
const MERGE_VARIABLES = [
  '{{employee_name}}',
  '{{employee_number}}',
  '{{company_name}}',
  '{{designation}}',
  '{{department}}',
  '{{date_joined}}',
];

const HISTORY_PER_PAGE = 20;

// i18n-ok: DEFAULT_TEMPLATES is seeded email *content* the admin edits before
// sending — document text, not UI copy.
const DEFAULT_TEMPLATES: Record<LetterType, { subject: string; body: string }> = {
  general: {
    subject: '',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p></p><p>Best regards,<br>{{company_name}}</p>',  // i18n-ok: email template content
  },
  welcome: {
    subject: 'Welcome to {{company_name}}',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Welcome to <strong>{{company_name}}</strong>! We are delighted to have you join our team.</p><p>Your employee number is <strong>{{employee_number}}</strong> and your start date is <strong>{{date_joined}}</strong>.</p><p>Best regards,<br>{{company_name}} HR Team</p>',  // i18n-ok: email template content
  },
  offer: {
    subject: 'Offer of Employment - {{company_name}}',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>We are pleased to extend this offer of employment for the position of <strong>{{designation}}</strong> in the <strong>{{department}}</strong> department at <strong>{{company_name}}</strong>.</p><p><strong>Start Date:</strong> {{date_joined}}</p><p>Please review the terms and conditions of this offer. We look forward to welcoming you to the team.</p><p>Sincerely,<br>{{company_name}} HR Team</p>',  // i18n-ok: email template content
  },
  appointment: {
    subject: 'Appointment Letter - {{company_name}}',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Following your acceptance of our offer, we are pleased to formally appoint you as <strong>{{designation}}</strong> in the <strong>{{department}}</strong> department at <strong>{{company_name}}</strong>.</p><p><strong>Employee Number:</strong> {{employee_number}}<br><strong>Date of Joining:</strong> {{date_joined}}</p><p>This letter serves as formal confirmation of your employment. The detailed terms and conditions are as discussed.</p><p>We look forward to your contributions to the team.</p><p>Best regards,<br>{{company_name}} HR Team</p>',  // i18n-ok: email template content
  },
  warning: {
    subject: 'Warning Letter - {{company_name}}',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Employee Number: <strong>{{employee_number}}</strong><br>Department: <strong>{{department}}</strong></p><p>This letter serves as a formal warning regarding [describe the issue]. This behaviour/performance is in violation of company policy.</p><p>We expect immediate improvement. Failure to comply may result in further disciplinary action.</p><p>Please acknowledge receipt of this letter.</p><p>Regards,<br>{{company_name}} HR Department</p>',  // i18n-ok: email template content
  },
  termination: {
    subject: 'Termination of Employment - {{company_name}}',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Employee Number: <strong>{{employee_number}}</strong><br>Department: <strong>{{department}}</strong></p><p>We regret to inform you that your employment with <strong>{{company_name}}</strong> is terminated effective [last working day].</p><p><strong>Reason:</strong> [Describe reason]</p><p>Please arrange to return all company property. Settlement of your final pay and benefits will be processed accordingly.</p><p>Regards,<br>{{company_name}} HR Department</p>',  // i18n-ok: email template content
  },
  promotion: {
    subject: 'Congratulations on Your Promotion - {{company_name}}',  // i18n-ok: email template content
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>We are delighted to inform you that you have been promoted to the position of <strong>[New Position]</strong> effective [effective date].</p><p>This promotion is in recognition of your hard work and contributions to <strong>{{company_name}}</strong>.</p><p>Your new role will include [briefly describe new responsibilities].</p><p>Congratulations and best wishes in your new role!</p><p>Best regards,<br>{{company_name}} HR Team</p>',  // i18n-ok: email template content
  },
};

function StatusBadge({ status }: { status: string }) {
  const { t } = useTranslation();
  if (status === 'sent') {
    return (
      <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-50 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300 whitespace-nowrap">
        <CheckCircle className="w-3 h-3" /> {t('letters.sent')}
      </span>
    );
  }
  if (status === 'failed') {
    return (
      <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300 whitespace-nowrap">
        <XCircle className="w-3 h-3" /> {t('letters.failed')}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-medium bg-yellow-50 text-yellow-700 dark:bg-amber-500/15 dark:text-amber-300 whitespace-nowrap">
      <Clock className="w-3 h-3" /> {t('enums.requestStatus.pending')}
    </span>
  );
}

type RecipientMode = 'employee' | 'custom';
type LetterTab = 'compose' | 'history' | 'templates';

export function LettersPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const { can } = usePermissions();
  const templateFormId = useId();
  const bodyTextareaRef = useRef<HTMLTextAreaElement>(null);

  const canSendLetters = can('send_letters');
  const canManageTemplates = can('manage_email_templates');
  const canUseCustomEmail = can('manage_employees') || hasAnyRole(user, SUPER_ADMIN_ROLES);
  const canPreview = canSendLetters || canManageTemplates;

  const [tab, setTab] = useState<LetterTab>(() => (canPreview ? 'compose' : 'history'));
  const activeTab: LetterTab = !canPreview && tab === 'compose' ? 'history' : tab;

  const [selectedType, setSelectedType] = useState<LetterType>('general');
  const [recipientMode, setRecipientMode] = useState<RecipientMode>('employee');
  const effectiveRecipientMode: RecipientMode = canUseCustomEmail ? recipientMode : 'employee';
  const [selectedEmployee, setSelectedEmployee] = useState('');
  const [customEmail, setCustomEmail] = useState('');
  const [customName, setCustomName] = useState('');
  const [subject, setSubject] = useState(DEFAULT_TEMPLATES.general.subject);
  const [bodyHtml, setBodyHtml] = useState(DEFAULT_TEMPLATES.general.body);
  const [seedBaseline, setSeedBaseline] = useState({
    subject: DEFAULT_TEMPLATES.general.subject,
    body: DEFAULT_TEMPLATES.general.body,
  });
  const [loadedTemplateId, setLoadedTemplateId] = useState<string | undefined>(undefined);
  const [preview, setPreview] = useState<PreviewLetterResponse | null>(null);
  const [showPreview, setShowPreview] = useState(false);

  // Template modal (create from compose or edit existing template)
  const [templateModalMode, setTemplateModalMode] = useState<'create' | 'edit' | null>(null);
  const [editingTemplate, setEditingTemplate] = useState<EmailTemplate | null>(null);
  const [templateName, setTemplateName] = useState('');
  const [templateSubject, setTemplateSubject] = useState('');
  const [templateBodyHtml, setTemplateBodyHtml] = useState('');
  const [templateFormError, setTemplateFormError] = useState('');
  const [deletingTemplateId, setDeletingTemplateId] = useState<string | null>(null);
  const [historyPage, setHistoryPage] = useState(1);

  const isComposeDirty =
    canPreview &&
    (subject !== seedBaseline.subject || bodyHtml !== seedBaseline.body) &&
    (subject.trim().length > 0 || bodyHtml.trim().length > 0);
  useUnsavedChanges(isComposeDirty);

  // One employee, fetched by id — the picker resolves the choice, and this is
  // only here for the "no email on file" warning below.
  const {
    data: selectedEmp,
    isError: selectedEmpError,
    error: selectedEmpErrorObj,
    refetch: refetchSelectedEmp,
  } = useQuery({
    queryKey: ['employee', selectedEmployee],
    queryFn: () => getEmployee(selectedEmployee),
    enabled: Boolean(selectedEmployee) && canPreview,
  });

  const {
    data: templates,
    isLoading: templatesLoading,
    isError: templatesError,
    error: templatesErrorObj,
    refetch: refetchTemplates,
  } = useQuery({
    queryKey: ['emailTemplates'],
    queryFn: () => getEmailTemplates(),
  });

  const {
    data: logs,
    isLoading: logsLoading,
    isError: logsError,
    error: logsErrorObj,
    refetch: refetchLogs,
  } = useQuery({
    queryKey: ['emailLogs', historyPage],
    queryFn: () => getEmailLogs({ page: historyPage, per_page: HISTORY_PER_PAGE }),
    placeholderData: keepPreviousData,
  });

  const sendMutation = useMutation({
    mutationFn: sendLetter,
    // A refused send still writes an email_logs row, and that row is the
    // evidence of the attempt — so History refreshes either way, not only on
    // success.
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['emailLogs'] });
    },
    onSuccess: (data) => {
      if (data.status !== 'sent' && data.status !== 'pending') return;
      if (data.status === 'pending') {
        toast.info(t('letters.queuedForRetry'));
      } else {
        toast.success(t('letters.sendSuccess'));
      }
      setShowPreview(false);
      setPreview(null);
      setSelectedEmployee('');
      setCustomEmail('');
      setCustomName('');
      setSeedBaseline({ subject, body: bodyHtml });
    },
  });

  const previewMutation = useMutation({
    mutationFn: previewLetter,
    onSuccess: (data) => {
      // Reopening the preview must not still show the previous attempt's error.
      sendMutation.reset();
      setPreview(data);
      setShowPreview(true);
    },
  });

  const saveTemplateMutation = useMutation({
    mutationFn: createEmailTemplate,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['emailTemplates'] });
      setTemplateModalMode(null);
      setTemplateName('');
      toast.success(t('letters.templateSaved'));
    },
  });

  const updateTemplateMutation = useMutation({
    mutationFn: ({ id, name, subject: nextSubject, body_html }: { id: string; name: string; subject: string; body_html: string }) =>
      updateEmailTemplate(id, { name, subject: nextSubject, body_html }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['emailTemplates'] });
      setTemplateModalMode(null);
      setEditingTemplate(null);
      toast.success(t('letters.templateSaved'));
    },
  });

  const deleteTemplateMutation = useMutation({
    mutationFn: deleteEmailTemplate,
    onMutate: (id) => setDeletingTemplateId(id),
    onSuccess: (_data, deletedId) => {
      if (loadedTemplateId === deletedId) {
        setLoadedTemplateId(undefined);
      }
      queryClient.invalidateQueries({ queryKey: ['emailTemplates'] });
      toast.success(t('letters.templateDeleted'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('letters.templateDeleteFailed'))),
    onSettled: () => setDeletingTemplateId(null),
  });

  const confirmOverwriteIfDirty = async () => {
    if (!isComposeDirty) return true;
    return confirmDialog({
      title: t('letters.replaceDraftTitle'),
      description: t('letters.replaceDraftConfirm'),
      confirmLabel: t('ux.confirm.proceed'),
      variant: 'warning',
    });
  };

  const handleTypeChange = async (type: LetterType) => {
    if (type === selectedType && !loadedTemplateId) return;
    const ok = await confirmOverwriteIfDirty();
    if (!ok) return;
    const nextSubject = DEFAULT_TEMPLATES[type].subject;
    const nextBody = DEFAULT_TEMPLATES[type].body;
    setSelectedType(type);
    setSubject(nextSubject);
    setBodyHtml(nextBody);
    setSeedBaseline({ subject: nextSubject, body: nextBody });
    setLoadedTemplateId(undefined);
  };

  const handleLoadTemplate = async (template: EmailTemplate) => {
    const ok = await confirmOverwriteIfDirty();
    if (!ok) return;
    setSelectedType(template.letter_type);
    setSubject(template.subject);
    setBodyHtml(template.body_html);
    setSeedBaseline({ subject: template.subject, body: template.body_html });
    setLoadedTemplateId(template.id);
    setTab('compose');
  };

  const insertMergeVariable = (variable: string) => {
    const el = bodyTextareaRef.current;
    if (!el) {
      setBodyHtml((prev) => `${prev}${variable}`);
      return;
    }
    const start = el.selectionStart ?? bodyHtml.length;
    const end = el.selectionEnd ?? bodyHtml.length;
    const next = bodyHtml.slice(0, start) + variable + bodyHtml.slice(end);
    setBodyHtml(next);
    window.requestAnimationFrame(() => {
      el.focus();
      const pos = start + variable.length;
      el.setSelectionRange(pos, pos);
    });
  };

  const openSaveTemplateModal = () => {
    saveTemplateMutation.reset();
    updateTemplateMutation.reset();
    setTemplateFormError('');
    setEditingTemplate(null);
    setTemplateName('');
    setTemplateSubject(subject);
    setTemplateBodyHtml(bodyHtml);
    setTemplateModalMode('create');
  };

  const openEditTemplateModal = (tpl: EmailTemplate) => {
    saveTemplateMutation.reset();
    updateTemplateMutation.reset();
    setTemplateFormError('');
    setEditingTemplate(tpl);
    setTemplateName(tpl.name);
    setTemplateSubject(tpl.subject);
    setTemplateBodyHtml(tpl.body_html);
    setTemplateModalMode('edit');
  };

  const trimmedCustomEmail = customEmail.trim();
  const hasValidRecipient =
    effectiveRecipientMode === 'employee'
      ? Boolean(selectedEmployee && selectedEmp?.email)
      : canUseCustomEmail && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmedCustomEmail);

  const canSend = hasValidRecipient && subject.trim().length > 0 && bodyHtml.trim().length > 0;

  const handlePreview = () => {
    if (!canSend || !canPreview) return;
    if (effectiveRecipientMode === 'employee') {
      previewMutation.mutate({
        employee_id: selectedEmployee,
        subject: subject.trim(),
        body_html: bodyHtml,
      });
    } else {
      previewMutation.mutate({
        recipient_email: trimmedCustomEmail,
        recipient_name: customName.trim() || undefined,
        subject: subject.trim(),
        body_html: bodyHtml,
      });
    }
  };

  const handleSend = () => {
    if (!canSend || !canSendLetters) return;
    if (effectiveRecipientMode === 'employee') {
      sendMutation.mutate({
        employee_id: selectedEmployee,
        letter_type: selectedType,
        subject: subject.trim(),
        body_html: bodyHtml,
        template_id: loadedTemplateId,
      });
    } else {
      sendMutation.mutate({
        recipient_email: trimmedCustomEmail,
        recipient_name: customName.trim() || undefined,
        letter_type: selectedType,
        subject: subject.trim(),
        body_html: bodyHtml,
        template_id: loadedTemplateId,
      });
    }
  };

  const availableTabs: LetterTab[] = canPreview
    ? ['compose', 'history', 'templates']
    : ['history', 'templates'];

  const historyColumns: Column<EmailLog>[] = [
    {
      key: 'date',
      header: t('common.date'),
      render: (log) => <span className="text-gray-500 dark:text-slate-400 whitespace-nowrap">{formatDate(log.created_at)}</span>,
    },
    {
      key: 'type',
      header: t('letters.type'),
      render: (log) => (
        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-300 whitespace-nowrap">
          {t(`letters.types.${log.letter_type}`, { defaultValue: log.letter_type })}
        </span>
      ),
    },
    {
      key: 'recipient',
      primary: true,
      header: t('letters.recipient'),
      render: (log) => (
        <div className="min-w-0">
          <p className="font-medium text-gray-900 dark:text-slate-100 truncate">{log.recipient_name || '-'}</p>
          <p className="text-xs text-gray-500 dark:text-slate-400 truncate">{log.recipient_email}</p>
        </div>
      ),
    },
    {
      key: 'subject',
      primary: true,
      header: t('letters.subject'),
      render: (log) => (
        <span className="text-gray-700 dark:text-slate-200 max-w-xs truncate block">{log.subject}</span>
      ),
    },
    {
      key: 'status',
      header: t('common.status'),
      render: (log) => (
        <div>
          <StatusBadge status={log.status} />
          {log.error_message && (
            <p className="text-xs text-red-500 dark:text-red-400 mt-1 break-words max-w-xs">{log.error_message}</p>
          )}
        </div>
      ),
    },
  ];

  const isSavingTemplate = saveTemplateMutation.isPending || updateTemplateMutation.isPending;

  return (
    <div>
      <PageHeader title={t('letters.title')} subtitle={t('letters.subtitle')} />

      {/* Tabs */}
      <div className="flex gap-1 bg-gray-100 dark:bg-slate-800/70 p-1 rounded-xl mb-6 w-fit max-w-full overflow-x-auto">
        {availableTabs.map((tabKey) => (
          <button
            type="button"
            key={tabKey}
            aria-pressed={activeTab === tabKey}
            onClick={() => setTab(tabKey)}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors capitalize whitespace-nowrap shrink-0 ${
              activeTab === tabKey
                ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-slate-200'
            }`}
          >
            {tabKey === 'compose' && <span className="flex items-center gap-1.5"><Mail className="w-4 h-4" /> {t('letters.tabs.compose')}</span>}
            {tabKey === 'history' && <span className="flex items-center gap-1.5"><Clock className="w-4 h-4" /> {t('letters.tabs.history')}</span>}
            {tabKey === 'templates' && <span className="flex items-center gap-1.5"><FileText className="w-4 h-4" /> {t('letters.tabs.templates')}</span>}
          </button>
        ))}
      </div>

      {/* ── Compose Tab ── */}
      {activeTab === 'compose' && canPreview && (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left: Compose form */}
          <div className="lg:col-span-2 space-y-4">
            {/* Letter Type */}
            <div className="bg-white dark:bg-slate-900 rounded-2xl shadow-sm border border-gray-200 dark:border-slate-800 p-5 sm:p-6">
              <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100 mb-3">{t('letters.type')}</h2>
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
                {LETTER_TYPE_VALUES.map((ltv) => (
                  <button
                    type="button"
                    key={ltv}
                    aria-pressed={selectedType === ltv}
                    onClick={() => { void handleTypeChange(ltv); }}
                    className={`text-left p-3 rounded-xl border-2 transition-colors ${
                      selectedType === ltv
                        ? 'border-black dark:border-slate-200 bg-gray-50 dark:bg-slate-800'
                        : 'border-gray-100 dark:border-slate-800 hover:border-gray-200 dark:hover:border-slate-700'
                    }`}
                  >
                    <p className="text-sm font-medium text-gray-900 dark:text-slate-100">{t(`letters.types.${ltv}`)}</p>
                    <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{t(`letters.types.${ltv}Description`)}</p>
                  </button>
                ))}
              </div>
            </div>

            {/* Recipient */}
            <div className="bg-white dark:bg-slate-900 rounded-2xl shadow-sm border border-gray-200 dark:border-slate-800 p-5 sm:p-6">
              <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
                <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('letters.recipient')}</h2>
                {canUseCustomEmail && (
                  <div className="flex gap-1 bg-gray-100 dark:bg-slate-800 p-0.5 rounded-lg">
                    <button
                      type="button"
                      onClick={() => setRecipientMode('employee')}
                      className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                        effectiveRecipientMode === 'employee'
                          ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                          : 'text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
                      }`}
                    >
                      <Users className="w-3.5 h-3.5" />
                      {t('common.employee')}
                    </button>
                    <button
                      type="button"
                      onClick={() => setRecipientMode('custom')}
                      className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                        effectiveRecipientMode === 'custom'
                          ? 'bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 shadow-sm'
                          : 'text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200'
                      }`}
                    >
                      <AtSign className="w-3.5 h-3.5" />
                      {t('letters.customEmail')}
                    </button>
                  </div>
                )}
              </div>

              {effectiveRecipientMode === 'employee' ? (
                <>
                  {/* `isActive={null}` — a termination or an EA-form letter is
                      routinely addressed to someone who has already left. */}
                  <EmployeePicker
                    value={selectedEmployee}
                    onChange={(id) => setSelectedEmployee(id)}
                    isActive={null}
                    placeholder={t('letters.searchEmployee')}
                  />
                  {selectedEmployee && selectedEmpError && (
                    <div role="alert" className="mt-2 flex items-center justify-between gap-2 rounded-lg border border-red-200 dark:border-red-500/30 bg-red-50 dark:bg-red-500/10 px-3 py-2 text-xs text-red-700 dark:text-red-300">
                      <span>{getErrorMessage(selectedEmpErrorObj, t('letters.employeeLoadFailed'))}</span>
                      <button type="button" onClick={() => void refetchSelectedEmp()} className="font-medium underline shrink-0">
                        {t('common.retry')}
                      </button>
                    </div>
                  )}
                  {selectedEmp && !selectedEmp.email && (
                    <p className="text-xs text-red-500 dark:text-red-400 mt-2">{t('letters.noEmailOnFile')}</p>
                  )}
                  {selectedEmp?.email && (
                    <p className="text-xs text-gray-500 dark:text-slate-400 mt-2">{t('common.email')}: {selectedEmp.email}</p>
                  )}
                </>
              ) : (
                <div className="space-y-3">
                  <FormField label={t('common.email')} required>
                    {(aria) => (
                      <input
                        {...aria}
                        type="email"
                        value={customEmail}
                        onChange={(e) => setCustomEmail(e.target.value)}
                        // i18n-ok: email-format example
                        placeholder="recipient@example.com"
                        className="form-input"
                      />
                    )}
                  </FormField>
                  <FormField label={t('letters.recipientNameOptional')}>
                    {(aria) => (
                      <input
                        {...aria}
                        type="text"
                        value={customName}
                        onChange={(e) => setCustomName(e.target.value)}
                        // i18n-ok: example name
                        placeholder="John Doe"
                        className="form-input"
                      />
                    )}
                  </FormField>
                </div>
              )}
            </div>

            {/* Subject & Body */}
            <div className="bg-white dark:bg-slate-900 rounded-2xl shadow-sm border border-gray-200 dark:border-slate-800 p-5 sm:p-6 space-y-4">
              <FormField label={t('letters.subject')} required>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={subject}
                    onChange={(e) => setSubject(e.target.value)}
                    className="form-input"
                    placeholder={t('letters.subjectPlaceholder')}
                  />
                )}
              </FormField>

              <div>
                <p className="text-xs font-medium text-gray-500 dark:text-slate-400 mb-1.5">
                  {t('letters.insertVariable')}
                </p>
                <div className="flex flex-wrap gap-1.5">
                  {MERGE_VARIABLES.map((token) => (
                    <button
                      type="button"
                      key={token}
                      onClick={() => insertMergeVariable(token)}
                      className="text-xs font-mono px-2.5 py-1 rounded-md bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-200 hover:bg-gray-200 dark:hover:bg-slate-700 border border-gray-200 dark:border-slate-700 transition-colors"
                    >
                      {token}
                    </button>
                  ))}
                </div>
              </div>

              <FormField label={t('letters.bodyHtml')} required>
                {(aria) => (
                  <textarea
                    {...aria}
                    ref={bodyTextareaRef}
                    value={bodyHtml}
                    onChange={(e) => setBodyHtml(e.target.value)}
                    rows={12}
                    className="form-input font-mono text-sm"
                  />
                )}
              </FormField>

              {previewMutation.isError && (
                <div role="alert" className="bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-lg border border-red-200 dark:border-red-500/30">
                  {getErrorMessage(previewMutation.error, t('letters.previewFailed'))}
                </div>
              )}

              {/* Action buttons */}
              <div className="flex flex-wrap gap-3">
                <button
                  type="button"
                  onClick={handlePreview}
                  disabled={!canSend || previewMutation.isPending}
                  className="btn-secondary"
                >
                  <Eye className="w-4 h-4" />
                  {previewMutation.isPending ? t('common.loading') : t('letters.preview')}
                </button>
                {canManageTemplates && (
                  <button
                    type="button"
                    onClick={openSaveTemplateModal}
                    className="btn-secondary"
                  >
                    <Plus className="w-4 h-4" />
                    {t('letters.saveAsTemplate')}
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* Right: Quick templates sidebar */}
          <div className="space-y-4">
            <div className="bg-white dark:bg-slate-900 rounded-2xl shadow-sm border border-gray-200 dark:border-slate-800 p-5 sm:p-6">
              <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100 mb-3">{t('letters.savedTemplates')}</h2>
              {templatesLoading ? (
                <p className="text-sm text-gray-400 dark:text-slate-500">{t('common.loading')}</p>
              ) : templatesError ? (
                <EmptyState
                  compact
                  icon={AlertTriangle}
                  title={t('letters.templatesLoadFailed')}
                  description={getErrorMessage(templatesErrorObj)}
                  action={
                    <button type="button" onClick={() => refetchTemplates()} className="btn-secondary !text-xs">
                      {t('common.retry')}
                    </button>
                  }
                />
              ) : (!templates || templates.length === 0) ? (
                <p className="text-sm text-gray-400 dark:text-slate-500">{t('letters.noSavedTemplates')}</p>
              ) : (
                <div className="space-y-2">
                  {templates.map((tpl) => (
                    <button
                      type="button"
                      key={tpl.id}
                      onClick={() => { void handleLoadTemplate(tpl); }}
                      className="w-full text-left p-3 rounded-xl border border-gray-100 dark:border-slate-800 hover:border-gray-200 dark:hover:border-slate-700 hover:bg-gray-50 dark:hover:bg-slate-800/70 transition-colors"
                    >
                      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{tpl.name}</p>
                      <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{t(`letters.types.${tpl.letter_type}`, { defaultValue: tpl.letter_type })}</p>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ── History Tab ── */}
      {activeTab === 'history' && (
        logsError ? (
          <EmptyState
            icon={AlertTriangle}
            title={t('letters.logsLoadFailed')}
            description={getErrorMessage(logsErrorObj, t('letters.logsLoadFailed'))}
            action={
              <button type="button" onClick={() => refetchLogs()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        ) : (
          <DataTable
            columns={historyColumns}
            data={logs?.data ?? []}
            total={logs?.total}
            page={historyPage}
            onPageChange={setHistoryPage}
            perPage={HISTORY_PER_PAGE}
            isLoading={logsLoading}
            emptyIcon={<Mail className="w-8 h-8 text-gray-300 dark:text-slate-600" />}
            emptyMessage={t('letters.noEmails')}
          />
        )
      )}

      {/* ── Templates Tab ── */}
      {activeTab === 'templates' && (
        templatesLoading ? (
          <div className="flex items-center justify-center py-12">
            <div className="spinner" />
          </div>
        ) : templatesError ? (
          <EmptyState
            icon={AlertTriangle}
            title={t('letters.templatesLoadFailed')}
            description={getErrorMessage(templatesErrorObj, t('letters.templatesLoadFailed'))}
            action={
              <button type="button" onClick={() => refetchTemplates()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {(!templates || templates.length === 0) ? (
              <div className="col-span-full text-center py-12 text-gray-400 dark:text-slate-500">
                <FileText className="w-8 h-8 mx-auto mb-2 opacity-30" />
                <p>{t('letters.noTemplates')}</p>
              </div>
            ) : (
              templates.map((tpl) => (
                <div key={tpl.id} className="bg-white dark:bg-slate-900 rounded-2xl shadow-sm border border-gray-200 dark:border-slate-800 p-6 flex flex-col justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex items-start justify-between gap-2 mb-2">
                      <div className="min-w-0">
                        <h3 className="font-semibold text-gray-900 dark:text-slate-100 truncate">{tpl.name}</h3>
                        <span className="inline-block mt-1 text-xs px-2 py-0.5 rounded-full bg-gray-100 dark:bg-slate-800 text-gray-600 dark:text-slate-300">
                          {t(`letters.types.${tpl.letter_type}`, { defaultValue: tpl.letter_type })}
                        </span>
                      </div>
                      {canManageTemplates && (
                        <div className="flex items-center gap-1 shrink-0">
                          <button
                            type="button"
                            onClick={() => openEditTemplateModal(tpl)}
                            aria-label={t('common.editName', { name: tpl.name })}
                            title={t('common.edit')}
                            className="min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors"
                          >
                            <Pencil className="w-4 h-4" />
                          </button>
                          <button
                            type="button"
                            disabled={deletingTemplateId === tpl.id && deleteTemplateMutation.isPending}
                            onClick={async () => {
                              const ok = await confirmDialog({
                                description: t('letters.deleteTemplateConfirm', { name: tpl.name }),
                                variant: 'danger',
                              });
                              if (ok) deleteTemplateMutation.mutate(tpl.id);
                            }}
                            aria-label={t('common.deleteName', { name: tpl.name })}
                            title={t('common.delete')}
                            className="min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-gray-400 dark:text-slate-500 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 disabled:opacity-50 transition-colors"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      )}
                    </div>
                    <p className="text-sm text-gray-600 dark:text-slate-300 mt-2 truncate">{tpl.subject}</p>
                    <p className="text-xs text-gray-400 dark:text-slate-500 mt-1">{t('letters.updatedAt', { date: formatDate(tpl.updated_at) })}</p>
                  </div>
                  {canPreview && (
                    <button
                      type="button"
                      onClick={() => { void handleLoadTemplate(tpl); }}
                      className="text-left text-sm font-medium text-black dark:text-slate-100 hover:text-gray-600 dark:hover:text-slate-300 transition-colors"
                    >
                      {t('letters.useTemplate')}
                    </button>
                  )}
                </div>
              ))
            )}
          </div>
        )
      )}

      {/* ── Preview Modal ── */}
      <Modal
        open={showPreview && preview !== null}
        onClose={() => setShowPreview(false)}
        title={t('letters.reviewTitle')}
        maxWidth="max-w-2xl"
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button type="button" onClick={() => setShowPreview(false)} className="btn-secondary">
              {t('common.cancel')}
            </button>
            {canSendLetters && (
              <button
                type="button"
                onClick={handleSend}
                disabled={sendMutation.isPending}
                className="btn-primary"
              >
                <Send className="w-4 h-4" />
                {sendMutation.isPending ? t('common.sending') : t('letters.sendEmail')}
              </button>
            )}
          </div>
        }
      >
        {preview && (
          <div className="space-y-4">
            <div>
              <p className="text-xs text-gray-500 dark:text-slate-400 uppercase font-medium">{t('common.to')}</p>
              <p className="text-sm text-gray-900 dark:text-slate-100 break-words">
                {preview.recipient_name
                  ? `${preview.recipient_name} <${preview.recipient_email}>`
                  : preview.recipient_email}
              </p>
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-slate-400 uppercase font-medium">{t('letters.subject')}</p>
              <p className="text-sm font-medium text-gray-900 dark:text-slate-100 break-words">{preview.subject}</p>
            </div>
            <div>
              <p className="text-xs text-gray-500 dark:text-slate-400 uppercase font-medium mb-2">{t('letters.body')}</p>
              <iframe
                title={t('letters.previewTitle')}
                sandbox=""
                className="w-full min-h-[16rem] border border-gray-200 dark:border-slate-800 rounded-lg bg-white"
                srcDoc={preview.body_html}
              />
            </div>

            {sendMutation.isError && (
              <div role="alert" className="bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-lg">
                {getErrorMessage(sendMutation.error, t('letters.sendFailed'))}
              </div>
            )}
            {sendMutation.data && sendMutation.data.status !== 'sent' && sendMutation.data.status !== 'pending' && (
              <div role="alert" className="bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-lg">
                {sendMutation.data.error_message || t('letters.recordedNotSent')}
              </div>
            )}
          </div>
        )}
      </Modal>

      {/* ── Save / Edit Template Modal ── */}
      <Modal
        open={templateModalMode !== null}
        onClose={() => {
          setTemplateModalMode(null);
          setEditingTemplate(null);
          setTemplateFormError('');
        }}
        title={templateModalMode === 'edit' ? t('letters.editTemplate') : t('letters.saveAsTemplate')}
        maxWidth={templateModalMode === 'edit' ? 'max-w-xl' : 'max-w-md'}
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button
              type="button"
              onClick={() => {
                setTemplateModalMode(null);
                setEditingTemplate(null);
                setTemplateFormError('');
              }}
              className="btn-secondary"
            >
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              form={templateFormId}
              disabled={
                !templateName.trim() ||
                (templateModalMode === 'edit' && (!templateSubject.trim() || !templateBodyHtml.trim())) ||
                isSavingTemplate
              }
              className="btn-primary"
            >
              {isSavingTemplate
                ? t('common.saving')
                : templateModalMode === 'edit'
                  ? t('common.saveChanges')
                  : t('letters.saveTemplate')}
            </button>
          </div>
        }
      >
        <form
          id={templateFormId}
          onSubmit={(e) => {
            e.preventDefault();
            setTemplateFormError('');
            const trimmedName = templateName.trim();
            if (!trimmedName) return;
            if (templateModalMode === 'edit' && editingTemplate) {
              const trimmedSubject = templateSubject.trim();
              if (!trimmedSubject || !templateBodyHtml.trim()) {
                setTemplateFormError(t('letters.templateFieldsRequired'));
                return;
              }
              updateTemplateMutation.mutate({
                id: editingTemplate.id,
                name: trimmedName,
                subject: trimmedSubject,
                body_html: templateBodyHtml,
              });
            } else {
              const trimmedSubject = subject.trim();
              if (!trimmedSubject || !bodyHtml.trim()) {
                setTemplateFormError(t('letters.templateFieldsRequired'));
                return;
              }
              saveTemplateMutation.mutate({
                name: trimmedName,
                letter_type: selectedType,
                subject: trimmedSubject,
                body_html: bodyHtml,
              });
            }
          }}
          className="space-y-4"
        >
          {(templateFormError || saveTemplateMutation.isError || updateTemplateMutation.isError) && (
            <div role="alert" className="bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 text-sm px-3 py-2 rounded-lg border border-red-200 dark:border-red-500/30">
              {templateFormError || getErrorMessage(
                saveTemplateMutation.error || updateTemplateMutation.error,
                t('letters.templateSaveFailed'),
              )}
            </div>
          )}
          <FormField label={t('letters.templateName')} required>
            {(aria) => (
              <input
                {...aria}
                type="text"
                value={templateName}
                onChange={(e) => setTemplateName(e.target.value)}
                placeholder={t('letters.templateNamePlaceholder')}
                className="form-input"
                required
              />
            )}
          </FormField>
          {templateModalMode === 'edit' ? (
            <>
              <FormField label={t('letters.subject')} required>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={templateSubject}
                    onChange={(e) => setTemplateSubject(e.target.value)}
                    className="form-input"
                    required
                  />
                )}
              </FormField>
              <FormField label={t('letters.bodyHtml')} required>
                {(aria) => (
                  <textarea
                    {...aria}
                    value={templateBodyHtml}
                    onChange={(e) => setTemplateBodyHtml(e.target.value)}
                    rows={8}
                    className="form-input font-mono text-sm"
                    required
                  />
                )}
              </FormField>
            </>
          ) : (
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('letters.type')}: <span className="font-medium text-gray-900 dark:text-slate-100">{t(`letters.types.${selectedType}`)}</span>
            </p>
          )}
        </form>
      </Modal>
    </div>
  );
}
