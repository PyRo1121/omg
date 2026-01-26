import React, { useEffect, useState, useRef } from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import { motion, AnimatePresence, useInView } from 'framer-motion';
import { 
  Zap, 
  Shield, 
  Users, 
  Terminal, 
  Package, 
  Layers, 
  ChevronRight, 
  Cpu,
  Globe,
  Clock
} from 'lucide-react';
import styles from './index.module.css';

function SpeedMetric({ value, label, suffix = '' }: { value: string; label: string; suffix?: string }) {
  const ref = useRef(null);
  const isInView = useInView(ref, { once: true });
  const [count, setCount] = useState(0);

  useEffect(() => {
    if (isInView) {
      const target = parseFloat(value);
      if (isNaN(target)) return;
      
      let start = 0;
      const duration = 2000;
      const startTime = performance.now();

      const animate = (currentTime: number) => {
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1);
        const easeOutExpo = 1 - Math.pow(2, -10 * progress);
        const currentCount = easeOutExpo * target;
        
        setCount(currentCount);

        if (progress < 1) {
          requestAnimationFrame(animate);
        } else {
          setCount(target);
        }
      };

      requestAnimationFrame(animate);
    }
  }, [isInView, value]);

  return (
    <div className={styles.metric} ref={ref}>
      <motion.span 
        className={styles.metricValue}
        initial={{ opacity: 0, y: 20 }}
        animate={isInView ? { opacity: 1, y: 0 } : {}}
      >
        {value.includes('.') ? count.toFixed(1) : Math.floor(count)}
        <span className={styles.metricSuffix}>{suffix}</span>
      </motion.span>
      <span className={styles.metricLabel}>{label}</span>
    </div>
  );
}

function TerminalDemo() {
  const [displayedLines, setDisplayedLines] = useState<{ type: 'cmd' | 'out'; text: string }[]>([]);
  const [currentText, setCurrentText] = useState('');
  const [currentStep, setCurrentStep] = useState(0);
  const [isTyping, setIsTyping] = useState(true);

  const scenario = [
    { type: 'cmd', text: 'omg search neovim' },
    { type: 'out', text: '✓ 47 packages found in 6ms' },
    { type: 'cmd', text: 'omg use node 22' },
    { type: 'out', text: '✓ Node.js 22.0.0 activated' },
    { type: 'cmd', text: 'omg run dev' },
    { type: 'out', text: '→ Detected package.json, using bun' },
  ];

  useEffect(() => {
    let timeout: NodeJS.Timeout;

    const runScenario = async () => {
      const step = scenario[currentStep];
      
      if (step.type === 'cmd') {
        setIsTyping(true);
        let i = 0;
        const typeChar = () => {
          if (i < step.text.length) {
            setCurrentText(step.text.slice(0, i + 1));
            i++;
            timeout = setTimeout(typeChar, 40 + Math.random() * 40);
          } else {
            timeout = setTimeout(() => {
              setDisplayedLines(prev => [...prev, { type: 'cmd', text: step.text }]);
              setCurrentText('');
              setIsTyping(false);
              proceed();
            }, 600);
          }
        };
        typeChar();
      } else {
        timeout = setTimeout(() => {
          setDisplayedLines(prev => [...prev, { type: 'out', text: step.text }]);
          proceed();
        }, 400);
      }
    };

    const proceed = () => {
      if (currentStep < scenario.length - 1) {
        setCurrentStep(s => s + 1);
      } else {
        timeout = setTimeout(() => {
          setDisplayedLines([]);
          setCurrentStep(0);
        }, 3000);
      }
    };

    runScenario();
    return () => clearTimeout(timeout);
  }, [currentStep]);

  return (
    <div className={styles.terminal}>
      <div className={styles.terminalHeader}>
        <div className={styles.terminalDots}>
          <span></span><span></span><span></span>
        </div>
        <div className={styles.terminalTitle}>
          <Terminal size={14} style={{ marginRight: 6, verticalAlign: 'middle' }} />
          omg — zsh
        </div>
      </div>
      <div className={styles.terminalBody}>
        {displayedLines.map((line, i) => (
          <div key={i} className={line.type === 'cmd' ? styles.terminalCommand : styles.terminalOutput}>
            {line.type === 'cmd' && <span className={styles.prompt}>❯</span>}
            {line.text}
          </div>
        ))}
        {isTyping && (
          <div className={styles.terminalCommand}>
            <span className={styles.prompt}>❯</span>
            {currentText}
            <span className={styles.cursor}></span>
          </div>
        )}
        {!isTyping && currentStep !== scenario.length - 1 && displayedLines.length > 0 && (
           <div className={styles.terminalCommand}>
            <span className={styles.prompt}>❯</span>
            <span className={styles.cursor}></span>
          </div>
        )}
      </div>
    </div>
  );
}

