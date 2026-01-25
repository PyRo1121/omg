import { Component, createEffect, createSignal, For } from 'solid-js';
import { Terminal, Shield, Zap, Globe } from '../../ui/Icons';

interface CommandEvent {
  id: string;
  event_name: string;
  created_at: string;
  platform: string;
  version: string;
  properties?: {
    command?: string;
  };
}

interface CommandStreamProps {
  events: CommandEvent[];
}

export const CommandStream: Component<CommandStreamProps> = (props) => {
  let terminalRef: HTMLDivElement | undefined;
  const [displayedEvents, setDisplayedEvents] = createSignal<CommandEvent[]>([]);

  // Automatically scroll to bottom on new events
  createEffect(() => {
    if (props.events.length > 0) {
      // Sync events to local display state - reversed to show latest at bottom
      setDisplayedEvents([...props.events].reverse().slice(-50));

      if (terminalRef) {
        terminalRef.scrollTop = terminalRef.scrollHeight;
      }
    }
  });

  const getEventIcon = (name: string) => {
    if (name.includes('security')) return <Shield size={12} class="text-rose-400" />;
    if (name.includes('install')) return <Zap size={12} class="text-emerald-400" />;
    if (name.includes('heartbeat')) return <Globe size={12} class="text-indigo-400" />;
    return <Terminal size={12} class="text-slate-400" />;
  };

  return (
    <div
      ref={terminalRef}
      class="h-[400px] overflow-y-auto rounded-xl bg-black/40 p-4 font-mono text-[11px] leading-relaxed shadow-inner no-scrollbar border border-white/5"
    >
      <div class="space-y-1">
        <div class="text-slate-500 mb-2 select-none">
          # OMG System Command - Secure Telemetry Stream v1.0.0
          <br />
          # Listening for global events...
        </div>

        <For each={displayedEvents()}>
          {(event) => (
            <div class="group flex items-start gap-3 border-l border-white/5 pl-3 hover:bg-white/5 transition-colors animate-in fade-in slide-in-from-left-1 duration-300">
              <span class="shrink-0 text-slate-600 select-none">
                [{new Date(event.created_at).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}]
              </span>

              <div class="flex items-center gap-2 shrink-0">
                {getEventIcon(event.event_name)}
                <span class={`font-bold uppercase tracking-tighter ${
                  event.event_name === 'command_run' ? 'text-indigo-400' : 'text-slate-400'
                }`}>
                  {event.event_name.replace('_', '.')}
                </span>
              </div>

              <div class="flex-1 truncate">
                <span class="text-slate-400">{event.platform}</span>
                <span class="mx-2 text-slate-700">::</span>
                <span class="text-white font-medium">{event.properties?.command || 'heartbeat'}</span>
              </div>

              <span class="shrink-0 text-[9px] font-black text-slate-700 uppercase group-hover:text-slate-500 transition-colors">
                v{event.version}
              </span>
            </div>
          )}
        </For>

        <div class="flex items-center gap-2 pt-2 animate-pulse">
          <span class="h-1.5 w-1.5 rounded-full bg-emerald-500" />
          <span class="text-slate-600 italic">Streaming global telemetry...</span>
        </div>
      </div>
    </div>
  );
};
