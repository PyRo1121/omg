import { Component, createSignal, onMount, onCleanup, Show, For } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { Command } from "cmdk-solid";
import {
  Home,
  FileText,
  LayoutDashboard,
  Github,
  Terminal,
  Copy,
  ExternalLink,
  Zap,
  Package,
} from "lucide-solid";

interface CommandItem {
  id: string;
  label: string;
  icon: Component<{ class?: string }>;
  shortcut?: string[];
  action: () => void;
  group: string;
}

const CommandPalette: Component = () => {
  const [open, setOpen] = createSignal(false);
  const [search, setSearch] = createSignal("");
  const [copied, setCopied] = createSignal(false);
  const navigate = useNavigate();

  const copyInstallCommand = () => {
    navigator.clipboard.writeText("curl -fsSL https://pyro1121.com/install.sh | bash");
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
    setOpen(false);
  };

  const commands: CommandItem[] = [
    {
      id: "home",
      label: "Home",
      icon: Home,
      shortcut: ["H"],
      action: () => {
        navigate("/");
        setOpen(false);
      },
      group: "Navigation",
    },
    {
      id: "docs",
      label: "Documentation",
      icon: FileText,
      shortcut: ["G", "D"],
      action: () => {
        navigate("/docs");
        setOpen(false);
      },
      group: "Navigation",
    },
    {
      id: "dashboard",
      label: "Dashboard",
      icon: LayoutDashboard,
      shortcut: ["G", "A"],
      action: () => {
        navigate("/dashboard");
        setOpen(false);
      },
      group: "Navigation",
    },
    {
      id: "features",
      label: "Features",
      icon: Zap,
      action: () => {
        window.location.href = "/#features";
        setOpen(false);
      },
      group: "Navigation",
    },
    {
      id: "benchmarks",
      label: "Benchmarks",
      icon: Package,
      action: () => {
        window.location.href = "/#benchmarks";
        setOpen(false);
      },
      group: "Navigation",
    },
    {
      id: "github",
      label: "GitHub Repository",
      icon: Github,
      shortcut: ["G", "H"],
      action: () => {
        window.open("https://github.com/PyRo1121/omg", "_blank");
        setOpen(false);
      },
      group: "Links",
    },
    {
      id: "copy-install",
      label: copied() ? "Copied!" : "Copy Install Command",
      icon: copied() ? Terminal : Copy,
      shortcut: ["C", "I"],
      action: copyInstallCommand,
      group: "Actions",
    },
  ];

  const groupedCommands = () => {
    const groups: Record<string, CommandItem[]> = {};
    const query = search().toLowerCase();

    for (const cmd of commands) {
      if (query && !cmd.label.toLowerCase().includes(query)) continue;
      if (!groups[cmd.group]) groups[cmd.group] = [];
      groups[cmd.group].push(cmd);
    }

    return groups;
  };

  onMount(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setOpen((prev) => !prev);
      }

      if (e.key === "Escape" && open()) {
        setOpen(false);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
  });

  return (
    <Show when={open()}>
      <div
        class="fixed inset-0 z-[100] bg-black/60 backdrop-blur-sm"
        onClick={() => setOpen(false)}
      />

      <div class="fixed inset-0 z-[101] flex items-start justify-center pt-[15vh]">
        <div class="w-full max-w-xl overflow-hidden rounded-2xl border border-slate-700/50 bg-slate-900 shadow-2xl shadow-black/50">
          <Command
            class="flex flex-col"
            shouldFilter={false}
          >
            <div class="flex items-center border-b border-slate-700/50 px-4">
              <svg
                class="h-5 w-5 shrink-0 text-slate-500"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
              <Command.Input
                value={search()}
                onValueChange={setSearch}
                placeholder="Search commands..."
                class="w-full bg-transparent px-4 py-4 text-white placeholder-slate-500 outline-none"
              />
              <kbd class="hidden rounded border border-slate-600 bg-slate-800 px-2 py-1 text-xs text-slate-400 sm:inline">
                ESC
              </kbd>
            </div>

            <Command.List class="max-h-80 overflow-y-auto overscroll-contain p-2">
              <Command.Empty class="py-6 text-center text-sm text-slate-500">
                No commands found.
              </Command.Empty>

              <For each={Object.entries(groupedCommands())}>
                {([group, items]) => (
                  <Command.Group class="mb-2">
                    <div class="px-2 py-1.5 text-xs font-semibold uppercase tracking-wider text-slate-500">
                      {group}
                    </div>
                    <For each={items}>
                      {(cmd) => (
                        <Command.Item
                          value={cmd.id}
                          onSelect={cmd.action}
                          class="group flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2.5 text-slate-300 outline-none transition-colors hover:bg-slate-800 hover:text-white data-[selected=true]:bg-slate-800 data-[selected=true]:text-white"
                        >
                          <cmd.icon class="h-4 w-4 text-slate-500 transition-colors group-hover:text-indigo-400 group-data-[selected=true]:text-indigo-400" />
                          <span class="flex-1">{cmd.label}</span>
                          <Show when={cmd.shortcut}>
                            <div class="flex items-center gap-1">
                              <For each={cmd.shortcut}>
                                {(key) => (
                                  <kbd class="rounded border border-slate-700 bg-slate-800/50 px-1.5 py-0.5 text-[10px] font-medium text-slate-500">
                                    {key}
                                  </kbd>
                                )}
                              </For>
                            </div>
                          </Show>
                          <Show when={cmd.id === "github"}>
                            <ExternalLink class="h-3 w-3 text-slate-600" />
                          </Show>
                        </Command.Item>
                      )}
                    </For>
                  </Command.Group>
                )}
              </For>
            </Command.List>

            <div class="flex items-center justify-between border-t border-slate-700/50 px-4 py-2.5 text-xs text-slate-500">
              <div class="flex items-center gap-4">
                <span class="flex items-center gap-1">
                  <kbd class="rounded border border-slate-700 bg-slate-800 px-1 py-0.5 text-[10px]">↑</kbd>
                  <kbd class="rounded border border-slate-700 bg-slate-800 px-1 py-0.5 text-[10px]">↓</kbd>
                  <span class="ml-1">Navigate</span>
                </span>
                <span class="flex items-center gap-1">
                  <kbd class="rounded border border-slate-700 bg-slate-800 px-1.5 py-0.5 text-[10px]">↵</kbd>
                  <span class="ml-1">Select</span>
                </span>
              </div>
              <span class="flex items-center gap-1.5">
                <span class="inline-flex h-4 w-4 items-center justify-center rounded bg-indigo-500/20 text-[10px] font-bold text-indigo-400">
                  ⌘
                </span>
                <span>K to toggle</span>
              </span>
            </div>
          </Command>
        </div>
      </div>
    </Show>
  );
};

export default CommandPalette;
