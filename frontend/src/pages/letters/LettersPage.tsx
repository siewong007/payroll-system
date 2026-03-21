import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Mail, Send, Eye, Plus, FileText, Clock, CheckCircle, XCircle, X } from 'lucide-react';
import { getEmailTemplates, createEmailTemplate, sendLetter, previewLetter, getEmailLogs } from '@/api/email';
import { getEmployees } from '@/api/employees';
import { formatDate } from '@/lib/utils';
import type { LetterType, EmailTemplate, PreviewLetterResponse } from '@/types';

const LETTER_TYPES: { value: LetterType; label: string; description: string }[] = [
  { value: 'offer', label: 'Offer Letter', description: 'Sent to a candidate after selection' },
  { value: 'appointment', label: 'Appointment Letter', description: 'Formal confirmation of employment' },
  { value: 'warning', label: 'Warning Letter', description: 'Issued for misconduct or poor performance' },
  { value: 'termination', label: 'Termination Letter', description: 'When employment is ended by the company' },
  { value: 'promotion', label: 'Promotion Letter', description: 'Confirms employee promotion' },
];

const LETTER_TYPE_LABELS: Record<string, string> = {
  welcome: 'Welcome',
  offer: 'Offer Letter',
  appointment: 'Appointment Letter',
  warning: 'Warning Letter',
  termination: 'Termination Letter',
  promotion: 'Promotion Letter',
};

const DEFAULT_TEMPLATES: Record<LetterType, { subject: string; body: string }> = {
  welcome: {
    subject: 'Welcome to {{company_name}}',
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Welcome to <strong>{{company_name}}</strong>! We are delighted to have you join our team.</p><p>Your employee number is <strong>{{employee_number}}</strong> and your start date is <strong>{{date_joined}}</strong>.</p><p>Best regards,<br>{{company_name}} HR Team</p>',
  },
  offer: {
    subject: 'Offer of Employment - {{company_name}}',
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>We are pleased to extend this offer of employment for the position of <strong>{{designation}}</strong> in the <strong>{{department}}</strong> department at <strong>{{company_name}}</strong>.</p><p><strong>Start Date:</strong> {{date_joined}}</p><p>Please review the terms and conditions of this offer. We look forward to welcoming you to the team.</p><p>Sincerely,<br>{{company_name}} HR Team</p>',
  },
  appointment: {
    subject: 'Appointment Letter - {{company_name}}',
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Following your acceptance of our offer, we are pleased to formally appoint you as <strong>{{designation}}</strong> in the <strong>{{department}}</strong> department at <strong>{{company_name}}</strong>.</p><p><strong>Employee Number:</strong> {{employee_number}}<br><strong>Date of Joining:</strong> {{date_joined}}</p><p>This letter serves as formal confirmation of your employment. The detailed terms and conditions are as discussed.</p><p>We look forward to your contributions to the team.</p><p>Best regards,<br>{{company_name}} HR Team</p>',
  },
  warning: {
    subject: 'Warning Letter - {{company_name}}',
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Employee Number: <strong>{{employee_number}}</strong><br>Department: <strong>{{department}}</strong></p><p>This letter serves as a formal warning regarding [describe the issue]. This behaviour/performance is in violation of company policy.</p><p>We expect immediate improvement. Failure to comply may result in further disciplinary action.</p><p>Please acknowledge receipt of this letter.</p><p>Regards,<br>{{company_name}} HR Department</p>',
  },
  termination: {
    subject: 'Termination of Employment - {{company_name}}',
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>Employee Number: <strong>{{employee_number}}</strong><br>Department: <strong>{{department}}</strong></p><p>We regret to inform you that your employment with <strong>{{company_name}}</strong> is terminated effective [last working day].</p><p><strong>Reason:</strong> [Describe reason]</p><p>Please arrange to return all company property. Settlement of your final pay and benefits will be processed accordingly.</p><p>Regards,<br>{{company_name}} HR Department</p>',
  },
  promotion: {
    subject: 'Congratulations on Your Promotion - {{company_name}}',
    body: '<p>Dear <strong>{{employee_name}}</strong>,</p><p>We are delighted to inform you that you have been promoted to the position of <strong>[New Position]</strong> effective [effective date].</p><p>This promotion is in recognition of your hard work and contributions to <strong>{{company_name}}</strong>.</p><p>Your new role will include [briefly describe new responsibilities].</p><p>Congratulations and best wishes in your new role!</p><p>Best regards,<br>{{company_name}} HR Team</p>',
  },
};

