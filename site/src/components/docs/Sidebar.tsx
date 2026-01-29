import { For, Show, createSignal, createMemo } from "solid-js";
import { A, useLocation } from "@solidjs/router";
import { ChevronDown, ChevronRight } from "lucide-solid";

export interface SidebarItem {
  text: string;
  link?: string;
  collapsed?: boolean;
  items?: SidebarItem[];
}

const sidebarConfig: SidebarItem[] = [
  {
    text: "Quick Start",
    link: "/docs/quickstart",
  },
  {
    text: "Guides",
    collapsed: false,
    items: [
      { text: "Package Management", link: "/docs/guides/packages" },
      { text: "Runtime Management", link: "/docs/guides/runtimes" },
      { text: "Shell Integration", link: "/docs/guides/shell-integration" },
      { text: "Task Runner", link: "/docs/guides/task-runner" },
      { text: "Team Workflows", link: "/docs/guides/team" },
      { text: "Security", link: "/docs/guides/security" },
      { text: "Containers", link: "/docs/guides/containers" },
    ],
  },
  {
    text: "Migration",
    collapsed: true,
    items: [
      { text: "From yay/pacman", link: "/docs/migration/from-yay" },
      { text: "From nvm", link: "/docs/migration/from-nvm" },
      { text: "From pyenv", link: "/docs/migration/from-pyenv" },
    ],
  },
  {
    text: "Reference",
    collapsed: true,
    items: [
      { text: "CLI Reference", link: "/docs/reference/cli" },
      { text: "Configuration", link: "/docs/reference/configuration" },
      { text: "API Reference", link: "/docs/reference/api" },
      { text: "Troubleshooting", link: "/docs/reference/troubleshooting" },
      { text: "FAQ", link: "/docs/reference/faq" },
    ],
  },
  {
    text: "Advanced",
    collapsed: true,
    items: [
      { text: "Architecture", link: "/docs/advanced/architecture" },
      { text: "Daemon", link: "/docs/advanced/daemon" },
      { text: "IPC Protocol", link: "/docs/advanced/ipc" },
      { text: "Performance", link: "/docs/advanced/performance" },
    ],
  },
  {
    text: "About",
    collapsed: true,
    items: [
      { text: "Introduction", link: "/docs" },
      { text: "Changelog", link: "/docs/changelog" },
    ],
  },
];

export function Sidebar() {
  return (
    <nav class="space-y-1">
      <For each={sidebarConfig}>
        {(item) => <SidebarSection item={item} />}
      </For>
    </nav>
  );
}

function SidebarSection(props: { item: SidebarItem }) {
  const location = useLocation();
  const [expanded, setExpanded] = createSignal(!props.item.collapsed);

  const isActive = (link: string) => location.pathname === link;

  const hasActiveChild = createMemo(() =>
    props.item.items?.some((child) => isActive(child.link || ""))
  );

  const shouldExpand = createMemo(() => expanded() || hasActiveChild());

  return (
    <div class="sidebar-section">
      <Show
        when={props.item.items}
        fallback={
          <A
            href={props.item.link!}
            class="block rounded-lg px-3 py-2 text-sm transition-all duration-200"
            classList={{
              "bg-indigo-500/20 text-indigo-300 font-medium": isActive(
                props.item.link!
              ),
              "text-slate-400 hover:text-white hover:bg-white/5": !isActive(
                props.item.link!
              ),
            }}
          >
            {props.item.text}
          </A>
        }
      >
        <button
          class="flex w-full items-center justify-between rounded-lg px-3 py-2 text-sm font-medium transition-colors"
          classList={{
            "text-white": shouldExpand(),
            "text-slate-400 hover:text-white": !shouldExpand(),
          }}
          onClick={() => setExpanded((prev) => !prev)}
        >
          <span>{props.item.text}</span>
          <Show
            when={shouldExpand()}
            fallback={<ChevronRight size={16} class="text-slate-500" />}
          >
            <ChevronDown size={16} class="text-slate-500" />
          </Show>
        </button>

        <Show when={shouldExpand()}>
          <div class="ml-3 mt-1 space-y-1 border-l border-slate-700 pl-3">
            <For each={props.item.items}>
              {(child) => (
                <A
                  href={child.link!}
                  class="block rounded-lg px-3 py-1.5 text-sm transition-all duration-200"
                  classList={{
                    "bg-indigo-500/20 text-indigo-300 font-medium border-l-2 border-indigo-400 -ml-[13px] pl-[22px]":
                      isActive(child.link!),
                    "text-slate-400 hover:text-white hover:bg-white/5":
                      !isActive(child.link!),
                  }}
                >
                  {child.text}
                </A>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}

export default Sidebar;
