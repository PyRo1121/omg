import {
  createSignal,
  For,
  ParentProps,
  createContext,
  useContext,
  Show,
  JSX,
} from "solid-js";

interface TabsContextValue {
  activeTab: () => string;
  setActiveTab: (tab: string) => void;
}

const TabsContext = createContext<TabsContextValue>();

interface TabsProps extends ParentProps {
  defaultValue?: string;
}

export function Tabs(props: TabsProps) {
  const [activeTab, setActiveTab] = createSignal(props.defaultValue || "");

  return (
    <TabsContext.Provider value={{ activeTab, setActiveTab }}>
      <div class="tabs-container my-6">{props.children}</div>
    </TabsContext.Provider>
  );
}

interface TabListProps extends ParentProps {}

export function TabList(props: TabListProps) {
  return (
    <div class="flex gap-1 border-b border-slate-700 pb-px">
      {props.children}
    </div>
  );
}

interface TabProps extends ParentProps {
  value: string;
}

export function Tab(props: TabProps) {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error("Tab must be used within Tabs");

  const isActive = () => ctx.activeTab() === props.value;

  return (
    <button
      class="relative px-4 py-2 text-sm font-medium transition-colors"
      classList={{
        "text-indigo-400": isActive(),
        "text-slate-400 hover:text-white": !isActive(),
      }}
      onClick={() => ctx.setActiveTab(props.value)}
    >
      {props.children}
      <Show when={isActive()}>
        <span class="absolute inset-x-0 -bottom-px h-0.5 bg-indigo-400" />
      </Show>
    </button>
  );
}

interface TabPanelProps extends ParentProps {
  value: string;
}

export function TabPanel(props: TabPanelProps) {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error("TabPanel must be used within Tabs");

  return (
    <Show when={ctx.activeTab() === props.value}>
      <div class="py-4">{props.children}</div>
    </Show>
  );
}
