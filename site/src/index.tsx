import { render } from 'solid-js/web';
import { QueryClientProvider } from '@tanstack/solid-query';
import { queryClient } from './lib/query';
import App from './App';
import './index.css';

render(
  () => (
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  ),
  document.getElementById('root')!
);
