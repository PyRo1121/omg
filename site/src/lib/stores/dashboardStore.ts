import { createStore } from 'solid-js/store';
import { createEffect } from 'solid-js';
import { debounce } from '@solid-primitives/scheduled';
import type { AdminTab, SavedView, DateRange } from '~/types';

export interface DashboardState {
  navigation: {
    activeTab: AdminTab;
    tabHistory: AdminTab[];
  };
  filters: {
    dateRange: DateRange;
    segment: string;
    compareEnabled: boolean;
  };
  views: {
    saved: SavedView[];
    showSaveModal: boolean;
    newViewName: string;
  };
  ui: {
    exportMenuOpen: boolean;
    isExporting: boolean;
  };
  crm: {
    page: number;
    search: string;
    viewMode: 'cards' | 'table';
    selectedUserId: string | null;
  };
}

const STORAGE_KEY = 'omg-dashboard-state';
const STORAGE_VERSION = 1;

function getInitialState(): DashboardState {
  if (typeof window === 'undefined') {
    return createDefaultState();
  }

  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return createDefaultState();

    const parsed = JSON.parse(stored);
    if (parsed.version !== STORAGE_VERSION) {
      console.log('[DashboardStore] Version mismatch, using defaults');
      return createDefaultState();
    }

    return {
      ...createDefaultState(),
      navigation: {
        ...createDefaultState().navigation,
        activeTab: parsed.state.navigation?.activeTab || 'overview',
      },
      filters: {
        ...createDefaultState().filters,
        ...(parsed.state.filters || {}),
      },
      views: {
        ...createDefaultState().views,
        saved: parsed.state.views?.saved || [],
      },
      crm: {
        ...createDefaultState().crm,
        viewMode: parsed.state.crm?.viewMode || 'table',
      },
    };
  } catch (error) {
    console.error('[DashboardStore] Failed to restore state:', error);
    return createDefaultState();
  }
}

function createDefaultState(): DashboardState {
  return {
    navigation: {
      activeTab: 'overview',
      tabHistory: [],
    },
    filters: {
      dateRange: '30d',
      segment: 'all',
      compareEnabled: false,
    },
    views: {
      saved: [],
      showSaveModal: false,
      newViewName: '',
    },
    ui: {
      exportMenuOpen: false,
      isExporting: false,
    },
    crm: {
      page: 1,
      search: '',
      viewMode: 'table',
      selectedUserId: null,
    },
  };
}

export function createDashboardStore() {
  const [state, setState] = createStore<DashboardState>(getInitialState());

  const persistableState = () => ({
    version: STORAGE_VERSION,
    state: {
      navigation: {
        activeTab: state.navigation.activeTab,
      },
      filters: state.filters,
      views: {
        saved: state.views.saved,
      },
      crm: {
        viewMode: state.crm.viewMode,
      },
    },
  });

  if (typeof window !== 'undefined') {
    const debouncedPersist = debounce((state: ReturnType<typeof persistableState>) => {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
      } catch (error) {
        console.error('[DashboardStore] Failed to persist state:', error);
      }
    }, 500);

    createEffect(() => {
      debouncedPersist(persistableState());
    });
  }

  const actions = {
    setTab(tab: AdminTab) {
      setState('navigation', (prev) => ({
        activeTab: tab,
        tabHistory: [...prev.tabHistory.slice(-9), prev.activeTab],
      }));
    },

    goToPreviousTab() {
      const history = state.navigation.tabHistory;
      if (history.length === 0) return;
      const previousTab = history[history.length - 1];
      setState('navigation', {
        activeTab: previousTab,
        tabHistory: history.slice(0, -1),
      });
    },

    updateFilters(filters: Partial<DashboardState['filters']>) {
      setState('filters', (prev) => ({ ...prev, ...filters }));
    },

    setDateRange(dateRange: DateRange) {
      setState('filters', 'dateRange', dateRange);
    },

    setSegment(segment: string) {
      setState('filters', 'segment', segment);
    },

    toggleCompare() {
      setState('filters', 'compareEnabled', (prev) => !prev);
    },

    saveView() {
      const viewName = state.views.newViewName.trim();
      if (!viewName) return;

      const newView: SavedView = {
        id: `view-${Date.now()}`,
        name: viewName,
        tab: state.navigation.activeTab,
        dateRange: state.filters.dateRange,
        segment: state.filters.segment,
        compareEnabled: state.filters.compareEnabled,
      };

      setState('views', 'saved', (prev) => [...prev, newView]);
      setState('views', {
        showSaveModal: false,
        newViewName: '',
      });
    },

    loadView(view: SavedView) {
      setState('navigation', 'activeTab', view.tab);
      setState('filters', {
        dateRange: view.dateRange,
        segment: view.segment,
        compareEnabled: view.compareEnabled,
      });
    },

    deleteView(viewId: string) {
      setState('views', 'saved', (prev) => prev.filter((v) => v.id !== viewId));
    },

    showSaveViewModal() {
      setState('views', 'showSaveModal', true);
    },

    hideSaveViewModal() {
      setState('views', {
        showSaveModal: false,
        newViewName: '',
      });
    },

    setNewViewName(name: string) {
      setState('views', 'newViewName', name);
    },

    toggleExportMenu() {
      setState('ui', 'exportMenuOpen', (prev) => !prev);
    },

    closeExportMenu() {
      setState('ui', 'exportMenuOpen', false);
    },

    setExporting(isExporting: boolean) {
      setState('ui', 'isExporting', isExporting);
    },

    setCRMPage(page: number) {
      setState('crm', 'page', page);
    },

    setCRMSearch(search: string) {
      setState('crm', {
        search,
        page: 1,
      });
    },

    setCRMViewMode(viewMode: 'cards' | 'table') {
      setState('crm', 'viewMode', viewMode);
    },

    setSelectedUserId(userId: string | null) {
      setState('crm', 'selectedUserId', userId);
    },

    resetCRM() {
      setState('crm', createDefaultState().crm);
    },
  };

  return [state, actions] as const;
}

export type DashboardStore = ReturnType<typeof createDashboardStore>;
export type DashboardActions = DashboardStore[1];
