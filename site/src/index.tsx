import { render } from 'solid-js/web';
import App from './App';
import './index.css';

render(() => <App />, document.getElementById('root')!);

if (import.meta.env.PROD) {
  requestIdleCallback(
    async () => {
      const Sentry = await import('@sentry/solid');
      Sentry.init({
        dsn: import.meta.env.VITE_SENTRY_DSN,
        environment: import.meta.env.MODE,
        integrations: [Sentry.browserTracingIntegration()],
        tracesSampleRate: 0.1,
      });
    },
    { timeout: 3000 }
  );
}
