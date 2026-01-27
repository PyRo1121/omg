import { Component, ParentComponent, Show, For, createSignal, JSX } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface DashboardGridProps {
  columns?: 1 | 2 | 3 | 4 | 6 | 12;
  gap?: 'sm' | 'md' | 'lg' | 'xl';
  class?: string;
}

const gapClasses = {
  sm: 'gap-3',
  md: 'gap-4',
  lg: 'gap-6',
  xl: 'gap-8',
};

const columnClasses = {
  1: 'grid-cols-1',
  2: 'grid-cols-1 md:grid-cols-2',
  3: 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3',
  4: 'grid-cols-1 md:grid-cols-2 lg:grid-cols-4',
  6: 'grid-cols-2 md:grid-cols-3 lg:grid-cols-6',
  12: 'grid-cols-4 md:grid-cols-6 lg:grid-cols-12',
};

export const DashboardGrid: ParentComponent<DashboardGridProps> = (props) => {
  return (
    <div class={cn(
      'grid',
      columnClasses[props.columns || 4],
      gapClasses[props.gap || 'lg'],
      props.class
    )}>
      {props.children}
    </div>
  );
};

interface GridItemProps {
  span?: 1 | 2 | 3 | 4 | 6 | 12 | 'full';
  rowSpan?: 1 | 2 | 3;
  class?: string;
}

const spanClasses = {
  1: 'col-span-1',
  2: 'col-span-1 md:col-span-2',
  3: 'col-span-1 md:col-span-2 lg:col-span-3',
  4: 'col-span-1 md:col-span-2 lg:col-span-4',
  6: 'col-span-2 md:col-span-3 lg:col-span-6',
  12: 'col-span-full',
  full: 'col-span-full',
};

const rowSpanClasses = {
  1: 'row-span-1',
  2: 'row-span-2',
  3: 'row-span-3',
};

export const GridItem: ParentComponent<GridItemProps> = (props) => {
  return (
    <div class={cn(
      spanClasses[props.span || 1],
      props.rowSpan && rowSpanClasses[props.rowSpan],
      props.class
    )}>
      {props.children}
    </div>
  );
};

interface SectionProps {
  title?: string;
  subtitle?: string;
  action?: JSX.Element;
  variant?: 'default' | 'card' | 'transparent';
  collapsible?: boolean;
  defaultExpanded?: boolean;
  class?: string;
}

export const Section: ParentComponent<SectionProps> = (props) => {
  const [expanded, setExpanded] = createSignal(props.defaultExpanded !== false);

  const variantClasses = {
    default: 'bg-void-850 border border-white/5 rounded-3xl p-8',
    card: 'bg-void-800 border border-white/5 rounded-3xl p-6 shadow-lg',
    transparent: '',
  };

  return (
    <section class={cn(variantClasses[props.variant || 'default'], props.class)}>
      <Show when={props.title}>
        <div class="flex items-start justify-between mb-6">
          <div>
            <h3 class="text-xl font-black tracking-tight text-white">{props.title}</h3>
            <Show when={props.subtitle}>
              <p class="mt-1 text-sm text-nebula-500 font-medium">{props.subtitle}</p>
            </Show>
          </div>
          <div class="flex items-center gap-3">
            {props.action}
            <Show when={props.collapsible}>
              <button
                onClick={() => setExpanded(!expanded())}
                class="p-2 rounded-lg text-nebula-500 hover:text-white hover:bg-white/5 transition-colors"
              >
                <svg
                  width="16"
                  height="16"
                  viewBox="0 0 16 16"
                  fill="none"
                  class={cn('transition-transform', expanded() ? 'rotate-180' : '')}
                >
                  <path
                    d="M4 6L8 10L12 6"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
            </Show>
          </div>
        </div>
      </Show>
      <Show when={!props.collapsible || expanded()}>
        <div class={cn(props.collapsible && 'animate-slide-in-bottom')}>
          {props.children}
        </div>
      </Show>
    </section>
  );
};

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  breadcrumbs?: Array<{ label: string; href?: string }>;
  actions?: JSX.Element;
  class?: string;
}

