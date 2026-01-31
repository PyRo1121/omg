import { Component, JSX, Show, createSignal } from 'solid-js';
import { Portal } from 'solid-js/web';

interface TooltipProps {
  content: string | JSX.Element;
  children: JSX.Element;
  position?: 'top' | 'bottom' | 'left' | 'right';
  delay?: number;
}

export const Tooltip: Component<TooltipProps> = (props) => {
  const [isVisible, setIsVisible] = createSignal(false);
  const [tooltipPos, setTooltipPos] = createSignal({ x: 0, y: 0 });
  let timeoutId: number | undefined;
  let triggerRef: HTMLDivElement | undefined;

  const delay = () => props.delay || 200;

  const handleMouseEnter = (e: MouseEvent) => {
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    
    const position = props.position || 'top';
    let x = 0;
    let y = 0;

    switch (position) {
      case 'top':
        x = rect.left + rect.width / 2;
        y = rect.top - 8;
        break;
      case 'bottom':
        x = rect.left + rect.width / 2;
        y = rect.bottom + 8;
        break;
      case 'left':
        x = rect.left - 8;
        y = rect.top + rect.height / 2;
        break;
      case 'right':
        x = rect.right + 8;
        y = rect.top + rect.height / 2;
        break;
    }

    setTooltipPos({ x, y });

    timeoutId = window.setTimeout(() => {
      setIsVisible(true);
    }, delay());
  };

  const handleMouseLeave = () => {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    setIsVisible(false);
  };

  const getPositionClasses = () => {
    const position = props.position || 'top';
    switch (position) {
      case 'top':
        return '-translate-x-1/2 -translate-y-full';
      case 'bottom':
        return '-translate-x-1/2';
      case 'left':
        return '-translate-x-full -translate-y-1/2';
      case 'right':
        return '-translate-y-1/2';
      default:
        return '-translate-x-1/2 -translate-y-full';
    }
  };

  return (
    <>
      <div
        ref={triggerRef}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        class="inline-block"
      >
        {props.children}
      </div>
      
      <Show when={isVisible()}>
        <Portal>
          <div
            class={`pointer-events-none fixed z-[100] max-w-xs rounded-lg border border-white/10 bg-void-900/95 px-3 py-2 text-xs text-nebula-200 shadow-2xl backdrop-blur-sm transition-opacity duration-200 ${getPositionClasses()}`}
            style={{
              left: `${tooltipPos().x}px`,
              top: `${tooltipPos().y}px`,
            }}
          >
            {props.content}
          </div>
        </Portal>
      </Show>
    </>
  );
};
