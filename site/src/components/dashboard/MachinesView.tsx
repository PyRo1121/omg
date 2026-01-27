import { Component, createSignal, Show } from 'solid-js';
import { AlertCircle, RefreshCw } from '../ui/Icons';
import { useFleetStatus } from '../../lib/api-hooks';
import { FleetTable } from './tables/FleetTable';
import * as api from '../../lib/api';

interface MachinesViewProps {
  machines: api.Machine[];
  onRevoke: () => void;
}

export const MachinesView: Component<MachinesViewProps> = (props) => {
  const fleetQuery = useFleetStatus();
  const [error, setError] = createSignal<string | null>(null);

  const handleRevoke = async (machineIds: string[]) => {
    const isBulk = machineIds.length > 1;
    const message = isBulk 
      ? `Are you sure you want to revoke access for ${machineIds.length} machines?`
      : 'Are you sure you want to revoke access for this machine?';

    if (!confirm(message)) {
      return;
    }

    try {
      setError(null);
      // Execute all revocations in parallel
      const results = await Promise.all(
        machineIds.map(id => api.revokeMachine(id))
      );
      
      const allSuccessful = results.every(res => res.success);
      
      if (allSuccessful) {
        fleetQuery.refetch();
        props.onRevoke();
      } else {
        const failedCount = results.filter(res => !res.success).length;
        setError(`Failed to revoke ${failedCount} machine(s)`);
        fleetQuery.refetch();
      }
    } catch (_e) {
      setError('Network error during revocation');
    }
  };

  return (
    <div class="space-y-6 animate-fade-in">
      <div class="flex items-center justify-between mb-8">
        <div>
          <h2 class="text-2xl font-bold text-white mb-2">Connected Machines</h2>
          <p class="text-slate-400">Manage access for your CLI installations.</p>
        </div>
        <div class="flex items-center gap-4">
          <button 
            onClick={() => fleetQuery.refetch()}
            class="p-2 rounded-xl bg-white/5 border border-white/10 text-slate-400 hover:text-white transition-all"
            title="Refresh Fleet"
          >
            <RefreshCw size={18} class={fleetQuery.isFetching ? 'animate-spin' : ''} />
          </button>
          <div class="px-4 py-2 rounded-xl bg-blue-500/10 border border-blue-500/20 text-blue-400 font-mono text-sm">
            {fleetQuery.data?.length || 0} Active
          </div>
        </div>
      </div>

      <Show when={error() || fleetQuery.error}>
        <div class="p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-red-400 flex items-center gap-3 mb-6">
          <AlertCircle class="w-5 h-5" />
          {error() || 'Failed to load fleet data'}
        </div>
      </Show>

      <Show when={fleetQuery.isLoading}>
        <div class="flex items-center justify-center py-20">
          <div class="w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
        </div>
      </Show>

      <Show when={fleetQuery.isSuccess}>
        <FleetTable 
          data={fleetQuery.data || []} 
          onRevoke={handleRevoke} 
        />
      </Show>
    </div>
  );
};
