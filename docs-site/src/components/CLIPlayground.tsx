import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import styles from './CLIPlayground.module.css';

interface CommandResponse {
  command: string;
  output: string;
  duration?: string;
  success?: boolean;
}

const MOCK_COMMANDS: Record<string, CommandResponse> = {
  'omg search firefox': {
    command: 'omg search firefox',
    output: `  extra/firefox 121.0-1 [3.4 MiB]
    Standalone web browser from mozilla.org
  extra/firefox-developer-edition 122.0b1-1 [3.5 MiB]
    Developer Edition of the popular Firefox web browser
  aur/firefox-nightly 123.0a1-1 [COMMUNITY]
    Nightly build of Firefox`,
    duration: '6ms',
    success: true,
  },
  'omg use node 20': {
    command: 'omg use node 20',
    output: `⚡ Installing Node.js 20.10.0...
📦 Downloading... 100%
🔧 Extracting... done
✓ Activated Node.js 20.10.0

$ node --version
v20.10.0`,
    duration: '2.3s',
    success: true,
  },
  'omg status': {
    command: 'omg status',
    output: `System Status
─────────────
Packages: 1,247 total (892 explicit, 12 orphans)
Updates:  3 available
Security: 0 vulnerabilities

Active Runtimes
───────────────
node:   20.10.0
python: 3.12.0
rust:   1.75.0 (stable)

Daemon: connected (pid 1234)`,
    duration: '3ms',
    success: true,
  },
  'omg install neovim': {
    command: 'omg install neovim',
    output: `⚡ Resolving dependencies...
📦 Installing neovim 0.9.5-1 [3.2 MiB]
✓ Installation complete

$ nvim --version
NVIM v0.9.5`,
    duration: '891ms',
    success: true,
  },
  'omg run dev': {
    command: 'omg run dev',
    output: `⚡ Running task: dev
🔧 Starting development server...

> vite dev

  VITE v5.0.10  ready in 234 ms

  ➜  Local:   http://localhost:5173/
  ➜  Network: use --host to expose`,
    duration: '1.2s',
    success: true,
  },
  'omg help': {
    command: 'omg help',
    output: `OMG - One tool. Every package. All runtimes.

Usage: omg <command> [options]

Commands:
  search <query>      Search for packages
  install <pkg>       Install a package
  use <runtime> <ver> Switch runtime version
  run <task>          Run a project task
  status              Show system status
  help                Show this help

Try: omg search firefox`,
    duration: '1ms',
    success: true,
  },
  'omg update': {
    command: 'omg update',
    output: `⚡ Checking for updates...
📦 Found 3 updates available:

  firefox: 121.0-1 → 121.0-2
  rust: 1.75.0 → 1.76.0
  python: 3.12.0 → 3.12.1

Run 'omg upgrade' to install updates`,
    duration: '142ms',
    success: true,
  },
};

const SUGGESTIONS = [
  'omg search firefox',
  'omg use node 20',
  'omg install neovim',
  'omg run dev',
  'omg status',
  'omg update',
  'omg help',
];

export default function CLIPlayground() {
  const [input, setInput] = useState('');
  const [history, setHistory] = useState<CommandResponse[]>([]);
  const [commandIndex, setCommandIndex] = useState(-1);
  const [suggestion, setSuggestion] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [history]);

  useEffect(() => {
    // Auto-suggest based on input
    if (input.length > 0) {
      const match = SUGGESTIONS.find(cmd =>
        cmd.toLowerCase().startsWith(input.toLowerCase()) && cmd !== input
      );
      setSuggestion(match ? match.slice(input.length) : '');
    } else {
      setSuggestion('');
    }
  }, [input]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = input.trim();
    if (!trimmed) return;

    const response = MOCK_COMMANDS[trimmed] || {
      command: trimmed,
      output: `Command not found: ${trimmed}\n\nTry one of these:\n${SUGGESTIONS.slice(0, 3).join('\n')}`,
      success: false,
    };

    setHistory([...history, response]);
    setInput('');
    setSuggestion('');
    setCommandIndex(-1);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Tab' && suggestion) {
      e.preventDefault();
      setInput(input + suggestion);
      setSuggestion('');
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const allCommands = Object.keys(MOCK_COMMANDS);
      if (commandIndex < allCommands.length - 1) {
        const newIndex = commandIndex + 1;
        setCommandIndex(newIndex);
        setInput(allCommands[allCommands.length - 1 - newIndex]);
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (commandIndex > 0) {
        const newIndex = commandIndex - 1;
        setCommandIndex(newIndex);
        const allCommands = Object.keys(MOCK_COMMANDS);
        setInput(allCommands[allCommands.length - 1 - newIndex]);
      } else if (commandIndex === 0) {
        setCommandIndex(-1);
        setInput('');
      }
    }
  };

  const handleClear = () => {
    setHistory([]);
    setInput('');
    setSuggestion('');
    setCommandIndex(-1);
  };

  return (
    <motion.div
      className={styles.playground}
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5 }}
    >
      <div className={styles.header}>
        <div className={styles.trafficLights}>
          <span className={styles.dotRed} />
          <span className={styles.dotYellow} />
          <span className={styles.dotGreen} />
        </div>
        <span className={styles.title}>Try OMG Commands</span>
        <button className={styles.clearBtn} onClick={handleClear}>
          Clear
        </button>
      </div>

      <div className={styles.body} ref={containerRef}>
        <div className={styles.welcome}>
          <div className={styles.welcomeText}>
            ⚡ Interactive OMG Terminal Playground
          </div>
          <div className={styles.welcomeHint}>
            Try commands like: <code>omg search firefox</code> or <code>omg use node 20</code>
            <br />
            Press <kbd>Tab</kbd> to autocomplete • <kbd>↑</kbd>/<kbd>↓</kbd> for history
          </div>
        </div>

        <AnimatePresence>
          {history.map((item, i) => (
            <motion.div
              key={i}
              className={styles.commandBlock}
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3 }}
            >
              <div className={styles.promptLine}>
                <span className={styles.promptSymbol}>$</span>
                <span className={styles.commandText}>{item.command}</span>
              </div>
              <pre className={`${styles.output} ${!item.success ? styles.error : ''}`}>
                {item.output}
              </pre>
              {item.duration && (
                <div className={styles.duration}>
                  <span className={styles.speedStreak} />
                  Completed in {item.duration}
                </div>
              )}
            </motion.div>
          ))}
        </AnimatePresence>

        <form onSubmit={handleSubmit} className={styles.inputLine}>
          <span className={styles.promptSymbol}>$</span>
          <div className={styles.inputWrapper}>
            <input
              ref={inputRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a command..."
              className={styles.input}
              autoFocus
              spellCheck={false}
            />
            {suggestion && (
              <span className={styles.suggestion}>
                {suggestion}
              </span>
            )}
          </div>
        </form>
      </div>

      <div className={styles.hints}>
        <div className={styles.hint}>
          <kbd>Tab</kbd> Autocomplete
        </div>
        <div className={styles.hint}>
          <kbd>↑</kbd> <kbd>↓</kbd> History
        </div>
        <div className={styles.hint}>
          <kbd>Enter</kbd> Run
        </div>
      </div>
    </motion.div>
  );
}
