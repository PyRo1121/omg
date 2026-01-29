import { ParentProps, Show, createSignal, createEffect } from "solid-js";
import { A, useLocation } from "@solidjs/router";
import { Menu, X, Search, ChevronLeft, ChevronRight } from "lucide-solid";
import { createShortcut } from "@solid-primitives/keyboard";
import { Sidebar } from "./Sidebar";
import { TableOfContents } from "./TableOfContents";
import { Breadcrumbs } from "./Breadcrumbs";
import { SearchDialog } from "./SearchDialog";
import Header from "../Header";
import { ReadingProgress } from "../ReadingProgress";

export interface DocFrontmatter {
  title: string;
  description?: string;
  sidebar_position?: number;
}

export interface TocItem {
  id: string;
  title: string;
  depth: number;
}

export interface PageNav {
  prev?: { title: string; link: string };
  next?: { title: string; link: string };
}

interface DocsLayoutProps extends ParentProps {
  frontmatter?: DocFrontmatter;
  toc?: TocItem[];
  nav?: PageNav;
}

export function DocsLayout(props: DocsLayoutProps) {
  const [mobileNavOpen, setMobileNavOpen] = createSignal(false);
  const [searchOpen, setSearchOpen] = createSignal(false);
  const location = useLocation();

  createShortcut(["Meta", "K"], () => setSearchOpen(true));
  createShortcut(["Control", "K"], () => setSearchOpen(true));

  createEffect(() => {
    location.pathname;
    setMobileNavOpen(false);
  });

  return (
    <div class="min-h-screen bg-[#0a0a0a]">
      <Header />
      <ReadingProgress targetSelector=".markdown-content" />

      <SearchDialog open={searchOpen()} onClose={() => setSearchOpen(false)} />

      <Show when={mobileNavOpen()}>
        <div
          class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm lg:hidden"
          onClick={() => setMobileNavOpen(false)}
        />
        <aside class="fixed inset-y-0 left-0 z-50 w-72 overflow-y-auto bg-[#0f0f0f] p-6 lg:hidden">
          <div class="mb-6 flex items-center justify-between">
            <span class="text-lg font-bold text-white">Documentation</span>
            <button
              onClick={() => setMobileNavOpen(false)}
              class="rounded-lg p-2 text-slate-400 hover:bg-white/5 hover:text-white"
            >
              <X size={20} />
            </button>
          </div>
          <Sidebar />
        </aside>
      </Show>

      <div class="mx-auto flex max-w-[1600px] pt-20">
        <aside class="sticky top-20 hidden h-[calc(100vh-5rem)] w-72 flex-shrink-0 overflow-y-auto border-r border-slate-800 px-6 py-8 lg:block">
          <div class="mb-4">
            <button
              onClick={() => setSearchOpen(true)}
              class="flex w-full items-center gap-2 rounded-lg border border-slate-700 bg-slate-800/50 px-3 py-2 text-sm text-slate-400 transition-colors hover:border-slate-600 hover:text-slate-300"
            >
              <Search size={16} />
              <span>Search docs...</span>
              <kbd class="ml-auto rounded bg-slate-700 px-1.5 py-0.5 text-xs">
                ⌘K
              </kbd>
            </button>
          </div>
          <Sidebar />
        </aside>

        <main class="min-w-0 flex-1 px-6 py-8 lg:px-12">
          <button
            onClick={() => setMobileNavOpen(true)}
            class="mb-6 flex items-center gap-2 rounded-lg border border-slate-700 px-3 py-2 text-sm text-slate-400 lg:hidden"
          >
            <Menu size={16} />
            <span>Menu</span>
          </button>

          <Breadcrumbs />

          <article class="prose prose-invert prose-indigo max-w-none">
            <Show when={props.frontmatter?.title}>
              <h1 class="mb-2 bg-gradient-to-r from-white to-slate-400 bg-clip-text text-4xl font-black text-transparent">
                {props.frontmatter!.title}
              </h1>
            </Show>
            <Show when={props.frontmatter?.description}>
              <p class="mt-2 text-lg text-slate-400">
                {props.frontmatter!.description}
              </p>
            </Show>

            <div class="markdown-content">{props.children}</div>
          </article>

          <Show when={props.nav?.prev || props.nav?.next}>
            <nav class="mt-16 flex items-center justify-between border-t border-slate-800 pt-8">
              <Show when={props.nav?.prev}>
                <A
                  href={props.nav!.prev!.link}
                  class="group flex items-center gap-2 text-slate-400 transition-colors hover:text-white"
                >
                  <ChevronLeft
                    size={20}
                    class="transition-transform group-hover:-translate-x-1"
                  />
                  <div>
                    <div class="text-xs uppercase tracking-wide">Previous</div>
                    <div class="font-medium">{props.nav!.prev!.title}</div>
                  </div>
                </A>
              </Show>
              <div class="flex-1" />
              <Show when={props.nav?.next}>
                <A
                  href={props.nav!.next!.link}
                  class="group flex items-center gap-2 text-right text-slate-400 transition-colors hover:text-white"
                >
                  <div>
                    <div class="text-xs uppercase tracking-wide">Next</div>
                    <div class="font-medium">{props.nav!.next!.title}</div>
                  </div>
                  <ChevronRight
                    size={20}
                    class="transition-transform group-hover:translate-x-1"
                  />
                </A>
              </Show>
            </nav>
          </Show>
        </main>

        <Show when={props.toc && props.toc.length > 0}>
          <aside class="sticky top-20 hidden h-[calc(100vh-5rem)] w-64 flex-shrink-0 overflow-y-auto px-6 py-8 xl:block">
            <TableOfContents items={props.toc!} />
          </aside>
        </Show>
      </div>
    </div>
  );
}

export default DocsLayout;
