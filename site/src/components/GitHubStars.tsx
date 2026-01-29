import { Component, createResource, Suspense, Show } from 'solid-js';

const GITHUB_REPO_URL = 'https://github.com/PyRo1121/omg';
const GITHUB_API_URL = 'https://api.github.com/repos/PyRo1121/omg';

interface GitHubRepoData {
  stargazers_count: number;
}

const formatStarCount = (count: number): string => {
  if (count >= 1000) {
    const wholeThousands = Math.floor(count / 1000);
    const remainder = count % 1000;
    const hasDecimal = remainder >= 100;
    
    return hasDecimal
      ? `${(count / 1000).toFixed(1)}k`
      : `${wholeThousands}k`;
  }
  return count.toString();
};

const fetchStarCount = async (): Promise<number | null> => {
  try {
    const response = await fetch(GITHUB_API_URL, {
      headers: {
        'Accept': 'application/vnd.github.v3+json',
      },
    });
    
    if (!response.ok) {
      return null;
    }
    
    const data: GitHubRepoData = await response.json();
    return data.stargazers_count;
  } catch {
    return null;
  }
};

const StarIcon: Component = () => (
  <svg 
    class="h-4 w-4 text-amber-400" 
    fill="currentColor" 
    viewBox="0 0 20 20"
    aria-hidden="true"
  >
    <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" />
  </svg>
);

const GitHubIcon: Component = () => (
  <svg 
    class="h-4 w-4" 
    fill="currentColor" 
    viewBox="0 0 24 24"
    aria-hidden="true"
  >
    <path fill-rule="evenodd" clip-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" />
  </svg>
);

const LoadingPulse: Component = () => (
  <div class="h-4 w-6 animate-pulse rounded bg-slate-600" />
);

interface StarCountDisplayProps {
  count: number | null | undefined;
}

const StarCountDisplay: Component<StarCountDisplayProps> = (props) => (
  <Show 
    when={props.count !== null && props.count !== undefined}
    fallback={<span class="text-xs text-slate-400">Star</span>}
  >
    <span class="text-xs font-medium tabular-nums text-slate-300">
      {formatStarCount(props.count!)}
    </span>
  </Show>
);

const GitHubStars: Component = () => {
  const [starCount] = createResource(fetchStarCount);

  return (
    <a
      href={GITHUB_REPO_URL}
      target="_blank"
      rel="noopener noreferrer"
      class="group flex items-center gap-2 rounded-full border border-slate-700/80 bg-slate-800/50 px-3 py-1.5 text-slate-400 transition-all duration-200 hover:border-slate-600 hover:bg-slate-700/50 hover:text-white"
      aria-label="Star OMG on GitHub"
    >
      <GitHubIcon />
      <div class="flex items-center gap-1.5">
        <StarIcon />
        <Suspense fallback={<LoadingPulse />}>
          <StarCountDisplay count={starCount()} />
        </Suspense>
      </div>
    </a>
  );
};

export default GitHubStars;
