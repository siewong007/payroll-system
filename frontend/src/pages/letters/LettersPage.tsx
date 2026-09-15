import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Mail, Send, Eye, Plus, FileText, Clock, CheckCircle, XCircle, X, Users, AtSign } from 'lucide-react';
import { getEmailTemplates, createEmailTemplate, sendLetter, previewLetter, getEmailLogs } from '@/api/email';
import { getEmployee } from '@/api/employees';
import { EmployeePicker } from '@/components/employees/EmployeePicker';
import { formatDate, getErrorMessage } from '@/lib/utils';
import type { LetterType, EmailTemplate, PreviewLetterResponse } from '@/types';

const LETTER_TYPE_VALUES: LetterType[] = [
  'general', 'offer', 'appointment', 'warning', 'termination', 'promotion',
];

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
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-50 text-green-700">
        <CheckCircle className="w-3 h-3" /> {t('letters.sent')}
      </span>
    );
  }
  if (status === 'failed') {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-50 text-red-700">
        <XCircle className="w-3 h-3" /> {t('letters.failed')}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-50 text-yellow-700">
      <Clock className="w-3 h-3" /> {t('enums.requestStatus.pending')}
    </span>
  );
}

type RecipientMode = 'employee' | 'custom';

export function LettersPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'compose' | 'history' | 'templates'>('compose');
  const [selectedType, setSelectedType] = useState<LetterType>('general');
  const [recipientMode, setRecipientMode] = useState<RecipientMode>('employee');
  const [selectedEmployee, setSelectedEmployee] = useState('');
  const [customEmail, setCustomEmail] = useState('');
  const [customName, setCustomName] = useState('');
  const [subject, setSubject] = useState(DEFAULT_TEMPLATES.general.subject);
  const [bodyHtml, setBodyHtml] = useState(DEFAULT_TEMPLATES.general.body);
  const [preview, setPreview] = useState<PreviewLetterResponse | null>(null);
  const [showPreview, setShowPreview] = useState(false);
  const [showSaveTemplate, setShowSaveTemplate] = useState(false);
  const [templateName, setTemplateName] = useState('');

  // One employee, fetched by id — the picker resolves the choice, and this is
  // only here for the "no email on file" warning below.
  const { data: selectedEmp } = useQuery({
    queryKey: ['employee', selectedEmployee],
    queryFn: () => getEmployee(selectedEmployee),
    enabled: Boolean(selectedEmployee),
  });

  const { data: templates } = useQuery({
    queryKey: ['emailTemplates'],
    queryFn: () => getEmailTemplates(),
  });

  const { data: logs } = useQuery({
    queryKey: ['emailLogs'],
    queryFn: () => getEmailLogs({ per_page: 50 }),
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
      // The API answers 502 when the letter was recorded but never delivered,
      // so this guard should be unreachable. It stays because a 200 carrying
      // status:"failed" is exactly what this page used to treat as delivery —
      // clearing the form and unmounting the modal before anyone could read it.
      if (data.status !== 'sent') return;
      setShowPreview(false);
      setPreview(null);
      setSelectedEmployee('');
      setCustomEmail('');
      setCustomName('');
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
      setShowSaveTemplate(false);
      setTemplateName('');
    },
  });

  const handleTypeChange = (type: LetterType) => {
    setSelectedType(type);
    setSubject(DEFAULT_TEMPLATES[type].subject);
    setBodyHtml(DEFAULT_TEMPLATES[type].body);
  };

  const handleLoadTemplate = (template: EmailTemplate) => {
    setSelectedType(template.letter_type);
    setSubject(template.subject);
    setBodyHtml(template.body_html);
    setTab('compose');
  };

  const canSend =
    recipientMode === 'employee'
      ? !!selectedEmployee
      : customEmail.includes('@');

  const handlePreview = () => {
    if (!canSend) return;
    if (recipientMode === 'employee') {
      previewMutation.mutate({
        employee_id: selectedEmployee,
        subject,
        body_html: bodyHtml,
      });
    } else {
      previewMutation.mutate({
        recipient_email: customEmail,
        recipient_name: customName,
        subject,
        body_html: bodyHtml,
      });
    }
  };

  const handleSend = () => {
    if (!canSend) return;
    if (recipientMode === 'employee') {
      sendMutation.mutate({
        employee_id: selectedEmployee,
        letter_type: selectedType,
        subject,
        body_html: bodyHtml,
      });
    } else {
      sendMutation.mutate({
        recipient_email: customEmail,
        recipient_name: customName,
        letter_type: selectedType,
        subject,
        body_html: bodyHtml,
      });
    }
  };

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">{t('letters.title')}</h1>
          <p className="text-sm text-gray-500 mt-1">{t('letters.subtitle')}</p>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 bg-gray-100 p-1 rounded-xl mb-6 w-fit">
        {(['compose', 'history', 'templates'] as const).map((tabKey) => (
          <button
            key={tabKey}
            onClick={() => setTab(tabKey)}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors capitalize ${
              tab === tabKey ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            {tabKey === 'compose' && <span className="flex items-center gap-1.5"><Mail className="w-4 h-4" /> {t('letters.tabs.compose')}</span>}
            {tabKey === 'history' && <span className="flex items-center gap-1.5"><Clock className="w-4 h-4" /> {t('letters.tabs.history')}</span>}
            {tabKey === 'templates' && <span className="flex items-center gap-1.5"><FileText className="w-4 h-4" /> {t('letters.tabs.templates')}</span>}
          </button>
        ))}
      </div>

      {/* ── Compose Tab ── */}
      {tab === 'compose' && (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left: Compose form */}
          <div className="lg:col-span-2 space-y-4">
            {/* Letter Type */}
            <div className="bg-white rounded-2xl shadow p-6">
              <h2 className="text-sm font-semibold text-gray-900 mb-3">{t('letters.type')}</h2>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
                {LETTER_TYPE_VALUES.map((ltv) => (
                  <button
                    key={ltv}
                    onClick={() => handleTypeChange(ltv)}
                    className={`text-left p-3 rounded-xl border-2 transition-colors ${
                      selectedType === ltv
                        ? 'border-black bg-gray-50'
                        : 'border-gray-100 hover:border-gray-200'
                    }`}
                  >
                    <p className="text-sm font-medium text-gray-900">{t(`letters.types.${ltv}`)}</p>
                    <p className="text-xs text-gray-500 mt-0.5">{t(`letters.types.${ltv}Description`)}</p>
                  </button>
                ))}
              </div>
            </div>

            {/* Recipient */}
            <div className="bg-white rounded-2xl shadow p-6">
              <div className="flex items-center justify-between mb-3">
                <h2 className="text-sm font-semibold text-gray-900">{t('letters.recipient')}</h2>
                <div className="flex gap-1 bg-gray-100 p-0.5 rounded-lg">
                  <button
                    onClick={() => setRecipientMode('employee')}
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                      recipientMode === 'employee'
                        ? 'bg-white text-gray-900 shadow-sm'
                        : 'text-gray-500 hover:text-gray-700'
                    }`}
                  >
                    <Users className="w-3.5 h-3.5" />
                    {t('common.employee')}
                  </button>
                  <button
                    onClick={() => setRecipientMode('custom')}
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                      recipientMode === 'custom'
                        ? 'bg-white text-gray-900 shadow-sm'
                        : 'text-gray-500 hover:text-gray-700'
                    }`}
                  >
                    <AtSign className="w-3.5 h-3.5" />
                    {t('letters.customEmail')}
                  </button>
                </div>
              </div>

              {recipientMode === 'employee' ? (
                <>
                  {/* `isActive={null}` — a termination or an EA-form letter is
                      routinely addressed to someone who has already left. */}
                  <EmployeePicker
                    value={selectedEmployee}
                    onChange={(id) => setSelectedEmployee(id)}
                    isActive={null}
                    placeholder={t('letters.searchEmployee')}
                  />
                  {selectedEmp && !selectedEmp.email && (
                    <p className="text-xs text-red-500 mt-2">{t('letters.noEmailOnFile')}</p>
                  )}
                  {selectedEmp?.email && (
                    <p className="text-xs text-gray-400 mt-2">{t('common.email')}: {selectedEmp.email}</p>
                  )}
                </>
              ) : (
                <div className="space-y-3">
                  <div>
                    <label className="block text-xs font-medium text-gray-500 mb-1">{t('common.email')}</label>
                    <input
                      type="email"
                      value={customEmail}
                      onChange={(e) => setCustomEmail(e.target.value)}
                      // i18n-ok: email-format example
                      placeholder="recipient@example.com" 
                      className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-500 mb-1">{t('letters.recipientNameOptional')}</label>
                    <input
                      type="text"
                      value={customName}
                      onChange={(e) => setCustomName(e.target.value)}
                      // i18n-ok: example name
                      placeholder="John Doe" 
                      className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                    />
                  </div>
                </div>
              )}
            </div>

            {/* Subject & Body */}
            <div className="bg-white rounded-2xl shadow p-6 space-y-4">
              <div>
                <label className="block text-sm font-semibold text-gray-900 mb-1">{t('letters.subject')}</label>
                <input
                  type="text"
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                  placeholder={t('letters.subjectPlaceholder')}
                />
                <p className="text-xs text-gray-400 mt-1">
                  {/* i18n-ok: merge-field identifiers are API values, not copy */}
                  {t('letters.variables')}: {'{{employee_name}}'}, {'{{employee_number}}'}, {'{{company_name}}'}, {'{{designation}}'}, {'{{department}}'}, {'{{date_joined}}'}
                </p>
              </div>
              <div>
                <label className="block text-sm font-semibold text-gray-900 mb-1">{t('letters.bodyHtml')}</label>
                <textarea
                  value={bodyHtml}
                  onChange={(e) => setBodyHtml(e.target.value)}
                  rows={12}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none font-mono text-sm"
                />
              </div>

              {/* Action buttons */}
              <div className="flex flex-wrap gap-3">
                <button
                  onClick={handlePreview}
                  disabled={!canSend || previewMutation.isPending}
                  className="flex items-center gap-2 bg-gray-100 text-gray-700 px-4 py-2.5 rounded-lg font-medium hover:bg-gray-200 disabled:opacity-50 transition-colors text-sm"
                >
                  <Eye className="w-4 h-4" />
                  {previewMutation.isPending ? t('common.loading') : t('letters.preview')}
                </button>
                <button
                  onClick={() => setShowSaveTemplate(true)}
                  className="flex items-center gap-2 bg-gray-100 text-gray-700 px-4 py-2.5 rounded-lg font-medium hover:bg-gray-200 transition-colors text-sm"
                >
                  <Plus className="w-4 h-4" />
                  {t('letters.saveAsTemplate')}
                </button>
              </div>
            </div>
          </div>

          {/* Right: Quick templates sidebar */}
          <div className="space-y-4">
            <div className="bg-white rounded-2xl shadow p-6">
              <h2 className="text-sm font-semibold text-gray-900 mb-3">{t('letters.savedTemplates')}</h2>
              {(!templates || templates.length === 0) ? (
                <p className="text-sm text-gray-400">{t('letters.noSavedTemplates')}</p>
              ) : (
                <div className="space-y-2">
                  {templates.map((tpl) => (
                    <button
                      key={tpl.id}
                      onClick={() => handleLoadTemplate(tpl)}
                      className="w-full text-left p-3 rounded-xl border border-gray-100 hover:border-gray-200 hover:bg-gray-50 transition-colors"
                    >
                      <p className="text-sm font-medium text-gray-900">{tpl.name}</p>
                      <p className="text-xs text-gray-400 mt-0.5">{t(`letters.types.${tpl.letter_type}`, { defaultValue: tpl.letter_type })}</p>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ── History Tab ── */}
      {tab === 'history' && (
        <div className="bg-white rounded-2xl shadow">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-100">
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">{t('common.date')}</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">{t('letters.type')}</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">{t('letters.recipient')}</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">{t('letters.subject')}</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">{t('common.status')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-50">
                {(!logs || logs.data.length === 0) ? (
                  <tr>
                    <td colSpan={5} className="px-6 py-12 text-center text-gray-400">
                      <Mail className="w-8 h-8 mx-auto mb-2 opacity-30" />
                      {t('letters.noEmails')}
                    </td>
                  </tr>
                ) : (
                  logs.data.map((log) => (
                    <tr key={log.id} className="hover:bg-gray-50">
                      <td className="px-6 py-3 text-gray-500">{formatDate(log.created_at)}</td>
                      <td className="px-6 py-3">
                        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-700">
                          {t(`letters.types.${log.letter_type}`, { defaultValue: log.letter_type })}
                        </span>
                      </td>
                      <td className="px-6 py-3">
                        <p className="font-medium text-gray-900">{log.recipient_name || '-'}</p>
                        <p className="text-xs text-gray-400">{log.recipient_email}</p>
                      </td>
                      <td className="px-6 py-3 text-gray-700 max-w-xs truncate">{log.subject}</td>
                      <td className="px-6 py-3">
                        <StatusBadge status={log.status} />
                        {log.error_message && (
                          <p className="text-xs text-red-500 mt-1">{log.error_message}</p>
                        )}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ── Templates Tab ── */}
      {tab === 'templates' && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {(!templates || templates.length === 0) ? (
            <div className="col-span-full text-center py-12 text-gray-400">
              <FileText className="w-8 h-8 mx-auto mb-2 opacity-30" />
              <p>{t('letters.noTemplates')}</p>
            </div>
          ) : (
            templates.map((tpl) => (
              <div key={tpl.id} className="bg-white rounded-2xl shadow p-6">
                <div className="flex items-start justify-between mb-2">
                  <div>
                    <h3 className="font-semibold text-gray-900">{tpl.name}</h3>
                    <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600">
                      {t(`letters.types.${tpl.letter_type}`, { defaultValue: tpl.letter_type })}
                    </span>
                  </div>
                </div>
                <p className="text-sm text-gray-500 mt-2 truncate">{tpl.subject}</p>
                <p className="text-xs text-gray-400 mt-1">{t('letters.updatedAt', { date: formatDate(tpl.updated_at) })}</p>
                <button
                  onClick={() => handleLoadTemplate(tpl)}
                  className="mt-3 text-sm font-medium text-black hover:text-gray-600 transition-colors"
                >
                  {t('letters.useTemplate')}
                </button>
              </div>
            ))
          )}
        </div>
      )}

      {/* ── Preview Modal ── */}
      {showPreview && preview && (
        <div className="fixed inset-0 bg-black/40 z-50 flex items-center justify-center p-4">
          <div className="bg-white rounded-2xl shadow-xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
            <div className="flex items-center justify-between p-6 border-b border-gray-100">
              <h2 className="text-lg font-semibold">{t('letters.reviewTitle')}</h2>
              <button onClick={() => setShowPreview(false)} className="text-gray-400 hover:text-gray-600">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              <div>
                <p className="text-xs text-gray-400 uppercase font-medium">{t('common.to')}</p>
                <p className="text-sm">
                  {preview.recipient_name
                    ? `${preview.recipient_name} <${preview.recipient_email}>`
                    : preview.recipient_email}
                </p>
              </div>
              <div>
                <p className="text-xs text-gray-400 uppercase font-medium">{t('letters.subject')}</p>
                <p className="text-sm font-medium">{preview.subject}</p>
              </div>
              <div>
                <p className="text-xs text-gray-400 uppercase font-medium mb-2">{t('letters.body')}</p>
                {/*
                  Render the server-rendered letter HTML inside a sandboxed
                  iframe. The sandbox attribute (no `allow-scripts`) neutralizes
                  any script/event-handler that may have been injected via
                  employee-controlled merge fields, so the preview cannot run
                  code in the admin's session. Replaces dangerouslySetInnerHTML.
                */}
                <iframe
                  title={t('letters.previewTitle')}
                  sandbox=""
                  className="w-full min-h-[16rem] border border-gray-200 rounded-lg bg-white"
                  srcDoc={preview.body_html}
                />
              </div>

              {/*
                `getErrorMessage` reads the API's own text out of the axios
                error. Rendering `error.message` gave "Request failed with
                status code 502" and left the operator none the wiser about
                which mail server refused what.
              */}
              {sendMutation.isError && (
                <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-lg">
                  {getErrorMessage(sendMutation.error, t('letters.sendFailed'))}
                </div>
              )}
              {sendMutation.data && sendMutation.data.status !== 'sent' && (
                <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-lg">
                  {sendMutation.data.error_message || t('letters.recordedNotSent')}
                </div>
              )}
            </div>
            <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
              <button
                onClick={() => setShowPreview(false)}
                className="px-4 py-2.5 rounded-lg text-sm font-medium text-gray-700 border border-gray-200 hover:bg-gray-50 transition-colors"
              >
                {t('common.cancel')}
              </button>
              <button
                onClick={handleSend}
                disabled={sendMutation.isPending}
                className="flex items-center gap-2 bg-black text-white px-6 py-2.5 rounded-lg font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors text-sm"
              >
                <Send className="w-4 h-4" />
                {sendMutation.isPending ? t('common.sending') : t('letters.sendEmail')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Save Template Modal ── */}
      {showSaveTemplate && (
        <div className="fixed inset-0 bg-black/40 z-50 flex items-center justify-center p-4">
          <div className="bg-white rounded-2xl shadow-xl max-w-md w-full">
            <div className="flex items-center justify-between p-6 border-b border-gray-100">
              <h2 className="text-lg font-semibold">{t('letters.saveAsTemplate')}</h2>
              <button onClick={() => setShowSaveTemplate(false)} className="text-gray-400 hover:text-gray-600">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">{t('letters.templateName')}</label>
                <input
                  type="text"
                  value={templateName}
                  onChange={(e) => setTemplateName(e.target.value)}
                  placeholder={t('letters.templateNamePlaceholder')}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                />
              </div>
              <p className="text-sm text-gray-500">
                {t('letters.type')}: <span className="font-medium">{t(`letters.types.${selectedType}`)}</span>
              </p>
            </div>
            <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
              <button
                onClick={() => setShowSaveTemplate(false)}
                className="px-4 py-2.5 rounded-lg text-sm font-medium text-gray-700 border border-gray-200 hover:bg-gray-50 transition-colors"
              >
                {t('common.cancel')}
              </button>
              <button
                onClick={() => {
                  if (!templateName.trim()) return;
                  saveTemplateMutation.mutate({
                    name: templateName,
                    letter_type: selectedType,
                    subject,
                    body_html: bodyHtml,
                  });
                }}
                disabled={!templateName.trim() || saveTemplateMutation.isPending}
                className="bg-black text-white px-6 py-2.5 rounded-lg font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors text-sm"
              >
                {saveTemplateMutation.isPending ? t('common.saving') : t('letters.saveTemplate')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
