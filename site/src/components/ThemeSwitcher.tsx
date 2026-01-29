import { Component, createSignal, createEffect, onMount, onCleanup } from 'solid-js';

type Theme = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'omg-theme-preference';

const ThemeSwitcher: Component = () => {
  const [theme, setTheme] = createSignal<Theme>('system');
  const [resolvedTheme, setResolvedTheme] = createSignal<'light' | 'dark'>('dark');

  const getSystemTheme = (): 'light' | 'dark' => {
    if (typeof window === 'undefined') return 'dark';
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  };

  const applyTheme = (resolved: 'light' | 'dark') => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(resolved);
    setResolvedTheme(resolved);
  };

  const resolveAndApplyTheme = () => {
    const currentTheme = theme();
    const resolved = currentTheme === 'system' ? getSystemTheme() : currentTheme;
    applyTheme(resolved);
  };

  const cycleTheme = () => {
    const current = theme();
    const next: Theme = current === 'system' ? 'light' : current === 'light' ? 'dark' : 'system';
    setTheme(next);
    localStorage.setItem(STORAGE_KEY, next);
  };

  onMount(() => {
    const stored = localStorage.getItem(STORAGE_KEY) as Theme | null;
    if (stored && ['light', 'dark', 'system'].includes(stored)) {
      setTheme(stored);
    }

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemChange = () => {
      if (theme() === 'system') {
        resolveAndApplyTheme();
      }
    };

    mediaQuery.addEventListener('change', handleSystemChange);
    onCleanup(() => mediaQuery.removeEventListener('change', handleSystemChange));
  });

  createEffect(() => {
    resolveAndApplyTheme();
  });

  const SunIcon = () => (
    <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
      />
    </svg>
  );

  const MoonIcon = () => (
    <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
      />
    </svg>
  );

  const MonitorIcon = () => (
    <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
      />
    </svg>
  );

  const getIcon = () => {
    const currentTheme = theme();
    if (currentTheme === 'light') return <SunIcon />;
    if (currentTheme === 'dark') return <MoonIcon />;
    return <MonitorIcon />;
  };

  const getTitle = () => {
    const currentTheme = theme();
    const resolved = resolvedTheme();
    if (currentTheme === 'system') {
      return `System theme (${resolved})`;
    }
    return `${currentTheme.charAt(0).toUpperCase() + currentTheme.slice(1)} theme`;
  };

  return (
    <button
      onClick={cycleTheme}
      class="theme-switcher rounded-lg border border-slate-700 p-2 text-slate-400 transition-all hover:border-slate-600 hover:bg-slate-800/50 hover:text-white"
      title={getTitle()}
      aria-label={getTitle()}
    >
      <span class="theme-icon">{getIcon()}</span>
    </button>
  );
};

export default ThemeSwitcher;
