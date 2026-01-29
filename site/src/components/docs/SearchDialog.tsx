import { createSignal, For, Show, onMount, createEffect } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { Dialog } from "@kobalte/core";
import { Search, FileText, ArrowRight, Command } from "lucide-solid";

interface SearchResult {
  url: string;
  title: string;
  excerpt: string;
}

interface SearchDialogProps {
  open: boolean;
  onClose: () => void;
}

export function SearchDialog(props: SearchDialogProps) {
  const [query, setQuery] = createSignal("");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  const navigate = useNavigate();

  let pagefind: any = null;
  let inputRef: HTMLInputElement | undefined;

  onMount(async () => {
    try {
      pagefind = await import(
        /* @vite-ignore */ "/pagefind/pagefind.js"
      );
    } catch {
      console.warn("Pagefind not available - search disabled");
    }
  });

  createEffect(() => {
    if (props.open && inputRef) {
      setTimeout(() => inputRef?.focus(), 50);
    }
  });

  createEffect(() => {
    setSelectedIndex(0);
  });

  const handleSearch = async (term: string) => {
    setQuery(term);

    if (!term.trim() || !pagefind) {
      setResults([]);
      return;
    }

    setLoading(true);

    try {
      const search = await pagefind.search(term);
      const data = await Promise.all(
        search.results.slice(0, 8).map(async (r: any) => {
          const d = await r.data();
          return {
            url: d.url,
            title: d.meta?.title || d.url,
            excerpt: d.excerpt,
          };
        })
      );
      setResults(data);
    } catch (err) {
      console.error("Search failed:", err);
      setResults([]);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const len = results().length;
    if (len === 0) return;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((i) => (i + 1) % len);
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((i) => (i - 1 + len) % len);
        break;
      case "Enter":
        e.preventDefault();
        const result = results()[selectedIndex()];
        if (result) {
          navigate(result.url);
          props.onClose();
        }
        break;
    }
  };

  const handleResultClick = (url: string) => {
    navigate(url);
    props.onClose();
    setQuery("");
    setResults([]);
  };

  return (
    <Dialog.Root open={props.open} onOpenChange={(open) => !open && props.onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm" />
        <div class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]">
          <Dialog.Content class="w-full max-w-xl overflow-hidden rounded-xl border border-slate-700 bg-[#0f0f0f] shadow-2xl">
            <div class="flex items-center gap-3 border-b border-slate-800 px-4 py-3">
              <Search size={20} class="text-slate-500" />
              <input
                ref={inputRef}
                type="text"
                value={query()}
                onInput={(e) => handleSearch(e.currentTarget.value)}
                onKeyDown={handleKeyDown}
                placeholder="Search documentation..."
                class="flex-1 bg-transparent text-white placeholder:text-slate-500 focus:outline-none"
              />
              <kbd class="rounded bg-slate-800 px-2 py-1 text-xs text-slate-400">
                ESC
              </kbd>
            </div>

            <Show when={loading()}>
              <div class="flex items-center justify-center py-8">
                <div class="h-6 w-6 animate-spin rounded-full border-2 border-indigo-500 border-t-transparent" />
              </div>
            </Show>

            <Show when={!loading() && results().length > 0}>
              <div class="max-h-96 overflow-y-auto p-2">
                <For each={results()}>
                  {(result, index) => (
                    <button
                      onClick={() => handleResultClick(result.url)}
                      onMouseEnter={() => setSelectedIndex(index())}
                      class="group flex w-full items-start gap-3 rounded-lg px-3 py-3 text-left transition-colors"
                      classList={{
                        "bg-indigo-500/20": selectedIndex() === index(),
                        "hover:bg-white/5": selectedIndex() !== index(),
                      }}
                    >
                      <FileText
                        size={18}
                        class="mt-0.5 flex-shrink-0 text-slate-500"
                      />
                      <div class="min-w-0 flex-1">
                        <div class="font-medium text-white">{result.title}</div>
                        <div
                          class="mt-1 line-clamp-2 text-sm text-slate-400"
                          innerHTML={result.excerpt}
                        />
                      </div>
                      <ArrowRight
                        size={16}
                        class="mt-1 flex-shrink-0 text-slate-600 transition-transform group-hover:translate-x-1"
                        classList={{
                          "text-indigo-400": selectedIndex() === index(),
                        }}
                      />
                    </button>
                  )}
                </For>
              </div>
            </Show>

            <Show when={!loading() && query() && results().length === 0}>
              <div class="py-12 text-center">
                <Search size={40} class="mx-auto mb-3 text-slate-600" />
                <p class="text-slate-400">No results found for "{query()}"</p>
                <p class="mt-1 text-sm text-slate-500">
                  Try searching for something else
                </p>
              </div>
            </Show>

            <Show when={!query()}>
              <div class="p-4 text-center text-sm text-slate-500">
                <p>Start typing to search the documentation</p>
                <div class="mt-4 flex items-center justify-center gap-4">
                  <span class="flex items-center gap-1">
                    <kbd class="rounded bg-slate-800 px-1.5 py-0.5 text-xs">
                      <Command size={10} />
                    </kbd>
                    <kbd class="rounded bg-slate-800 px-1.5 py-0.5 text-xs">
                      K
                    </kbd>
                    <span class="ml-1">to open</span>
                  </span>
                  <span class="flex items-center gap-1">
                    <kbd class="rounded bg-slate-800 px-1.5 py-0.5 text-xs">
                      ↑↓
                    </kbd>
                    <span class="ml-1">to navigate</span>
                  </span>
                  <span class="flex items-center gap-1">
                    <kbd class="rounded bg-slate-800 px-1.5 py-0.5 text-xs">
                      ↵
                    </kbd>
                    <span class="ml-1">to select</span>
                  </span>
                </div>
              </div>
            </Show>
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default SearchDialog;
