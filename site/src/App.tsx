import { Component, lazy, Suspense } from 'solid-js';
import { Router, Route } from '@solidjs/router';
import HomePage from './pages/HomePage';

const DashboardPage = lazy(() => import('./pages/DashboardPage'));
const DocsPage = lazy(() => import('./pages/DocsPage'));

const PageLoader = () => (
  <div class="flex min-h-screen items-center justify-center bg-[#0a0a0a]">
    <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
  </div>
);

const App: Component = () => {
  return (
    <Router>
      <Route path="/" component={HomePage} />
      <Route
        path="/dashboard"
        component={() => (
          <Suspense fallback={<PageLoader />}>
            <DashboardPage />
          </Suspense>
        )}
      />
      <Route
        path="/docs"
        component={() => (
          <Suspense fallback={<PageLoader />}>
            <DocsPage />
          </Suspense>
        )}
      />
      <Route
        path="/docs/*slug"
        component={() => (
          <Suspense fallback={<PageLoader />}>
            <DocsPage />
          </Suspense>
        )}
      />
    </Router>
  );
};

export default App;
