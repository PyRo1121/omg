import { For, ParentProps, JSX, Show } from "solid-js";

interface StepProps extends ParentProps {
  title?: string;
}

export function Step(props: StepProps) {
  return (
    <div class="step-content">
      <Show when={props.title}>
        <h4 class="step-title mb-2 font-semibold text-white">{props.title}</h4>
      </Show>
      <div class="step-body text-slate-300">{props.children}</div>
    </div>
  );
}

interface StepsProps extends ParentProps {}

export function Steps(props: StepsProps) {
  const children = () => {
    const c = props.children;
    if (Array.isArray(c)) return c;
    return [c];
  };

  return (
    <div class="steps-container my-8 space-y-6">
      <For each={children()}>
        {(child, index) => (
          <div class="step relative flex gap-4">
            <div class="step-indicator flex-shrink-0">
              <div class="flex h-8 w-8 items-center justify-center rounded-full bg-indigo-500 text-sm font-bold text-white">
                {index() + 1}
              </div>
              <Show when={index() < children().length - 1}>
                <div class="absolute left-4 top-10 h-[calc(100%-24px)] w-0.5 -translate-x-1/2 bg-slate-700" />
              </Show>
            </div>
            <div class="flex-1 pb-2">{child}</div>
          </div>
        )}
      </For>
    </div>
  );
}
