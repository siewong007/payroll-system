import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router';
import { describe, expect, it, vi } from 'vitest';
import { AuthContext, type AuthContextType } from '@/context/AuthContext';
import { ThemeProvider } from '@/context/ThemeProvider';
import { LegalPage } from '@/pages/legal/LegalPage';
import { payrollDiagnosticText } from '@/pages/payroll/statusMeta';

const unauthenticatedContext: AuthContextType = {
  user: null,
  token: null,
  login: vi.fn(),
  completeTwoFactorLogin: vi.fn(),
  logout: vi.fn(),
  switchCompany: vi.fn(),
  setSession: vi.fn(),
  isAuthenticated: false,
  isLoading: false,
};

function renderLegal(initialPath: string) {
  return render(
    <ThemeProvider>
      <AuthContext.Provider value={unauthenticatedContext}>
        <MemoryRouter initialEntries={[initialPath]}>
          <Routes>
            <Route path="/terms" element={<LegalPage />} />
            <Route path="/privacy" element={<LegalPage />} />
            <Route path="/compliance" element={<LegalPage />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </ThemeProvider>,
  );
}

describe('LegalPage & Malaysian statutory diagnostics', () => {
  it('renders Terms & Conditions and navigates across Privacy/PDPA and Statutory tabs', async () => {
    const user = userEvent.setup();
    renderLegal('/terms');

    expect(
      screen.getByRole('heading', { name: /terms and conditions of use/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /back to sign in/i })).toHaveAttribute(
      'href',
      '/login',
    );

    await user.click(screen.getByRole('link', { name: /privacy & pdpa notice/i }));
    expect(
      screen.getByRole('heading', {
        name: /privacy policy & pdpa personal data protection notice/i,
      }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('link', { name: /malaysian statutory guide/i }));
    expect(
      screen.getByRole('heading', {
        name: /malaysian employment, tax & statutory compliance reference/i,
      }),
    ).toBeInTheDocument();
  });

  it('formats below_minimum_wage diagnostics with localized MYR amounts', () => {
    const text = payrollDiagnosticText({
      code: 'below_minimum_wage',
      message: 'Contracted basic salary is below minimum wage',
      employee_id: 'emp-1',
      employee_number: 'E-001',
      employee_name: 'Ahmad',
      params: {
        salary: '150000',
        minimum: '170000',
      },
    });

    expect(text).toContain('1,500.00');
    expect(text).toContain('1,700.00');
    expect(text).toContain('Minimum Wages Order 2024');
  });
});
