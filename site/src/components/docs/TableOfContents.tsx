import { For, createSignal, onMount, onCleanup, createEffect } from "solid-js";

export interface TocItem {
  id: string;
  title: string;
  depth: number;
}

interface TableOfContentsProps {
  items: TocItem[];
}

export function TableOfContents(props: TableOfContentsProps) {
  const [activeId, setActiveId] = createSignal<string>();

  onMount(() => {
    const headings = props.items
      .map((item) => document.getElementById(item.id))
      .filter(Boolean) as HTMLElement[];

    if (headings.length === 0) return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
          }
        }
      },
      {
        rootMargin: "-80px 0px -80% 0px",
        threshold: 0,
      }
    );

    headings.forEach((heading) => observer.observe(heading));

    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    if (!activeId() && props.items.length > 0) {
      setActiveId(props.items[0].id);
    }
  });

  const scrollToHeading = (id: string) => {
    const element = document.getElementById(id);
    if (element) {
      const yOffset = -100;
      const y =
        element.getBoundingClientRect().top + window.pageYOffset + yOffset;
      window.scrollTo({ top: y, behavior: "smooth" });
    }
  };

  return (
    <nav class="toc-nav">
      <h4 class="mb-4 text-sm font-semibold uppercase tracking-wide text-slate-400">
        On this page
      </h4>
      <ul class="space-y-2">
        <For each={props.items}>
          {(item) => (
            <li
              style={{ "padding-left": `${(item.depth - 2) * 12}px` }}
              class="relative"
            >
              <button
                onClick={() => scrollToHeading(item.id)}
                class="w-full text-left text-sm transition-colors duration-200"
                classList={{
                  "text-indigo-400 font-medium": activeId() === item.id,
                  "text-slate-500 hover:text-slate-300":
                    activeId() !== item.id,
                }}
              >
                <span
                  class="absolute -left-3 top-1/2 h-1 w-1 -translate-y-1/2 rounded-full transition-colors"
                  classList={{
                    "bg-indigo-400": activeId() === item.id,
                    "bg-transparent": activeId() !== item.id,
                  }}
                />
                {item.title}
              </button>
            </li>
          )}
        </For>
      </ul>
    </nav>
  );
}

export default TableOfContents;
