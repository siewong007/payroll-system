import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { ErrorPage } from '@/pages/errors/ErrorPage';

describe('ErrorPage', () => {
  it('renders the status, requested path, and a safe home link', () => {
    render(
      <MemoryRouter>
        <ErrorPage
          status={404}
          title="Page not found"
          description="The requested page does not exist."
          homePath="/company"
          path="/missing-page"
        />
      </MemoryRouter>,
    );

    expect(screen.getByRole('heading', { name: 'Page not found' })).toBeInTheDocument();
    expect(screen.getByText('ERROR 404')).toBeInTheDocument();
    expect(screen.getByText('/missing-page')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Go to home' })).toHaveAttribute('href', '/company');
  });

  it('returns to the previous route from the back action', async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={['/previous', '/403']} initialIndex={1}>
        <Routes>
          <Route
            path="/403"
            element={(
              <ErrorPage
                status={403}
                title="Access denied"
                description="You do not have permission to view this page."
                homePath="/company"
              />
            )}
          />
          <Route path="/previous" element={<p>Previous page</p>} />
        </Routes>
      </MemoryRouter>,
    );

    await user.click(screen.getByRole('button', { name: 'Go back' }));

    expect(screen.getByText('Previous page')).toBeInTheDocument();
  });
});
