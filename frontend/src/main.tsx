import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App'
import { ErrorBoundary } from './components/ErrorBoundary'

// Outermost boundary: catches provider- and router-level throws that no
// boundary inside App can see. The one around the routes is what keeps the
// shell alive when a single page fails; this one is the last resort.
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
)
