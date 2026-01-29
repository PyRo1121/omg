import { Component, createSignal, Show, JSX } from 'solid-js';
import { Copy, Check } from 'lucide-solid';

export interface CopyButtonProps {
  /** The text to copy to clipboard */
  text: string;
  /** Optional button label - if provided, shown alongside icon */
  label?: string;
  /** Optional CSS classes */
  class?: string;
  /** Icon-only variant (no background, minimal padding) */
  iconOnly?: boolean;
  /** Size variant */
  size?: 'sm' | 'md' | 'lg';
}

const CopyButton: Component<CopyButtonProps> = (props) => {
  const [copied, setCopied] = createSignal(false);
  const [ripples, setRipples] = createSignal<Array<{ id: number; x: number; y: number }>>([]);

  let rippleId = 0;

  const handleCopy = async (e: MouseEvent) => {
    const button = e.currentTarget as HTMLButtonElement;
    const rect = button.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const id = ++rippleId;
    
    setRipples(prev => [...prev, { id, x, y }]);
    setTimeout(() => {
      setRipples(prev => prev.filter(r => r.id !== id));
    }, 600);

    try {
      await navigator.clipboard.writeText(props.text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      const textarea = document.createElement('textarea');
      textarea.value = props.text;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      try {
        document.execCommand('copy');
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch {
        console.warn('Copy to clipboard failed');
      }
      document.body.removeChild(textarea);
    }
  };

  const sizeClasses = () => {
    const size = props.size || 'md';
    if (props.iconOnly) {
      return {
        sm: 'p-1.5',
        md: 'p-2',
        lg: 'p-3',
      }[size];
    }
    return {
      sm: 'px-2.5 py-1.5 text-xs gap-1.5',
      md: 'px-3 py-2 text-sm gap-2',
      lg: 'px-4 py-2.5 text-base gap-2.5',
    }[size];
  };

  const iconSize = () => {
    const size = props.size || 'md';
    return {
      sm: 14,
      md: 16,
      lg: 18,
    }[size];
  };

  return (
    <button
      type="button"
      onClick={handleCopy}
      class={`
        copy-button group relative inline-flex items-center justify-center
        overflow-hidden rounded-lg font-medium
        transition-all duration-200 ease-out
        focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-900
        ${props.iconOnly
          ? 'bg-transparent hover:bg-slate-700/50 text-slate-400 hover:text-white'
          : 'border border-slate-600/50 bg-slate-800/80 text-slate-300 hover:border-indigo-500/50 hover:bg-slate-700/80 hover:text-white'
        }
        ${copied() ? 'border-green-500/50 text-green-400' : ''}
        ${sizeClasses()}
        ${props.class || ''}
      `}
      aria-label={copied() ? 'Copied to clipboard' : `Copy ${props.label || 'to clipboard'}`}
      title={copied() ? 'Copied!' : 'Copy to clipboard'}
    >
      {ripples().map(ripple => (
        <span
          class="absolute animate-ripple rounded-full bg-white/20 pointer-events-none"
          style={{
            left: `${ripple.x}px`,
            top: `${ripple.y}px`,
            width: '4px',
            height: '4px',
            transform: 'translate(-50%, -50%)',
          }}
        />
      ))}

      <span class="relative flex items-center justify-center">
        <span
          class={`
            absolute inset-0 flex items-center justify-center
            transition-all duration-300 ease-out
            ${copied() ? 'scale-100 opacity-100' : 'scale-50 opacity-0'}
          `}
        >
          <Check size={iconSize()} class="text-green-400" strokeWidth={2.5} />
        </span>
        <span
          class={`
            flex items-center justify-center
            transition-all duration-300 ease-out
            ${copied() ? 'scale-50 opacity-0' : 'scale-100 opacity-100'}
          `}
        >
          <Copy size={iconSize()} strokeWidth={2} />
        </span>
      </span>

      <Show when={props.label}>
        <span class="relative overflow-hidden">
          <span
            class={`
              block transition-all duration-300 ease-out
              ${copied() ? '-translate-y-full opacity-0' : 'translate-y-0 opacity-100'}
            `}
          >
            {props.label}
          </span>
          <span
            class={`
              absolute inset-0 flex items-center
              transition-all duration-300 ease-out
              ${copied() ? 'translate-y-0 opacity-100' : 'translate-y-full opacity-0'}
            `}
          >
            <span class="text-green-400">Copied!</span>
          </span>
        </span>
      </Show>
    </button>
  );
};

export default CopyButton;
