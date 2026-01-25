import { Component, For } from 'solid-js';
import GlassCard from '../ui/GlassCard';

const features = [
  {
    title: "Universal",
    description: "One tool for Node, Python, Go, Rust, and 100+ more via mise integration.",
    icon: (
      <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9-9c1.657 0 3 4.03 3 9s-1.343 9-3 9m0-18c-1.657 0-3 4.03-3 9s1.343 9 3 9" />
      </svg>
    ),
    color: "text-blue-400"
  },
  {
    title: "Sandboxed",
    description: "Every installation is isolated and reproducible. No more 'works on my machine' issues.",
    icon: (
      <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
      </svg>
    ),
    color: "text-green-400"
  },
  {
    title: "Blazing Fast",
    description: "Written in pure Rust with zero overhead. Up to 22x faster than traditional managers.",
    icon: (
      <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
      </svg>
    ),
    color: "text-yellow-400"
  },
  {
    title: "Team Sync",
    description: "Share environment definitions across your team with built-in drift detection.",
    icon: (
      <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
      </svg>
    ),
    color: "text-purple-400"
  }
];

const FeatureGrid: Component = () => {
  return (
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 px-6 max-w-7xl mx-auto py-20">
      <For each={features}>
        {(feature) => (
          <GlassCard class="p-8 group hover:border-white/20 transition-all duration-300">
            <div class={`mb-4 ${feature.color} opacity-80 group-hover:opacity-100 transition-opacity`}>
              {feature.icon}
            </div>
            <h3 class="text-xl font-bold text-white mb-3">{feature.title}</h3>
            <p class="text-slate-400 text-sm leading-relaxed">
              {feature.description}
            </p>
          </GlassCard>
        )}
      </For>
    </div>
  );
};

export default FeatureGrid;
