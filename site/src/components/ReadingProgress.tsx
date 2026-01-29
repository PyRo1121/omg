import { createSignal, onMount, onCleanup } from "solid-js";

interface ReadingProgressProps {
  /** Optional CSS selector for the element to track (defaults to document) */
  targetSelector?: string;
  /** Height of the progress bar in pixels (default: 2) */
  height?: number;
}

export function ReadingProgress(props: ReadingProgressProps) {
  const [progress, setProgress] = createSignal(0);

  const calculateProgress = () => {
    let scrollTop: number;
    let scrollHeight: number;

    if (props.targetSelector) {
      const target = document.querySelector(props.targetSelector);
      if (target) {
        const rect = target.getBoundingClientRect();
        const windowHeight = window.innerHeight;
        const targetTop = window.scrollY + rect.top;
        const targetHeight = rect.height;

        const scrolled = window.scrollY - targetTop + windowHeight;
        const total = targetHeight + windowHeight;

        setProgress(Math.min(100, Math.max(0, (scrolled / total) * 100)));
        return;
      }
    }

    scrollTop = window.scrollY || document.documentElement.scrollTop;
    scrollHeight =
      document.documentElement.scrollHeight -
      document.documentElement.clientHeight;

    if (scrollHeight > 0) {
      setProgress(Math.min(100, Math.max(0, (scrollTop / scrollHeight) * 100)));
    }
  };

  onMount(() => {
    calculateProgress();
    window.addEventListener("scroll", calculateProgress, { passive: true });
    window.addEventListener("resize", calculateProgress, { passive: true });
  });

  onCleanup(() => {
    window.removeEventListener("scroll", calculateProgress);
    window.removeEventListener("resize", calculateProgress);
  });

  const barHeight = () => props.height ?? 2;

  return (
    <div
      class="fixed left-0 right-0 top-0 z-[60] overflow-hidden"
      style={{ height: `${barHeight()}px` }}
      role="progressbar"
      aria-valuenow={Math.round(progress())}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label="Reading progress"
    >
      <div
        class="h-full bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500 transition-[width] duration-150 ease-out"
        style={{ width: `${progress()}%` }}
      />
    </div>
  );
}

export default ReadingProgress;
