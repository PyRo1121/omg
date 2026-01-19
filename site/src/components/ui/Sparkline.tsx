import type { Component } from 'solid-js';

interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  fillOpacity?: number;
  strokeWidth?: number;
  showDots?: boolean;
}

export const Sparkline: Component<SparklineProps> = props => {
  const width = props.width || 120;
  const height = props.height || 32;
  const color = props.color || '#6366f1';
  const strokeWidth = props.strokeWidth || 2;
  const fillOpacity = props.fillOpacity || 0.1;

  const points = () => {
    const data = props.data || [];
    if (data.length === 0) return '';
    
    const max = Math.max(...data, 1);
    const min = Math.min(...data, 0);
    const range = max - min || 1;
    const padding = 4;
    const chartWidth = width - padding * 2;
    const chartHeight = height - padding * 2;
    
    return data.map((value, i) => {
      const x = padding + (i / (data.length - 1 || 1)) * chartWidth;
      const y = padding + chartHeight - ((value - min) / range) * chartHeight;
      return `${x},${y}`;
    }).join(' ');
  };

  const areaPath = () => {
    const data = props.data || [];
    if (data.length === 0) return '';
    
    const max = Math.max(...data, 1);
    const min = Math.min(...data, 0);
    const range = max - min || 1;
    const padding = 4;
    const chartWidth = width - padding * 2;
    const chartHeight = height - padding * 2;
    
    const pts = data.map((value, i) => {
      const x = padding + (i / (data.length - 1 || 1)) * chartWidth;
      const y = padding + chartHeight - ((value - min) / range) * chartHeight;
      return { x, y };
    });
    
    if (pts.length === 0) return '';
    
    let path = `M ${pts[0].x},${height - padding}`;
    pts.forEach(pt => { path += ` L ${pt.x},${pt.y}`; });
    path += ` L ${pts[pts.length - 1].x},${height - padding} Z`;
    
    return path;
  };

  const lastPoint = () => {
    const data = props.data || [];
    if (data.length === 0) return null;
    
    const max = Math.max(...data, 1);
    const min = Math.min(...data, 0);
    const range = max - min || 1;
    const padding = 4;
    const chartWidth = width - padding * 2;
    const chartHeight = height - padding * 2;
    
    const lastValue = data[data.length - 1];
    const x = padding + chartWidth;
    const y = padding + chartHeight - ((lastValue - min) / range) * chartHeight;
    
    return { x, y };
  };

  return (
    <svg width={width} height={height} class="overflow-visible">
      <defs>
        <linearGradient id={`sparkline-gradient-${color.replace('#', '')}`} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stop-color={color} stop-opacity={fillOpacity * 2} />
          <stop offset="100%" stop-color={color} stop-opacity="0" />
        </linearGradient>
      </defs>
      <path
        d={areaPath()}
        fill={`url(#sparkline-gradient-${color.replace('#', '')})`}
        class="transition-all duration-500"
      />
      <polyline
        points={points()}
        fill="none"
        stroke={color}
        stroke-width={strokeWidth}
        stroke-linecap="round"
        stroke-linejoin="round"
        class="transition-all duration-500"
      />
      {props.showDots && lastPoint() && (
        <g>
          <circle
            cx={lastPoint()!.x}
            cy={lastPoint()!.y}
            r="4"
            fill={color}
            class="animate-pulse"
          />
          <circle
            cx={lastPoint()!.x}
            cy={lastPoint()!.y}
            r="6"
            fill={color}
            opacity="0.3"
            class="animate-ping"
          />
        </g>
      )}
    </svg>
  );
};

interface ProgressRingProps {
  value: number;
  max?: number;
  size?: number;
  strokeWidth?: number;
  color?: string;
  bgColor?: string;
  showValue?: boolean;
  label?: string;
}

export const ProgressRing: Component<ProgressRingProps> = props => {
  const size = props.size || 80;
  const strokeWidth = props.strokeWidth || 8;
  const max = props.max || 100;
  const color = props.color || '#6366f1';
  const bgColor = props.bgColor || '#1e293b';
  
  const radius = (size - strokeWidth) / 2;
  const circumference = radius * 2 * Math.PI;
  const progress = () => Math.min(props.value / max, 1);
  const offset = () => circumference - progress() * circumference;

  return (
    <div class="relative inline-flex items-center justify-center">
      <svg width={size} height={size} class="-rotate-90">
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={bgColor}
          stroke-width={strokeWidth}
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={color}
          stroke-width={strokeWidth}
          stroke-linecap="round"
          stroke-dasharray={`${circumference}`}
          stroke-dashoffset={`${offset()}`}
          class="transition-all duration-700 ease-out"
        />
      </svg>
      {props.showValue && (
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <span class="text-lg font-bold text-white">{Math.round(progress() * 100)}%</span>
          {props.label && <span class="text-xs text-slate-400">{props.label}</span>}
        </div>
      )}
    </div>
  );
};

interface TrendIndicatorProps {
  value: number;
  previousValue: number;
  format?: 'percent' | 'number' | 'currency';
  size?: 'sm' | 'md' | 'lg';
}

export const TrendIndicator: Component<TrendIndicatorProps> = props => {
  const change = () => {
    if (props.previousValue === 0) return props.value > 0 ? 100 : 0;
    return ((props.value - props.previousValue) / props.previousValue) * 100;
  };
  
  const isPositive = () => change() >= 0;
  const sizeClasses = {
    sm: 'text-xs gap-0.5',
    md: 'text-sm gap-1',
    lg: 'text-base gap-1.5',
  };

  const formatValue = () => {
    const val = Math.abs(change());
    switch (props.format) {
      case 'currency':
        return `$${val.toFixed(0)}`;
      case 'number':
        return val.toFixed(0);
      default:
        return `${val.toFixed(1)}%`;
    }
  };

  return (
    <div class={`inline-flex items-center ${sizeClasses[props.size || 'sm']} ${
      isPositive() ? 'text-emerald-400' : 'text-red-400'
    }`}>
      <svg 
        class={`h-3 w-3 ${isPositive() ? '' : 'rotate-180'}`} 
        fill="none" 
        viewBox="0 0 24 24" 
        stroke="currentColor"
      >
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 15l7-7 7 7" />
      </svg>
      <span class="font-medium">{formatValue()}</span>
    </div>
  );
};

export default Sparkline;
