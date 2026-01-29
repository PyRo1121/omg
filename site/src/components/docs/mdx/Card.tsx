import { ParentProps, Show, JSX } from "solid-js";
import { A } from "@solidjs/router";
import { ArrowRight } from "lucide-solid";

interface CardProps extends ParentProps {
  title: string;
  href?: string;
  icon?: JSX.Element;
}

export function Card(props: CardProps) {
  const content = (
    <div class="group h-full rounded-xl border border-slate-700 bg-slate-800/50 p-6 transition-all hover:border-indigo-500/50 hover:bg-slate-800">
      <Show when={props.icon}>
        <div class="mb-4 text-indigo-400">{props.icon}</div>
      </Show>
      <h3 class="mb-2 flex items-center gap-2 font-semibold text-white">
        {props.title}
        <Show when={props.href}>
          <ArrowRight
            size={16}
            class="opacity-0 transition-all group-hover:translate-x-1 group-hover:opacity-100"
          />
        </Show>
      </h3>
      <div class="text-sm text-slate-400">{props.children}</div>
    </div>
  );

  return (
    <Show when={props.href} fallback={content}>
      <A href={props.href!} class="block no-underline">
        {content}
      </A>
    </Show>
  );
}

interface CardGridProps extends ParentProps {
  cols?: 2 | 3 | 4;
}

export function CardGrid(props: CardGridProps) {
  const cols = () => props.cols || 2;

  return (
    <div
      class="my-6 grid gap-4"
      classList={{
        "grid-cols-1 md:grid-cols-2": cols() === 2,
        "grid-cols-1 md:grid-cols-2 lg:grid-cols-3": cols() === 3,
        "grid-cols-1 md:grid-cols-2 lg:grid-cols-4": cols() === 4,
      }}
    >
      {props.children}
    </div>
  );
}
