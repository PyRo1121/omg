import { ParentProps, Match, Switch } from "solid-js";
import {
  Info,
  AlertTriangle,
  XCircle,
  Lightbulb,
  CheckCircle,
} from "lucide-solid";

type CalloutType = "info" | "warning" | "danger" | "tip" | "success";

interface CalloutProps extends ParentProps {
  type?: CalloutType;
  title?: string;
}

const iconMap = {
  info: Info,
  warning: AlertTriangle,
  danger: XCircle,
  tip: Lightbulb,
  success: CheckCircle,
};

const styleMap = {
  info: "border-blue-500/50 bg-blue-500/10 text-blue-200",
  warning: "border-yellow-500/50 bg-yellow-500/10 text-yellow-200",
  danger: "border-red-500/50 bg-red-500/10 text-red-200",
  tip: "border-purple-500/50 bg-purple-500/10 text-purple-200",
  success: "border-green-500/50 bg-green-500/10 text-green-200",
};

const iconColorMap = {
  info: "text-blue-400",
  warning: "text-yellow-400",
  danger: "text-red-400",
  tip: "text-purple-400",
  success: "text-green-400",
};

const defaultTitles = {
  info: "Info",
  warning: "Warning",
  danger: "Danger",
  tip: "Tip",
  success: "Success",
};

export function Callout(props: CalloutProps) {
  const type = () => props.type || "info";
  const Icon = () => iconMap[type()];
  const title = () => props.title || defaultTitles[type()];

  return (
    <div
      class={`my-6 rounded-r-lg border-l-4 p-4 ${styleMap[type()]}`}
    >
      <div class="flex items-start gap-3">
        <span class={`mt-0.5 flex-shrink-0 ${iconColorMap[type()]}`}>
          <Switch>
            <Match when={type() === "info"}>
              <Info size={20} />
            </Match>
            <Match when={type() === "warning"}>
              <AlertTriangle size={20} />
            </Match>
            <Match when={type() === "danger"}>
              <XCircle size={20} />
            </Match>
            <Match when={type() === "tip"}>
              <Lightbulb size={20} />
            </Match>
            <Match when={type() === "success"}>
              <CheckCircle size={20} />
            </Match>
          </Switch>
        </span>
        <div class="min-w-0 flex-1">
          <div class="mb-1 font-semibold">{title()}</div>
          <div class="callout-content text-sm opacity-90">{props.children}</div>
        </div>
      </div>
    </div>
  );
}

export default Callout;
