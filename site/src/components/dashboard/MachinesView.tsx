import { Component, createSignal, For, Show } from 'solid-js';
import {
  Monitor,
  Trash2,
  Cpu,
  Clock,
  CheckCircle2,
  AlertCircle,
  Laptop
} from 'lucide-solid';
import * as api from '../../lib/api';

interface MachinesViewProps {
  machines: api.Machine[];
  onRevoke: () => void;
}

export const MachinesView: Component<MachinesViewProps> = (props) => {
  const [revokingId, setRevokingId] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const handleRevoke = async (machineId: string) => {
    if (!confirm('Are you sure you want to revoke access for this machine? It will need to be re-authenticated.')) {
      return;
    }

    setRevokingId(machineId);
    setError(null);

    try {
      const res = await api.revokeMachine(machineId);
      if (res.success) {
        props.onRevoke(); // Refresh parent data
      } else {
        setError('Failed to revoke machine');
      }
    } catch (e) {
      setError('Network error');
    } finally {
      setRevokingId(null);
    }
  };

  const getOsIcon = (os: string | null) => {
    const lower = os?.toLowerCase() || '';
    if (lower.includes('linux')) return 'bg-orange-500/20 text-orange-400';
    if (lower.includes('darwin') || lower.includes('mac')) return 'bg-slate-500/20 text-white';
    if (lower.includes('win')) return 'bg-blue-500/20 text-blue-400';
    return 'bg-slate-500/20 text-slate-400';
  };

  const formatLastSeen = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);

    if (minutes < 5) return 'Online';
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return date.toLocaleDateString();
  };

  // Styles
  const glassCard = "bg-white/5 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-xl hover:border-white/20 transition-all duration-300 group relative overflow-hidden";
  const glow = "absolute -inset-px bg-gradient-to-r from-blue-500/10 to-purple-500/10 opacity-0 group-hover:opacity-100 transition-opacity duration-500 rounded-2xl pointer-events-none";

  return (
    <div class="space-y-6 animate-fade-in">
      <div class="flex items-center justify-between mb-8">
        <div>
          <h2 class="text-2xl font-bold text-white mb-2">Connected Machines</h2>
          <p class="text-slate-400">Manage access for your CLI installations.</p>
        </div>
        <div class="px-4 py-2 rounded-xl bg-blue-500/10 border border-blue-500/20 text-blue-400 font-mono text-sm">
          {props.machines.length} Active
        </div>
      </div>

      <Show when={error()}>
        <div class="p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-red-400 flex items-center gap-3 mb-6">
          <AlertCircle class="w-5 h-5" />
          {error()}
        </div>
      </Show>

      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        <For each={props.machines}>
          {(machine) => (
            <div class={glassCard}>
              <div class={glow} />

              {/* Header */}
              <div class="flex items-start justify-between mb-6 relative z-10">
                <div class="flex items-center gap-4">
                  <div class={`w-12 h-12 rounded-xl flex items-center justify-center ${getOsIcon(machine.os)}`}>
                    <Monitor class="w-6 h-6" />
                  </div>
                  <div>
                    <h3 class="font-bold text-white text-lg truncate max-w-[150px]" title={machine.hostname || 'Unknown Host'}>
                      {machine.hostname || 'Unknown Host'}
                    </h3>
                    <div class="flex items-center gap-2 mt-1">
                      <span class={`w-2 h-2 rounded-full ${
                        formatLastSeen(machine.last_seen_at) === 'Online' ? 'bg-emerald-400 animate-pulse' : 'bg-slate-600'
                      }`} />
                      <span class="text-xs text-slate-400 font-mono">
                        {formatLastSeen(machine.last_seen_at)}
                      </span>
                    </div>
                  </div>
                </div>

                <button
                  onClick={() => handleRevoke(machine.machine_id)}
                  disabled={!!revokingId()}
                  class="p-2 rounded-lg hover:bg-red-500/10 text-slate-500 hover:text-red-400 transition-colors"
                  title="Revoke Access"
                >
                  <Show when={revokingId() === machine.machine_id} fallback={<Trash2 class="w-5 h-5" />}>
                    <div class="w-5 h-5 border-2 border-red-400 border-t-transparent rounded-full animate-spin" />
                  </Show>
                </button>
              </div>

              {/* Details */}
              <div class="space-y-3 relative z-10">
                <div class="flex items-center justify-between text-sm">
                  <div class="flex items-center gap-2 text-slate-500">
                    <Laptop class="w-4 h-4" />
                    <span>OS / Arch</span>
                  </div>
                  <span class="text-slate-300 font-mono text-xs">
                    {machine.os || 'Unknown'} / {machine.arch || 'x64'}
                  </span>
                </div>

                <div class="flex items-center justify-between text-sm">
                  <div class="flex items-center gap-2 text-slate-500">
                    <Cpu class="w-4 h-4" />
                    <span>CLI Version</span>
                  </div>
                  <span class="text-slate-300 font-mono text-xs bg-slate-800 px-2 py-0.5 rounded">
                    v{machine.omg_version || '0.0.0'}
                  </span>
                </div>

                <div class="flex items-center justify-between text-sm">
                  <div class="flex items-center gap-2 text-slate-500">
                    <Clock class="w-4 h-4" />
                    <span>Added</span>
                  </div>
                  <span class="text-slate-300 text-xs">
                    {new Date(machine.first_seen_at).toLocaleDateString()}
                  </span>
                </div>
              </div>

              {/* Status Footer */}
              <div class="mt-6 pt-4 border-t border-white/5 flex items-center justify-between relative z-10">
                <div class="flex items-center gap-2 text-emerald-400 text-xs font-medium bg-emerald-500/10 px-2 py-1 rounded-lg border border-emerald-500/20">
                  <CheckCircle2 class="w-3 h-3" />
                  Authorized
                </div>
                <span class="text-[10px] text-slate-600 font-mono">
                  ID: {machine.machine_id.slice(0, 8)}
                </span>
              </div>
            </div>
          )}
        </For>

        {/* Empty State / Add New */}
        <div class="border-2 border-dashed border-slate-800 rounded-2xl p-6 flex flex-col items-center justify-center text-center hover:border-slate-700 transition-colors group cursor-default min-h-[200px]">
          <div class="w-12 h-12 rounded-full bg-slate-900 flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
            <Monitor class="w-6 h-6 text-slate-600 group-hover:text-blue-400 transition-colors" />
          </div>
          <h3 class="text-slate-400 font-medium mb-1">Connect New Machine</h3>
          <p class="text-slate-600 text-sm max-w-[200px]">
            Run <code class="bg-slate-900 px-1 py-0.5 rounded text-blue-400 font-mono">omg login</code> in your terminal to add a device.
          </p>
        </div>
      </div>
    </div>
  );
};