function StatusBadge({ status }: { status: string }) {
  if (status === 'sent') {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-50 text-green-700">
        <CheckCircle className="w-3 h-3" /> Sent
      </span>
    );
  }
  if (status === 'failed') {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-50 text-red-700">
        <XCircle className="w-3 h-3" /> Failed
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-50 text-yellow-700">
      <Clock className="w-3 h-3" /> Pending
    </span>
  );
}

export function LettersPage() {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'compose' | 'history' | 'templates'>('compose');
  const [selectedType, setSelectedType] = useState<LetterType>('offer');
  const [selectedEmployee, setSelectedEmployee] = useState('');
  const [subject, setSubject] = useState(DEFAULT_TEMPLATES.offer.subject);
  const [bodyHtml, setBodyHtml] = useState(DEFAULT_TEMPLATES.offer.body);
  const [preview, setPreview] = useState<PreviewLetterResponse | null>(null);
  const [showPreview, setShowPreview] = useState(false);
  const [employeeSearch, setEmployeeSearch] = useState('');
  const [showEmployeeDropdown, setShowEmployeeDropdown] = useState(false);
  const [showSaveTemplate, setShowSaveTemplate] = useState(false);
  const [templateName, setTemplateName] = useState('');

  const { data: employees } = useQuery({
    queryKey: ['employees-select'],
    queryFn: () => getEmployees({ per_page: 200 }),
  });

  const { data: templates } = useQuery({
    queryKey: ['emailTemplates'],
    queryFn: () => getEmailTemplates(),
  });

  const { data: logs } = useQuery({
    queryKey: ['emailLogs'],
    queryFn: () => getEmailLogs({ per_page: 50 }),
  });

  const previewMutation = useMutation({
    mutationFn: previewLetter,
    onSuccess: (data) => {
      setPreview(data);
      setShowPreview(true);
    },
  });

  const sendMutation = useMutation({
    mutationFn: sendLetter,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['emailLogs'] });
      setShowPreview(false);
      setPreview(null);
      setSelectedEmployee('');
      setEmployeeSearch('');
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

  const handlePreview = () => {
    if (!selectedEmployee) return;
    previewMutation.mutate({
      employee_id: selectedEmployee,
      subject,
      body_html: bodyHtml,
    });
  };

  const handleSend = () => {
    if (!selectedEmployee) return;
    sendMutation.mutate({
      employee_id: selectedEmployee,
      letter_type: selectedType,
      subject,
      body_html: bodyHtml,
    });
  };

  const filteredEmployees = employees?.data.filter(
    (emp) =>
      emp.full_name.toLowerCase().includes(employeeSearch.toLowerCase()) ||
      emp.employee_number.toLowerCase().includes(employeeSearch.toLowerCase())
  );

  const selectedEmp = employees?.data.find((e) => e.id === selectedEmployee);

  return (
    <div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">Letters & Email</h1>
          <p className="text-sm text-gray-500 mt-1">Compose and send HR letters to employees</p>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 bg-gray-100 p-1 rounded-xl mb-6 w-fit">
        {(['compose', 'history', 'templates'] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors capitalize ${
              tab === t ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            {t === 'compose' && <span className="flex items-center gap-1.5"><Mail className="w-4 h-4" /> Compose</span>}
            {t === 'history' && <span className="flex items-center gap-1.5"><Clock className="w-4 h-4" /> History</span>}
            {t === 'templates' && <span className="flex items-center gap-1.5"><FileText className="w-4 h-4" /> Templates</span>}
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
              <h2 className="text-sm font-semibold text-gray-900 mb-3">Letter Type</h2>
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
                {LETTER_TYPES.map((lt) => (
                  <button
                    key={lt.value}
                    onClick={() => handleTypeChange(lt.value)}
                    className={`text-left p-3 rounded-xl border-2 transition-colors ${
                      selectedType === lt.value
                        ? 'border-black bg-gray-50'
                        : 'border-gray-100 hover:border-gray-200'
                    }`}
                  >
                    <p className="text-sm font-medium text-gray-900">{lt.label}</p>
                    <p className="text-xs text-gray-500 mt-0.5">{lt.description}</p>
                  </button>
                ))}
              </div>
            </div>

            {/* Recipient */}
            <div className="bg-white rounded-2xl shadow p-6">
              <h2 className="text-sm font-semibold text-gray-900 mb-3">Recipient</h2>
              <div className="relative">
                <input
                  type="text"
                  value={selectedEmp ? `${selectedEmp.full_name} (${selectedEmp.employee_number})` : employeeSearch}
                  onChange={(e) => {
                    setEmployeeSearch(e.target.value);
                    setSelectedEmployee('');
                    setShowEmployeeDropdown(true);
                  }}
                  onFocus={() => setShowEmployeeDropdown(true)}
                  placeholder="Search employee by name or number..."
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                />
                {selectedEmployee && (
                  <button
                    onClick={() => { setSelectedEmployee(''); setEmployeeSearch(''); }}
                    className="absolute right-2 top-2.5 text-gray-400 hover:text-gray-600"
                  >
                    <X className="w-4 h-4" />
                  </button>
                )}
                {showEmployeeDropdown && !selectedEmployee && filteredEmployees && filteredEmployees.length > 0 && (
                  <div className="absolute z-10 mt-1 w-full bg-white border border-gray-200 rounded-lg shadow-lg max-h-48 overflow-y-auto">
                    {filteredEmployees.slice(0, 20).map((emp) => (
                      <button
                        key={emp.id}
                        onClick={() => {
                          setSelectedEmployee(emp.id);
                          setEmployeeSearch('');
                          setShowEmployeeDropdown(false);
                        }}
                        className="w-full text-left px-3 py-2 hover:bg-gray-50 text-sm flex justify-between"
                      >
                        <span className="font-medium">{emp.full_name}</span>
                        <span className="text-gray-400">{emp.employee_number}</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
              {selectedEmp && !selectedEmp.email && (
                <p className="text-xs text-red-500 mt-2">This employee has no email address on file.</p>
              )}
              {selectedEmp?.email && (
                <p className="text-xs text-gray-400 mt-2">Email: {selectedEmp.email}</p>
              )}
            </div>

            {/* Subject & Body */}
            <div className="bg-white rounded-2xl shadow p-6 space-y-4">
              <div>
                <label className="block text-sm font-semibold text-gray-900 mb-1">Subject</label>
                <input
                  type="text"
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                />
                <p className="text-xs text-gray-400 mt-1">
                  Variables: {'{{employee_name}}'}, {'{{employee_number}}'}, {'{{company_name}}'}, {'{{designation}}'}, {'{{department}}'}, {'{{date_joined}}'}
                </p>
              </div>
              <div>
                <label className="block text-sm font-semibold text-gray-900 mb-1">Body (HTML)</label>
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
                  disabled={!selectedEmployee || previewMutation.isPending}
                  className="flex items-center gap-2 bg-gray-100 text-gray-700 px-4 py-2.5 rounded-lg font-medium hover:bg-gray-200 disabled:opacity-50 transition-colors text-sm"
                >
                  <Eye className="w-4 h-4" />
                  {previewMutation.isPending ? 'Loading...' : 'Preview'}
                </button>
                <button
                  onClick={() => setShowSaveTemplate(true)}
                  className="flex items-center gap-2 bg-gray-100 text-gray-700 px-4 py-2.5 rounded-lg font-medium hover:bg-gray-200 transition-colors text-sm"
                >
                  <Plus className="w-4 h-4" />
                  Save as Template
                </button>
              </div>
            </div>
          </div>

          {/* Right: Quick templates sidebar */}
          <div className="space-y-4">
            <div className="bg-white rounded-2xl shadow p-6">
              <h2 className="text-sm font-semibold text-gray-900 mb-3">Saved Templates</h2>
              {(!templates || templates.length === 0) ? (
                <p className="text-sm text-gray-400">No saved templates yet.</p>
              ) : (
                <div className="space-y-2">
                  {templates.map((t) => (
                    <button
                      key={t.id}
                      onClick={() => handleLoadTemplate(t)}
                      className="w-full text-left p-3 rounded-xl border border-gray-100 hover:border-gray-200 hover:bg-gray-50 transition-colors"
                    >
                      <p className="text-sm font-medium text-gray-900">{t.name}</p>
                      <p className="text-xs text-gray-400 mt-0.5">{LETTER_TYPE_LABELS[t.letter_type] || t.letter_type}</p>
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
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Date</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Type</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Recipient</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Subject</th>
                  <th className="text-left px-6 py-3 text-xs font-medium text-gray-500 uppercase">Status</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-50">
                {(!logs || logs.data.length === 0) ? (
                  <tr>
                    <td colSpan={5} className="px-6 py-12 text-center text-gray-400">
                      <Mail className="w-8 h-8 mx-auto mb-2 opacity-30" />
                      No emails sent yet
                    </td>
                  </tr>
                ) : (
                  logs.data.map((log) => (
                    <tr key={log.id} className="hover:bg-gray-50">
                      <td className="px-6 py-3 text-gray-500">{formatDate(log.created_at)}</td>
                      <td className="px-6 py-3">
                        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-700">
                          {LETTER_TYPE_LABELS[log.letter_type] || log.letter_type}
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
              <p>No saved templates. Create one from the Compose tab.</p>
            </div>
          ) : (
            templates.map((t) => (
              <div key={t.id} className="bg-white rounded-2xl shadow p-6">
                <div className="flex items-start justify-between mb-2">
                  <div>
                    <h3 className="font-semibold text-gray-900">{t.name}</h3>
                    <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 text-gray-600">
                      {LETTER_TYPE_LABELS[t.letter_type] || t.letter_type}
                    </span>
                  </div>
                </div>
                <p className="text-sm text-gray-500 mt-2 truncate">{t.subject}</p>
                <p className="text-xs text-gray-400 mt-1">Updated {formatDate(t.updated_at)}</p>
                <button
                  onClick={() => handleLoadTemplate(t)}
                  className="mt-3 text-sm font-medium text-black hover:text-gray-600 transition-colors"
                >
                  Use Template
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
              <h2 className="text-lg font-semibold">Review Email Before Sending</h2>
              <button onClick={() => setShowPreview(false)} className="text-gray-400 hover:text-gray-600">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              <div>
                <p className="text-xs text-gray-400 uppercase font-medium">To</p>
                <p className="text-sm">{preview.recipient_name} &lt;{preview.recipient_email}&gt;</p>
              </div>
              <div>
                <p className="text-xs text-gray-400 uppercase font-medium">Subject</p>
                <p className="text-sm font-medium">{preview.subject}</p>
              </div>
              <div>
                <p className="text-xs text-gray-400 uppercase font-medium mb-2">Body</p>
                <div
                  className="border border-gray-200 rounded-lg p-4 prose prose-sm max-w-none"
                  dangerouslySetInnerHTML={{ __html: preview.body_html }}
                />
              </div>

              {sendMutation.isError && (
                <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-lg">
                  {(sendMutation.error as Error)?.message || 'Failed to send email'}
                </div>
              )}
              {sendMutation.isSuccess && (
                <div className="bg-green-50 text-green-600 text-sm px-4 py-3 rounded-lg">
                  Email sent successfully! Check the History tab for details.
                </div>
              )}
            </div>
            <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
              <button
                onClick={() => setShowPreview(false)}
                className="px-4 py-2.5 rounded-lg text-sm font-medium text-gray-700 border border-gray-200 hover:bg-gray-50 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleSend}
                disabled={sendMutation.isPending || sendMutation.isSuccess}
                className="flex items-center gap-2 bg-black text-white px-6 py-2.5 rounded-lg font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors text-sm"
              >
                <Send className="w-4 h-4" />
                {sendMutation.isPending ? 'Sending...' : 'Send Email'}
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
              <h2 className="text-lg font-semibold">Save as Template</h2>
              <button onClick={() => setShowSaveTemplate(false)} className="text-gray-400 hover:text-gray-600">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">Template Name</label>
                <input
                  type="text"
                  value={templateName}
                  onChange={(e) => setTemplateName(e.target.value)}
                  placeholder="e.g., Standard Offer Letter"
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                />
              </div>
              <p className="text-sm text-gray-500">
                Type: <span className="font-medium">{LETTER_TYPE_LABELS[selectedType]}</span>
              </p>
            </div>
            <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
              <button
                onClick={() => setShowSaveTemplate(false)}
                className="px-4 py-2.5 rounded-lg text-sm font-medium text-gray-700 border border-gray-200 hover:bg-gray-50 transition-colors"
              >
                Cancel
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
                {saveTemplateMutation.isPending ? 'Saving...' : 'Save Template'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
