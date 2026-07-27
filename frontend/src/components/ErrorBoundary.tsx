import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, Home, RotateCcw } from 'lucide-react';

/**
 * Marker recording that a stale-chunk reload has already been attempted.
 *
 * `sessionStorage`, not `localStorage`: the recovery is per tab, and a stale
 * marker must not survive into tomorrow's session.
 */
const RELOAD_MARKER_KEY = 'chunk-reload-at';

/**
 * How long a reload marker suppresses a second reload. Long enough to cover the
 * reload itself plus a slow first paint, short enough that a genuine second
 * deploy an hour later still recovers on its own.
 */
const RELOAD_GUARD_MS = 10_000;

/**
 * The wordings Chrome, Firefox and Safari use for the same event: a
 * content-hashed chunk that no longer exists.
 */
const STALE_CHUNK_PATTERN =
  /failed to fetch dynamically imported module|importing a module script failed|error loading dynamically imported module|loading chunk .* failed|dynamically imported module/i;

function isStaleChunkError(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  return error.name === 'ChunkLoadError' || STALE_CHUNK_PATTERN.test(error.message);
}

function readReloadMarker(): number | null {
  try {
    const raw = sessionStorage.getItem(RELOAD_MARKER_KEY);
    if (!raw) return null;
    const at = Number(raw);
    return Number.isFinite(at) ? at : null;
  } catch {
    // Private browsing can make sessionStorage throw. Treat that as "no marker"
    // and rely on the fallback UI rather than crashing the boundary itself.
    return null;
  }
}

function writeReloadMarker(at: number): void {
  try {
    sessionStorage.setItem(RELOAD_MARKER_KEY, String(at));
  } catch {
    // See readReloadMarker.
  }
}

function clearReloadMarker(): void {
  try {
    sessionStorage.removeItem(RELOAD_MARKER_KEY);
  } catch {
    // See readReloadMarker.
  }
}

interface ErrorBoundaryProps {
  children: ReactNode;
  // Injection points for tests; production uses the real page and clock.
  reload?: () => void;
  now?: () => number;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Catches render-phase throws so one broken page does not blank the whole app.
 *
 * The trigger this exists for is a deploy, not a bug: every route is loaded
 * through `lazy()`, the deploy job runs `aws s3 sync --delete`, and CloudFront
 * rewrites a 404 to `index.html`. A tab left open across a release therefore
 * requests a chunk that no longer exists, receives HTML with the wrong
 * content-type, and the dynamic import rejects inside `<Suspense>`. With no
 * boundary React 19 unmounts the entire root: a white page with no navigation,
 * recoverable only by a refresh the user has no reason to attempt.
 *
 * So a stale chunk reloads — but exactly once. A broken deploy is
 * indistinguishable from a stale one at this point, and reloading in a loop is
 * strictly worse than showing a page that says what happened.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidMount(): void {
    // Reached the commit phase with a live tree, so whatever the last reload was
    // for is resolved. Leaving the marker would suppress the next genuine one.
    if (!this.state.error) {
      clearReloadMarker();
    }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('Unhandled render error', error, info.componentStack);

    if (!isStaleChunkError(error)) {
      return;
    }

    const now = this.props.now ?? (() => Date.now());
    const at = now();
    const previous = readReloadMarker();
    if (previous !== null && at - previous < RELOAD_GUARD_MS) {
      return;
    }

    writeReloadMarker(at);
    const reload = this.props.reload ?? (() => window.location.reload());
    reload();
  }

  private handleRetry = (): void => {
    clearReloadMarker();
    const reload = this.props.reload ?? (() => window.location.reload());
    reload();
  };

  render(): ReactNode {
    const { error } = this.state;
    if (!error) {
      return this.props.children;
    }

    return (
      <main className="flex min-h-screen items-center justify-center bg-gray-50 px-4 py-12">
        <section className="w-full max-w-lg text-center" aria-labelledby="boundary-title">
          <div className="mx-auto mb-6 flex h-14 w-14 items-center justify-center rounded-2xl bg-red-50 text-red-700 ring-8 ring-red-100">
            <AlertTriangle className="h-7 w-7" aria-hidden="true" />
          </div>

          <h1 id="boundary-title" className="text-3xl font-bold tracking-tight text-gray-950 sm:text-4xl">
            Something went wrong
          </h1>
          <p className="mx-auto mt-4 max-w-md text-base leading-7 text-gray-600">
            This page could not be displayed. Reloading usually fixes it — the application may have
            been updated while your tab was open.
          </p>

          {import.meta.env.DEV && (
            <p className="mx-auto mt-4 max-w-full overflow-x-auto rounded-lg border border-gray-200 bg-white px-3 py-2 text-left font-mono text-xs text-gray-500">
              {error.message}
            </p>
          )}

          <div className="mt-8 flex flex-col-reverse justify-center gap-3 sm:flex-row">
            <a href="/" className="btn-secondary">
              <Home className="h-4 w-4" aria-hidden="true" />
              Go to home
            </a>
            <button type="button" onClick={this.handleRetry} className="btn-primary">
              <RotateCcw className="h-4 w-4" aria-hidden="true" />
              Reload
            </button>
          </div>
        </section>
      </main>
    );
  }
}
