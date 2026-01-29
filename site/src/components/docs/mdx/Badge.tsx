import { ParentProps } from "solid-js";

type BadgeVariant =
  | "default"
  | "success"
  | "warning"
  | "danger"
  | "info"
  | "purple";

interface BadgeProps extends ParentProps {
  variant?: BadgeVariant;
}

const variantStyles: Record<BadgeVariant, string> = {
  default: "bg-slate-700 text-slate-300",
  success: "bg-green-500/20 text-green-400 border-green-500/30",
  warning: "bg-yellow-500/20 text-yellow-400 border-yellow-500/30",
  danger: "bg-red-500/20 text-red-400 border-red-500/30",
  info: "bg-blue-500/20 text-blue-400 border-blue-500/30",
  purple: "bg-purple-500/20 text-purple-400 border-purple-500/30",
};

export function Badge(props: BadgeProps) {
  const variant = () => props.variant || "default";

  return (
    <span
      class={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium ${variantStyles[variant()]}`}
    >
      {props.children}
    </span>
  );
}

export default Badge;
