import { Component, createSignal, onMount, For, onCleanup } from 'solid-js';
import GlassCard from '../ui/GlassCard';

const HeroTerminal: Component = () => {
  const [lines, setLines] = createSignal<string[]>([]);
  const [currentLine, setCurrentLine] = createSignal("");
  const [cursorVisible, setCursorVisible] = createSignal(true);

  const demoScript = [
    { text: "omg install bun", delay: 100 },
    { text: "[1/3] Resolving dependencies...", delay: 50, isOutput: true },
    { text: "[2/3] Downloading bun-v1.1.0...", delay: 30, isOutput: true },
    { text: "[3/3] Extracting to /usr/local/bin...", delay: 20, isOutput: true },
    { text: "Success! Bun is now globally available.", delay: 0, isOutput: true },
    { text: "", delay: 500 },
    { text: "omg list --installed", delay: 100 },
    { text: "node v22.1.0", delay: 10, isOutput: true },
    { text: "rust v1.78.0", delay: 10, isOutput: true },
    { text: "bun v1.1.0 (new)", delay: 10, isOutput: true },
  ];

  onMount(() => {
    let scriptIndex = 0;
    let charIndex = 0;
    let timeoutId: number;

    const type = () => {
      const line = demoScript[scriptIndex];
      if (!line) {
        // Restart after a pause
        timeoutId = window.setTimeout(() => {
          setLines([]);
          setCurrentLine("");
          scriptIndex = 0;
          charIndex = 0;
          type();
        }, 5000);
        return;
      }

      if (line.isOutput) {
        setLines(prev => [...prev, line.text]);
        scriptIndex++;
        timeoutId = window.setTimeout(type, 500);
      } else {
        if (charIndex < line.text.length) {
          setCurrentLine(prev => prev + line.text[charIndex]);
          charIndex++;
          timeoutId = window.setTimeout(type, line.delay);
        } else {
          setLines(prev => [...prev, "$ " + line.text]);
          setCurrentLine("");
          charIndex = 0;
          scriptIndex++;
          timeoutId = window.setTimeout(type, 1000);
        }
      }
    };

    type();

    const cursorInterval = setInterval(() => {
      setCursorVisible(v => !v);
    }, 500);

    onCleanup(() => {
      clearTimeout(timeoutId);
      clearInterval(cursorInterval);
    });
  });

  return (
    <div class="perspective-1000 w-full max-w-2xl mx-auto">
      <GlassCard 
        class="transform rotate-x-6 rotate-y--6 hover:rotate-x-0 hover:rotate-y-0 transition-transform duration-700 ease-out"
        style={{ "transform-style": "preserve-3d" }}
      >
        {/* Terminal Header */}
        <div class="flex items-center gap-2 px-4 py-3 border-b border-white/10 bg-white/5">
          <div class="w-3 h-3 rounded-full bg-red-500/80" />
          <div class="w-3 h-3 rounded-full bg-yellow-500/80" />
          <div class="w-3 h-3 rounded-full bg-green-500/80" />
          <div class="ml-2 text-xs font-mono text-white/40 select-none">omg — bash</div>
        </div>

        {/* Terminal Body */}
        <div class="p-6 font-mono text-sm sm:text-base min-h-[320px] bg-black/20">
          <div class="flex flex-col gap-1">
            <For each={lines()}>
              {(line) => (
                <div class={line.startsWith("$") ? "text-cyan-400" : "text-white/70"}>
                  {line}
                </div>
              )}
            </For>
            <div class="text-cyan-400">
              <span class={currentLine() || lines().length === 0 ? "inline" : "hidden"}>$ </span>
              {currentLine()}
              <span class={`inline-block w-2 h-5 bg-cyan-400 align-middle ml-1 ${cursorVisible() ? 'opacity-100' : 'opacity-0'}`} />
            </div>
          </div>
        </div>
      </GlassCard>
    </div>
  );
};

export default HeroTerminal;