export const PageHeader: Component<PageHeaderProps> = (props) => {
  return (
    <header class={cn('mb-8', props.class)}>
      <Show when={props.breadcrumbs && props.breadcrumbs.length > 0}>
        <nav class="mb-4 flex items-center gap-2 text-sm">
          <For each={props.breadcrumbs}>
            {(crumb, index) => (
              <>
                <Show when={crumb.href} fallback={
                  <span class="text-nebula-400">{crumb.label}</span>
                }>
                  <a href={crumb.href} class="text-nebula-500 hover:text-white transition-colors">
                    {crumb.label}
                  </a>
                </Show>
                <Show when={index() < props.breadcrumbs!.length - 1}>
                  <span class="text-nebula-700">/</span>
                </Show>
              </>
            )}
          </For>
        </nav>
      </Show>
      <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h1 class="text-4xl font-black tracking-tight text-white font-display">
            {props.title}
          </h1>
          <Show when={props.subtitle}>
            <p class="mt-2 text-base text-nebula-400 font-medium">{props.subtitle}</p>
          </Show>
        </div>
        <Show when={props.actions}>
          <div class="flex items-center gap-3">{props.actions}</div>
        </Show>
      </div>
    </header>
  );
};

interface TabNavigationProps {
  tabs: Array<{
    id: string;
    label: string;
    icon?: JSX.Element;
    badge?: string | number;
  }>;
  activeTab: string;
  onChange: (id: string) => void;
  variant?: 'pills' | 'underline' | 'cards';
  class?: string;
}

export const TabNavigation: Component<TabNavigationProps> = (props) => {
  const variant = () => props.variant || 'pills';

  const baseClasses = {
    pills: 'flex items-center gap-2 rounded-2xl border border-white/5 bg-white/[0.02] p-1.5',
    underline: 'flex items-center gap-6 border-b border-white/5',
    cards: 'flex items-center gap-3',
  };

  const tabClasses = {
    pills: (active: boolean) => cn(
      'flex items-center gap-2 rounded-xl px-5 py-2.5 font-bold transition-all',
      active
        ? 'bg-white text-black shadow-lg scale-[1.02]'
        : 'text-nebula-400 hover:bg-white/5 hover:text-white'
    ),
    underline: (active: boolean) => cn(
      'flex items-center gap-2 py-3 font-bold transition-all border-b-2 -mb-px',
      active
        ? 'border-indigo-500 text-white'
        : 'border-transparent text-nebula-400 hover:text-white hover:border-white/20'
    ),
    cards: (active: boolean) => cn(
      'flex items-center gap-2 px-4 py-3 rounded-xl font-bold transition-all border',
      active
        ? 'bg-indigo-500/10 border-indigo-500/30 text-indigo-400'
        : 'bg-void-850 border-white/5 text-nebula-400 hover:border-white/10 hover:text-white'
    ),
  };

  return (
    <nav class={cn(baseClasses[variant()], props.class)}>
      <For each={props.tabs}>
        {(tab) => (
          <button
            onClick={() => props.onChange(tab.id)}
            class={tabClasses[variant()](props.activeTab === tab.id)}
          >
            {tab.icon}
            <span>{tab.label}</span>
            <Show when={tab.badge !== undefined}>
              <span class={cn(
                'px-1.5 py-0.5 rounded-full text-2xs font-black',
                props.activeTab === tab.id
                  ? 'bg-black/20 text-inherit'
                  : 'bg-white/10 text-nebula-300'
              )}>
                {tab.badge}
              </span>
            </Show>
          </button>
        )}
      </For>
    </nav>
  );
};

interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  subtitle?: string;
  width?: 'sm' | 'md' | 'lg' | 'xl' | '2xl';
  class?: string;
}

const drawerWidths = {
  sm: 'max-w-sm',
  md: 'max-w-md',
  lg: 'max-w-lg',
  xl: 'max-w-xl',
  '2xl': 'max-w-2xl',
};

