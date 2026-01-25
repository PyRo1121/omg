import { Component, mergeProps } from 'solid-js';
import { SolidApexCharts } from 'solid-apexcharts';
import GlassCard from '../../ui/GlassCard';
import { LiveIndicator } from '../../ui/Chart';

interface RoiChartProps {
  data: number[];
  labels?: string[];
  height?: number;
  title?: string;
  subtitle?: string;
  peakVelocity?: number;
}

export const RoiChart: Component<RoiChartProps> = (rawProps) => {
  const props = mergeProps({ height: 300, title: 'Organization Productivity', subtitle: 'Aggregate efficiency gains over the last period.' }, rawProps);

  const options = {
    chart: {
      type: 'area',
      toolbar: { show: false },
      sparkline: { enabled: false },
      background: 'transparent',
      animations: {
        enabled: true,
        easing: 'easeinout',
        speed: 800,
      }
    },
    stroke: {
      curve: 'smooth',
      width: 3,
      colors: ['#6366f1']
    },
    fill: {
      type: 'gradient',
      gradient: {
        shadeIntensity: 1,
        opacityFrom: 0.45,
        opacityTo: 0.05,
        stops: [20, 100],
        colorStops: [
          {
            offset: 0,
            color: '#6366f1',
            opacity: 0.4
          },
          {
            offset: 100,
            color: '#6366f1',
            opacity: 0
          }
        ]
      }
    },
    grid: {
      borderColor: 'rgba(255, 255, 255, 0.05)',
      strokeDashArray: 4,
      padding: { left: 20, right: 20 }
    },
    xaxis: {
      categories: props.labels || (props.data || []).map((_, i) => `D${i}`),
      axisBorder: { show: false },
      axisTicks: { show: false },
      labels: {
        style: {
          colors: '#64748b',
          fontSize: '10px',
          fontWeight: 700,
        }
      }
    },
    yaxis: {
      labels: {
        style: {
          colors: '#64748b',
          fontSize: '10px',
          fontWeight: 700,
        },
        formatter: (val: number) => `+${val}%`
      }
    },
    tooltip: {
      theme: 'dark',
      x: { show: false },
      y: {
        formatter: (val: number) => `+${val}% gain`
      }
    },
    colors: ['#6366f1'],
    dataLabels: { enabled: false }
  };

  const series = [{
    name: 'Productivity Gain',
    data: props.data
  }];

  return (
    <GlassCard class="p-10">
      <div class="mb-10 flex items-center justify-between">
        <div>
          <h3 class="text-2xl font-black text-white tracking-tight">{props.title}</h3>
          <p class="text-sm font-medium text-slate-500">{props.subtitle}</p>
        </div>
        <div class="flex items-center gap-6">
          <div class="text-right">
            <p class="text-[10px] font-bold uppercase tracking-widest text-slate-500">Peak Velocity</p>
            <p class="text-lg font-black text-white">+{props.peakVelocity || 0}%</p>
          </div>
          <div class="h-10 w-[1px] bg-white/10" />
          <LiveIndicator label="Streaming" />
        </div>
      </div>
      <div class="min-h-[300px]">
        <SolidApexCharts
          type="area"
          options={options}
          series={series}
          height={props.height}
        />
      </div>
    </GlassCard>
  );
};
