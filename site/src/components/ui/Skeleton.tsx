import { Component, JSX } from 'solid-js';

interface SkeletonProps {
  class?: string;
  width?: string;
  height?: string;
  circle?: boolean;
}

export const Skeleton: Component<SkeletonProps> = (props) => {
  return (
    <div
      class={`animate-pulse bg-white/5 ${props.circle ? 'rounded-full' : 'rounded-lg'} ${props.class || ''}`}
      style={{
        width: props.width,
        height: props.height,
      }}
    />
  );
};

export const CardSkeleton: Component = () => {
  return (
    <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
      <div class="mb-8 flex items-center justify-between">
        <Skeleton width="200px" height="32px" />
        <Skeleton width="24px" height="24px" />
      </div>
      <div class="space-y-4">
        <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02] border border-white/[0.03]">
          <div class="space-y-2">
            <div class="flex items-center gap-2">
              <Skeleton width="60px" height="20px" />
              <Skeleton width="100px" height="20px" />
            </div>
            <Skeleton width="150px" height="16px" />
          </div>
          <Skeleton width="24px" height="24px" />
        </div>
        <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02] border border-white/[0.03]">
          <div class="space-y-2">
            <div class="flex items-center gap-2">
              <Skeleton width="60px" height="20px" />
              <Skeleton width="100px" height="20px" />
            </div>
            <Skeleton width="150px" height="16px" />
          </div>
          <Skeleton width="24px" height="24px" />
        </div>
        <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02] border border-white/[0.03]">
          <div class="space-y-2">
            <div class="flex items-center gap-2">
              <Skeleton width="60px" height="20px" />
              <Skeleton width="100px" height="20px" />
            </div>
            <Skeleton width="150px" height="16px" />
          </div>
          <Skeleton width="24px" height="24px" />
        </div>
      </div>
    </div>
  );
};
