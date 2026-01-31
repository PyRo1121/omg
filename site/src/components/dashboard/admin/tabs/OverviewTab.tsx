import { Component } from 'solid-js';
import {
  ExecutiveKPIDashboard,
  RealTimeCommandCenter,
} from '../../premium';
import type {
  ExecutiveKPI,
  AdvancedMetrics,
  FirehoseEvent,
  GeoDistribution,
  CommandHealth,
} from '../../premium/types';

interface OverviewTabProps {
  executiveKPI: ExecutiveKPI;
  advancedMetrics: AdvancedMetrics | undefined;
  firehoseEvents: FirehoseEvent[];
  geoDistribution: GeoDistribution[];
  commandHealth: CommandHealth;
  isMetricsLoading: boolean;
  onRefresh: () => void;
}

export const OverviewTab: Component<OverviewTabProps> = (props) => {
  return (
    <div class="space-y-8">
      <ExecutiveKPIDashboard
        kpi={props.executiveKPI}
        metrics={props.advancedMetrics}
        isLoading={props.isMetricsLoading}
      />

      <RealTimeCommandCenter
        events={props.firehoseEvents}
        geoDistribution={props.geoDistribution}
        commandHealth={props.commandHealth}
        isLive={true}
        onRefresh={props.onRefresh}
      />
    </div>
  );
};
