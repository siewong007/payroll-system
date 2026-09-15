# Internationalization (i18n)

The application is a **four-locale first-class** product:

| Locale | Code | Autonym |
|---|---|---|
| English | `en` | English |
| Bahasa Melayu | `ms` | Bahasa Melayu |
| Simplified Chinese | `zh-CN` | 简体中文 |
| Traditional Chinese | `zh-TW` | 繁體中文 |

`en` is the source of truth for **key structure only** — all four locales carry a
full translation, not a fallback layer.

## Architecture

- **Library**: `i18next` + `react-i18next` + `i18next-browser-languagedetector`
  (`frontend/package.json`).
- **Module**: `frontend/src/i18n/index.ts` — resources, `SUPPORTED_LOCALES`,
  `LOCALE_LABELS`, `intlLocale()`, and locale normalization (any `zh-Hant`,
  `-TW`, `-HK`, `-MO` locale resolves to `zh-TW`; other `zh*` to `zh-CN`).
- **Locale files**: `frontend/src/i18n/locales/{en,ms,zh-CN,zh-TW}.ts`.
  `en.ts` exports `export type Messages = typeof en` and every other locale is
  typed `Messages`, so **structural parity is enforced by `tsc`** — a missing or
  extra key is a compile error.
- **Formatters**: `frontend/src/lib/format.ts` — `formatDate`, `formatDateTime`,
  `monthName`, `weekdayName`, `formatMYR`, `formatRelativeTime`, etc. All take
  `intlLocale()` so dates/numbers follow the active UI locale. **Never** call
  `toLocaleDateString('en-GB', …)` or similar pinned-locale formatters for
  user-facing output; use the helpers (an `i18n-ok` marker is required for the
  rare internal-parse exception).
- **Persistence**: selected language is stored in `localStorage` under
  `payroll.lang` (detector `caches`), so switching survives reloads and does
  not reset app state (React state, route, filters are untouched — only
  strings re-render).
- **Server errors**: backend `AppError` literals are translated client-side via
  `frontend/src/i18n/serverErrorMap.ts` (literal → `serverErrors.*` key) plus
  regex patterns for `format!` messages. `getErrorMessage` in
  `frontend/src/lib/utils.ts` resolves them. Payroll diagnostics/action items
  carry a stable `code` + `params` map from the backend so they translate by
  code, never by parsing English text.

## Key conventions

- Semantic nested keys mirroring the feature: `payroll.detail.approve`,
  `attendance.kiosk.title`, `auth.login.submit`.
- **Never** language-suffixed keys (`foo.en`, `foo.ms`). One key, four values.
- Shared verbs/states live in `common.*` (`save`, `cancel`, `loading`,
  `noResults`). Don't duplicate them per feature unless the context genuinely
  changes the meaning.
- Interpolation: `{{var}}` (i18next `escapeValue: false` — React already
  escapes). Every locale must supply **the same variable set** per key.
- Plurals: `_one` / `_other` suffixes (or base + `_other` when singular equals
  base). Chinese/Malay often keep both forms identical — that's fine, the keys
  must still exist.
- HTML in translations: only `<strong>`-style inline markup via `<Trans>`;
  `t()` renders markup literally.

## Adding a user-facing string

```
New UI text
  → add key to en.ts (semantic namespace)
  → add the same key to ms.ts, zh-CN.ts, zh-TW.ts  (tsc errors if you forget)
  → run: bun run test   (src/tests/i18n.test.ts validates parity)
  → run: node scripts/i18n-scan.mjs --ci   (hardcoded-string gate)
```

`src/tests/i18n.test.ts` enforces: exact key parity, no empty values,
interpolation-var parity, plural pairing, **no en residue** (values identical
to `en` outside a curated allowlist of acronyms/brand names/autonyms), no CJK
in `ms`, **script purity** (Simplified-only chars rejected in `zh-TW`,
Traditional-only chars rejected in `zh-CN` — deny-lists cover the common
differing set; same-in-both chars like 需/限/零/餐 are deliberately excluded).

## Terminology dictionary

### Malay (`ms`)

| English | ms | Notes |
|---|---|---|
| Employee | Pekerja | not `kakitangan` |
| Check in / out | Daftar masuk / Daftar keluar | attendance |
| Payroll | Penggajian | |
| Payslip | Slip gaji | |
| Leave | Cuti | |
| Claim | Tuntutan | expense claim |
| Overtime | Kerja lebih masa | |
| Approval | Kelulusan | |
| Deduction | Potongan | |
| Gross / Net | Kasar / Bersih | pay figures |
| Statutory | Berkanun | EPF/SOCSO/EIS/PCB stay as acronyms |
| Settings | Tetapan | |
| Kiosk | Kiosk | loanword, kept |
| Geofence | Geofencing / Pagar geo | |
| Role | Peranan | |
| Department | Jabatan | |
| Team | Pasukan | |
| Report | Laporan | |
| Backup | Sandaran | |
| Restore | Pulihkan | noun: pemulihan |
| Template | Templat | |
| Attachment | Lampiran | |

### Chinese (`zh-CN` / `zh-TW`)

| English | zh-CN | zh-TW |
|---|---|---|
| Employee | 员工 | 員工 |
| Check in / out | 打卡 / 签退 | 打卡 / 簽退 |
| Payroll | 薪资 | 薪資 |
| Payslip | 工资单 | 薪資單 |
| Leave | 请假 / 假期 | 請假 / 假別 |
| Claim | 报销 / 请款 | 請款 |
| Overtime | 加班 | 加班 |
| Approval | 审批 | 審核 |
| Deduction | 扣款 | 扣款 |
| Gross / Net | 应发 / 实发 | 應發 / 實發 |
| Statutory | 法定 | 法定 |
| Settings | 设置 | 設定 |
| Kiosk | 自助终端 | 自助終端 / Kiosk |
| Geofence | 地理围栏 | 地理圍欄 |
| Role | 角色 | 角色 |
| Department | 部门 | 部門 |
| Team | 团队 | 團隊 |
| Report | 报表 | 報表 |
| Backup | 备份 | 備份 |
| Template | 模板 | 範本 |
| Attachment | 附件 | 附件 |
| Save | 保存 | 儲存 |
| Submit | 提交 | 送出 |
| Default | 默认 | 預設 |
| Reset | 重置 | 重設 |
| View | 查看 | 檢視 |
| Export | 导出 | 匯出 |
| Search | 搜索 | 搜尋 |
| Filter | 筛选 | 篩選 |
| Cancel | 取消 | 取消 |
| Confirm | 确认 | 確認 |

Keep statutory acronyms (EPF, SOCSO, EIS, PCB, MTD, EA, NRIC, TIN) and product
names (Face ID, WebAuthn, Tabung Haji) untranslated in all locales.

## CI gates

- `.github/workflows/ci.yml` → `frontend-lint` runs
  `node scripts/i18n-scan.mjs --ci` (fails on hardcoded user-facing literals;
  backend unmapped messages warn) plus the vitest suite containing
  `i18n.test.ts`.
- The scanner skips lines marked `// i18n-ok` — use it for intentional literals
  only: admin-editable template bodies, statutory CSV headers, internal parse
  formats, test fixtures. Each use should be self-evidently justified.