export const Drawer: ParentComponent<DrawerProps> = (props) => {
  return (
    <Show when={props.open}>
      <div class="fixed inset-0 z-50 flex justify-end">
        <div
          class="absolute inset-0 bg-black/60 backdrop-blur-sm animate-fade-in"
          onClick={props.onClose}
        />
        <div
          class={cn(
            'relative w-full overflow-y-auto border-l border-white/10 bg-void-950 shadow-2xl',
            'animate-slide-in-right',
            drawerWidths[props.width || 'lg'],
            props.class
          )}
        >
          <div class="sticky top-0 z-10 border-b border-white/5 bg-void-950/95 p-6 backdrop-blur-xl">
            <div class="flex items-start justify-between">
              <div>
                <Show when={props.title}>
                  <h2 class="text-2xl font-black tracking-tight text-white">{props.title}</h2>
                </Show>
                <Show when={props.subtitle}>
                  <p class="mt-1 text-sm text-nebula-500">{props.subtitle}</p>
                </Show>
              </div>
              <button
                onClick={props.onClose}
                class="p-2 rounded-xl text-nebula-400 hover:bg-white/5 hover:text-white transition-colors"
              >
                <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
                  <path
                    d="M15 5L5 15M5 5L15 15"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
            </div>
          </div>
          <div class="p-6">{props.children}</div>
        </div>
      </div>
    </Show>
  );
};

interface EmptyStateProps {
  icon?: JSX.Element;
  title: string;
  description?: string;
  action?: JSX.Element;
  class?: string;
}

export const EmptyState: Component<EmptyStateProps> = (props) => {
  return (
    <div class={cn(
      'flex flex-col items-center justify-center py-16 text-center',
      props.class
    )}>
      <Show when={props.icon}>
        <div class="mb-4 text-nebula-600">{props.icon}</div>
      </Show>
      <h3 class="text-lg font-bold text-white">{props.title}</h3>
      <Show when={props.description}>
        <p class="mt-2 text-sm text-nebula-500 max-w-sm">{props.description}</p>
      </Show>
      <Show when={props.action}>
        <div class="mt-6">{props.action}</div>
      </Show>
    </div>
  );
};

interface LoadingStateProps {
  variant?: 'spinner' | 'skeleton' | 'pulse';
  count?: number;
  class?: string;
}

export const LoadingState: Component<LoadingStateProps> = (props) => {
  const variant = () => props.variant || 'skeleton';
  const count = () => props.count || 3;

  if (variant() === 'spinner') {
    return (
      <div class={cn('flex items-center justify-center py-16', props.class)}>
        <div class="relative w-12 h-12">
          <div class="absolute inset-0 rounded-full border-2 border-void-700" />
          <div class="absolute inset-0 rounded-full border-2 border-indigo-500 border-t-transparent animate-spin" />
        </div>
      </div>
    );
  }

  if (variant() === 'pulse') {
    return (
      <div class={cn('space-y-4', props.class)}>
        <For each={Array(count()).fill(0)}>
          {() => (
            <div class="h-24 rounded-2xl bg-white/5 animate-pulse" />
          )}
        </For>
      </div>
    );
  }

  return (
    <div class={cn('space-y-4', props.class)}>
      <For each={Array(count()).fill(0)}>
        {() => (
          <div class="rounded-2xl border border-white/5 bg-void-850 p-6">
            <div class="flex items-start gap-4">
              <div class="w-12 h-12 rounded-xl bg-white/5 animate-pulse" />
              <div class="flex-1 space-y-2">
                <div class="h-4 w-1/3 rounded bg-white/5 animate-pulse" />
                <div class="h-3 w-2/3 rounded bg-white/5 animate-pulse" />
              </div>
              <div class="h-6 w-16 rounded-full bg-white/5 animate-pulse" />
            </div>
          </div>
        )}
      </For>
    </div>
  );
};

export default {
  DashboardGrid,
  GridItem,
  Section,
  PageHeader,
  TabNavigation,
  Drawer,
  EmptyState,
  LoadingState,
};
