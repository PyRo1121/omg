import React, { useEffect, useState } from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import styles from './index.module.css';

function SpeedMetric({ value, label, suffix = '' }: { value: string; label: string; suffix?: string }) {
  return (
    <div className={styles.metric}>
      <span className={styles.metricValue}>{value}<span className={styles.metricSuffix}>{suffix}</span></span>
      <span className={styles.metricLabel}>{label}</span>
    </div>
  );
}

function TerminalDemo() {
  const [step, setStep] = useState(0);
  const commands = [
    { cmd: 'omg search neovim', delay: 1200 },
    { cmd: '', output: '✓ 47 packages found in 6ms', delay: 800 },
    { cmd: 'omg use node 22', delay: 1200 },
    { cmd: '', output: '✓ Node.js 22.0.0 activated', delay: 800 },
    { cmd: 'omg run dev', delay: 1000 },
    { cmd: '', output: '→ Detected package.json, using bun', delay: 600 },
  ];

  useEffect(() => {
    const timer = setInterval(() => {
      setStep((s) => (s + 1) % commands.length);
    }, 2000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className={styles.terminal}>
      <div className={styles.terminalHeader}>
        <div className={styles.terminalDots}>
          <span></span><span></span><span></span>
        </div>
        <span className={styles.terminalTitle}>~</span>
      </div>
      <div className={styles.terminalBody}>
        {commands.slice(0, step + 1).map((item, i) => (
          <div key={i} className={item.cmd ? styles.terminalCommand : styles.terminalOutput}>
            {item.cmd ? (
              <><span className={styles.prompt}>❯</span> {item.cmd}</>
            ) : (
              item.output
            )}
          </div>
        ))}
        <div className={styles.cursor}></div>
      </div>
    </div>
  );
}

function FeatureCard({ icon, title, description }: { icon: string; title: string; description: string }) {
  return (
    <div className={styles.featureCard}>
      <div className={styles.featureIcon}>{icon}</div>
      <h3>{title}</h3>
      <p>{description}</p>
    </div>
  );
}

function ComparisonBar({ tool, time, maxTime }: { tool: string; time: number; maxTime: number }) {
  const width = (time / maxTime) * 100;
  const isOmg = tool.toLowerCase() === 'omg';

  return (
    <div className={styles.comparisonRow}>
      <span className={styles.comparisonTool}>{tool}</span>
      <div className={styles.comparisonBarContainer}>
        <div
          className={`${styles.comparisonBar} ${isOmg ? styles.comparisonBarOmg : ''}`}
          style={{ width: `${width}%` }}
        >
          <span className={styles.comparisonTime}>{time}ms</span>
        </div>
      </div>
    </div>
  );
}

export default function Home(): React.JSX.Element {
  return (
    <Layout
      title="Documentation"
      description="The fastest unified package manager for Arch Linux and all language runtimes"
    >
      <div className={styles.hero}>
        <div className={styles.heroGrid}></div>
        <div className={styles.heroContent}>
          <div className={styles.heroText}>
            <div className={styles.badge}>22x faster than pacman</div>
            <h1>One tool.<br />Every package.<br />All runtimes.</h1>
            <p className={styles.heroSubtitle}>
              Stop juggling pacman, yay, nvm, pyenv, and rustup.<br />
              OMG unifies everything into one blazing-fast CLI.
            </p>
            <div className={styles.heroActions}>
              <Link to="/quickstart" className={styles.primaryButton}>
                Get Started
              </Link>
              <Link to="/cli" className={styles.secondaryButton}>
                CLI Reference
              </Link>
            </div>
            <div className={styles.installCommand}>
              <code>curl -fsSL https://pyro1121.com/install.sh | bash</code>
            </div>
          </div>
          <div className={styles.heroVisual}>
            <TerminalDemo />
          </div>
        </div>
      </div>

      <section className={styles.metrics}>
        <SpeedMetric value="6" suffix="ms" label="Package search" />
        <SpeedMetric value="22" suffix="x" label="Faster than pacman" />
        <SpeedMetric value="7" suffix="+" label="Language runtimes" />
        <SpeedMetric value="0" suffix="" label="Runtime dependencies" />
      </section>

      <section className={styles.comparison}>
        <h2>Raw Performance</h2>
        <p className={styles.sectionSubtitle}>Measured on real hardware, not benchmarketing</p>
        <div className={styles.comparisonChart}>
          <ComparisonBar tool="OMG" time={6} maxTime={150} />
          <ComparisonBar tool="pacman" time={133} maxTime={150} />
          <ComparisonBar tool="yay" time={145} maxTime={150} />
        </div>
        <p className={styles.comparisonNote}>Package search latency (lower is better)</p>
      </section>

      <section className={styles.features}>
        <h2>Everything you need</h2>
        <p className={styles.sectionSubtitle}>One tool replaces your entire toolkit</p>
        <div className={styles.featureGrid}>
          <FeatureCard
            icon="📦"
            title="System Packages"
            description="Official repos + AUR with automatic detection. Install anything with one command."
          />
          <FeatureCard
            icon="🔧"
            title="Runtime Versions"
            description="Node, Python, Rust, Go, Ruby, Java, Bun. Switch versions instantly."
          />
          <FeatureCard
            icon="🔒"
            title="Security Built-in"
            description="Vulnerability scanning, SBOM generation, secret detection, audit logging."
          />
          <FeatureCard
            icon="👥"
            title="Team Sync"
            description="Lock files, drift detection, environment sharing. No more 'works on my machine'."
          />
          <FeatureCard
            icon="🚀"
            title="Task Runner"
            description="One command runs any project. Auto-detects npm, cargo, make, and more."
          />
          <FeatureCard
            icon="🐳"
            title="Container Ready"
            description="Generate Dockerfiles, run dev shells, build images. Full Docker/Podman support."
          />
        </div>
      </section>

      <section className={styles.cta}>
        <h2>Ready to simplify your workflow?</h2>
        <p>Join thousands of developers who've already made the switch.</p>
        <Link to="/quickstart" className={styles.primaryButton}>
          Start in 5 minutes →
        </Link>
      </section>
    </Layout>
  );
}
