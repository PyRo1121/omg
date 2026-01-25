import { ParentComponent, JSX } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface GlassCardProps {
  class?: string;
  style?: JSX.CSSProperties;
}

const GlassCard: ParentComponent<GlassCardProps> = (props) => {
  return (
    <div
      class={cn(
        "backdrop-blur-xl bg-white/5 border border-white/10 rounded-xl shadow-2xl overflow-hidden",
        props.class
      )}
      style={props.style}
    >
      {props.children}
    </div>
  );
};

export default GlassCard;