function FeatureCard({ icon: Icon, title, description, delay }: { icon: any; title: string; description: string; delay: number }) {
  return (
    <motion.div 
      className={styles.featureCard}
      initial={{ opacity: 0, y: 20 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ delay }}
    >
      <div className={styles.featureIcon}>
        <Icon size={32} strokeWidth={2} color="#FFED4E" />
      </div>
      <h3>{title}</h3>
      <p>{description}</p>
    </motion.div>
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
      title="OMG - The Fastest Unified Package Manager"
      description="The fastest unified package manager for Arch Linux and all language runtimes. One tool for everything."
    >
      <div className={styles.hero}>
        <div className={styles.heroGrid}></div>
        <div className={styles.heroContent}>
          <motion.div 
            className={styles.heroText}
            initial={{ opacity: 0, x: -30 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.6 }}
          >
            <motion.div 
              className={styles.badge}
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ delay: 0.2 }}
            >
              <Zap size={14} style={{ marginRight: 8 }} />
              22x faster than pacman
            </motion.div>
            <motion.h1
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.3 }}
            >
              One tool.<br />Every package.<br />All runtimes.
            </motion.h1>
            <motion.p 
              className={styles.heroSubtitle}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.4 }}
            >
              Stop juggling pacman, yay, nvm, pyenv, and rustup.<br />
              OMG unifies your entire dev stack into one blazing-fast CLI.
            </motion.p>
            <motion.div 
              className={styles.heroActions}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.5 }}
            >
              <Link to="/quickstart" className={styles.primaryButton}>
                Get Started <ChevronRight size={20} style={{ marginLeft: 8 }} />
              </Link>
              <Link to="/cli" className={styles.secondaryButton}>
                CLI Reference
              </Link>
            </motion.div>
            <motion.div 
              className={styles.installCommand}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ delay: 0.7 }}
            >
              <code>curl -fsSL https://pyro1121.com/install.sh | bash</code>
            </motion.div>
          </motion.div>
          
          <motion.div 
            className={styles.heroVisual}
            initial={{ opacity: 0, scale: 0.95, x: 30 }}
            animate={{ opacity: 1, scale: 1, x: 0 }}
            transition={{ duration: 0.8, ease: "easeOut" }}
          >
            <TerminalDemo />
          </motion.div>
        </div>
      </div>

      <section className={styles.metrics}>
        <SpeedMetric value="6" suffix="ms" label="Package search" />
        <SpeedMetric value="22" suffix="x" label="Faster than pacman" />
        <SpeedMetric value="7" suffix="+" label="Language runtimes" />
        <SpeedMetric value="1.2" suffix="ms" label="Daemon latency" />
      </section>

      {/* PROGRESSIVE DISCLOSURE: Quick Start First */}
      <section className={styles.quickStart}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
        >
          <h2>Get Started in 60 Seconds</h2>
          <p className={styles.sectionSubtitle}>Three commands. That's all it takes.</p>

          <div className={styles.quickStartGrid}>
            <div className={styles.quickStartStep}>
              <div className={styles.stepNumber}>1</div>
              <h3>Install OMG</h3>
              <div className={styles.codeBlock}>
                <code>curl -fsSL https://pyro1121.com/install.sh | bash</code>
              </div>
            </div>

            <div className={styles.quickStartStep}>
              <div className={styles.stepNumber}>2</div>
              <h3>Add Shell Hook</h3>
              <div className={styles.codeBlock}>
                <code>eval "$(omg hook zsh)"</code>
              </div>
            </div>

            <div className={styles.quickStartStep}>
              <div className={styles.stepNumber}>3</div>
              <h3>Install Your First Package</h3>
              <div className={styles.codeBlock}>
                <code>omg install ripgrep</code>
              </div>
            </div>
          </div>

          <div style={{ textAlign: 'center', marginTop: '3rem' }}>
            <Link to="/quickstart" className={styles.secondaryButton}>
              Full Installation Guide <ChevronRight size={20} style={{ marginLeft: 8 }} />
            </Link>
          </div>
        </motion.div>
      </section>

      <section className={styles.comparison}>
        <motion.h2
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
        >
          Raw Performance
        </motion.h2>
        <motion.p 
          className={styles.sectionSubtitle}
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ delay: 0.2 }}
        >
          Measured on real hardware, not benchmarketing
        </motion.p>
        <div className={styles.comparisonChart}>
          <ComparisonBar tool="OMG" time={6} maxTime={150} />
          <ComparisonBar tool="pacman" time={133} maxTime={150} />
          <ComparisonBar tool="yay" time={145} maxTime={150} />
        </div>
        <p className={styles.comparisonNote}>Package search latency (lower is better)</p>
      </section>

      <section className={styles.features}>
        <motion.h2
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
        >
          Technical Precision
        </motion.h2>
        <motion.p 
          className={styles.sectionSubtitle}
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ delay: 0.2 }}
        >
          Built for engineers who value speed and reliability
        </motion.p>
        <div className={styles.featureGrid}>
          <FeatureCard
            icon={Package}
            title="System Packages"
            description="Direct libalpm integration. Repos + AUR with sub-10ms search latency."
            delay={0.1}
          />
          <FeatureCard
            icon={Cpu}
            title="Unified Runtimes"
            description="Native management for Node, Python, Rust, Go, Ruby, Java, and Bun."
            delay={0.2}
          />
          <FeatureCard
            icon={Shield}
            title="Security First"
            description="Vulnerability scanning, SBOM generation, and PGP verification built-in."
            delay={0.3}
          />
          <FeatureCard
            icon={Users}
            title="Team Sync"
            description="Lock files and environment sharing to ensure consistency across the team."
            delay={0.4}
          />
          <FeatureCard
            icon={Layers}
            title="Task Runner"
            description="Auto-detects project types and runs tasks with correct runtime versions."
            delay={0.5}
          />
          <FeatureCard
            icon={Globe}
            title="Cloud Native"
            description="Generate Dockerfiles and OCI images from your local environment instantly."
            delay={0.6}
          />
        </div>
      </section>

      <motion.section 
        className={styles.cta}
        initial={{ opacity: 0 }}
        whileInView={{ opacity: 1 }}
        viewport={{ once: true }}
      >
        <h2>Ready to simplify your workflow?</h2>
        <p>Join thousands of developers who've already made the switch.</p>
        <Link to="/quickstart" className={styles.primaryButton}>
          Get Started for Free <ChevronRight size={20} style={{ marginLeft: 8 }} />
        </Link>
      </motion.section>
    </Layout>
  );
}
