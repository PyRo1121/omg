import React from 'react';
import { motion } from 'framer-motion';
import styles from './CommandComparison.module.css';

interface Comparison {
  category: string;
  oldTool: string;
  oldCommand: string;
  omgCommand: string;
  benefit: string;
  icon: string;
}

const comparisons: Comparison[] = [
  {
    category: 'Package Management',
    oldTool: 'yay',
    oldCommand: 'yay -S firefox',
    omgCommand: 'omg install firefox',
    benefit: '22× faster search',
    icon: '📦',
  },
  {
    category: 'Package Management',
    oldTool: 'pacman',
    oldCommand: 'pacman -Ss firefox',
    omgCommand: 'omg search firefox',
    benefit: 'Unified interface',
    icon: '🔍',
  },
  {
    category: 'Node.js',
    oldTool: 'nvm',
    oldCommand: 'nvm install 20 && nvm use 20',
    omgCommand: 'omg use node 20',
    benefit: 'Single command',
    icon: '⚡',
  },
  {
    category: 'Node.js',
    oldTool: 'nvm',
    oldCommand: 'nvm current',
    omgCommand: 'omg status',
    benefit: 'See all runtimes',
    icon: '📊',
  },
  {
    category: 'Python',
    oldTool: 'pyenv',
    oldCommand: 'pyenv install 3.12 && pyenv global 3.12',
    omgCommand: 'omg use python 3.12',
    benefit: 'Single command',
    icon: '🐍',
  },
  {
    category: 'Rust',
    oldTool: 'rustup',
    oldCommand: 'rustup toolchain install stable && rustup default stable',
    omgCommand: 'omg use rust stable',
    benefit: 'Consistent syntax',
    icon: '🦀',
  },
  {
    category: 'Task Running',
    oldTool: 'npm',
    oldCommand: 'npm run dev',
    omgCommand: 'omg run dev',
    benefit: 'Runtime agnostic',
    icon: '🎯',
  },
  {
    category: 'Updates',
    oldTool: 'yay',
    oldCommand: 'yay -Syu',
    omgCommand: 'omg update',
    benefit: 'All packages + runtimes',
    icon: '🔄',
  },
];

interface ComparisonCardProps {
  comparison: Comparison;
  index: number;
}

function ComparisonCard({ comparison, index }: ComparisonCardProps) {
  return (
    <motion.div
      className={styles.card}
      initial={{ opacity: 0, y: 20 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ duration: 0.5, delay: index * 0.1 }}
    >
      <div className={styles.cardHeader}>
        <span className={styles.icon}>{comparison.icon}</span>
        <div>
          <div className={styles.category}>{comparison.category}</div>
          <div className={styles.oldTool}>Migrating from {comparison.oldTool}</div>
        </div>
      </div>

      <div className={styles.commandComparison}>
        <div className={styles.oldCommand}>
          <div className={styles.commandLabel}>
            <span className={styles.labelText}>Old Way</span>
            <span className={styles.toolBadge}>{comparison.oldTool}</span>
          </div>
          <div className={styles.commandCode}>
            <span className={styles.promptSymbol}>$</span>
            <code>{comparison.oldCommand}</code>
          </div>
        </div>

        <div className={styles.arrow}>
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
            <path
              d="M5 12h14m0 0l-6-6m6 6l-6 6"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>

        <div className={styles.newCommand}>
          <div className={styles.commandLabel}>
            <span className={styles.labelText}>OMG Way</span>
            <span className={styles.benefitBadge}>{comparison.benefit}</span>
          </div>
          <div className={`${styles.commandCode} ${styles.highlight}`}>
            <span className={styles.promptSymbol}>$</span>
            <code>{comparison.omgCommand}</code>
          </div>
        </div>
      </div>
    </motion.div>
  );
}

export default function CommandComparison() {
  // Group by category
  const grouped = comparisons.reduce((acc, comp) => {
    if (!acc[comp.category]) acc[comp.category] = [];
    acc[comp.category].push(comp);
    return acc;
  }, {} as Record<string, Comparison[]>);

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <h3 className={styles.title}>🔄 Command Migration Guide</h3>
        <p className={styles.subtitle}>
          Quick reference for switching from your current tools to OMG
        </p>
      </div>

      {Object.entries(grouped).map(([category, items], categoryIndex) => (
        <div key={category} className={styles.categorySection}>
          <h4 className={styles.categoryTitle}>{category}</h4>
          <div className={styles.grid}>
            {items.map((comparison, index) => (
              <ComparisonCard
                key={`${comparison.oldTool}-${comparison.oldCommand}`}
                comparison={comparison}
                index={categoryIndex * items.length + index}
              />
            ))}
          </div>
        </div>
      ))}

      <div className={styles.footer}>
        <div className={styles.footerCard}>
          <div className={styles.footerIcon}>💡</div>
          <div>
            <div className={styles.footerTitle}>Not Listed?</div>
            <div className={styles.footerText}>
              Most tools follow the pattern: <code>omg &lt;action&gt; &lt;target&gt;</code>
              <br />
              Try <code>omg help</code> or check the full CLI reference.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
