import { For, Show, createMemo } from "solid-js";
import { A, useLocation } from "@solidjs/router";
import { ChevronRight, Home } from "lucide-solid";

interface BreadcrumbItem {
  label: string;
  href: string;
}

export function Breadcrumbs() {
  const location = useLocation();

  const breadcrumbs = createMemo<BreadcrumbItem[]>(() => {
    const path = location.pathname;
    if (!path.startsWith("/docs")) return [];

    const segments = path.split("/").filter(Boolean);
    const items: BreadcrumbItem[] = [];

    let currentPath = "";
    for (const segment of segments) {
      currentPath += `/${segment}`;
      const label = segment
        .split("-")
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(" ");
      items.push({ label, href: currentPath });
    }

    return items;
  });

  return (
    <Show when={breadcrumbs().length > 1}>
      <nav class="mb-6 flex items-center gap-1 text-sm">
        <A
          href="/"
          class="text-slate-500 transition-colors hover:text-slate-300"
        >
          <Home size={14} />
        </A>

        <For each={breadcrumbs()}>
          {(item, index) => (
            <>
              <ChevronRight size={14} class="text-slate-600" />
              <Show
                when={index() < breadcrumbs().length - 1}
                fallback={
                  <span class="font-medium text-slate-300">{item.label}</span>
                }
              >
                <A
                  href={item.href}
                  class="text-slate-500 transition-colors hover:text-slate-300"
                >
                  {item.label}
                </A>
              </Show>
            </>
          )}
        </For>
      </nav>
    </Show>
  );
}

export default Breadcrumbs;
