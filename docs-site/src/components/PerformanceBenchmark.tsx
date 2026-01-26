import React, { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import styles from './PerformanceBenchmark.module.css';

interface Benchmark {
  operation: string;
  omg: number;
  competitor: string;
  competitorTime: number;
  speedup: number;
}

const benchmarks: Benchmark[] = [
  {
    operation: 'Package Search',
    omg: 6,
    competitor: 'pacman',
    competitorTime: 133,
    speedup: 22,
  },
  {
    operation: 'Package Info',
    omg: 6.5,
    competitor: 'pacman',
    competitorTime: 138,
    speedup: 21,
  },
  {
    operation: 'List Packages',
    omg: 1.2,
    competitor: 'pacman',
    competitorTime: 14,
    speedup: 12,
  },
  {
    operation: 'Install Node.js',
    omg: 2300,
    competitor: 'nvm',
    competitorTime: 8400,
    speedup: 3.7,
  },
  {
    operation: 'Switch Node Version',
    omg: 89,
    competitor: 'nvm',
    competitorTime: 342,
    speedup: 3.8,
  },
  {
    operation: 'Install Python',
    omg: 1800,
    competitor: 'pyenv',
    competitorTime: 12000,
    speedup: 6.7,
  },
];

function formatTime(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const seconds = (ms / 1000).toFixed(1);
  return `${seconds}s`;
}

interface BenchmarkBarProps {
  benchmark: Benchmark;
  index: number;
}

function BenchmarkBar({ benchmark, index }: BenchmarkBarProps) {
  const [animated, setAnimated] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setAnimated(true), index * 150);
    return () => clearTimeout(timer);
  }, [index]);

  const maxTime = Math.max(benchmark.omg, benchmark.competitorTime);
  const omgWidth = (benchmark.omg / maxTime) * 100;
  const competitorWidth = (benchmark.competitorTime / maxTime) * 100;

  return (
    <div className={styles.benchmarkRow}>
      <div className={styles.operation}>{benchmark.operation}</div>

      <div className={styles.comparison}>
        {/* OMG Bar */}
        <div className={styles.barRow}>
          <div className={styles.label}>OMG</div>
          <div className={styles.barContainer}>
            <motion.div
              className={`${styles.bar} ${styles.barOmg}`}
              initial={{ width: 0 }}
              animate={{ width: animated ? `${omgWidth}%` : 0 }}
              transition={{ duration: 0.8, ease: 'easeOut' }}
            >
              <div className={styles.speedStreakContainer}>
                <div className={styles.speedStreak} />
              </div>
            </motion.div>
          </div>
          <div className={styles.time}>{formatTime(benchmark.omg)}</div>
        </div>

        {/* Competitor Bar */}
        <div className={styles.barRow}>
          <div className={styles.label}>{benchmark.competitor}</div>
          <div className={styles.barContainer}>
            <motion.div
              className={`${styles.bar} ${styles.barCompetitor}`}
              initial={{ width: 0 }}
              animate={{ width: animated ? `${competitorWidth}%` : 0 }}
              transition={{ duration: 0.8, ease: 'easeOut', delay: 0.2 }}
            />
          </div>
          <div className={styles.time}>{formatTime(benchmark.competitorTime)}</div>
        </div>
      </div>

      {/* Speedup Badge */}
      <motion.div
        className={styles.speedupBadge}
        initial={{ scale: 0, rotate: -15 }}
        animate={{ scale: animated ? 1 : 0, rotate: animated ? 0 : -15 }}
        transition={{ duration: 0.5, delay: 0.6, type: 'spring' }}
      >
        <span className={styles.speedupValue}>{benchmark.speedup}×</span>
        <span className={styles.speedupLabel}>FASTER</span>
      </motion.div>
    </div>
  );
}

export default function PerformanceBenchmark() {
  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <h3 className={styles.title}>⚡ Performance Benchmarks</h3>
        <p className={styles.subtitle}>
          Real-world performance comparisons on production systems
        </p>
      </div>

      <div className={styles.benchmarks}>
        {benchmarks.map((benchmark, index) => (
          <BenchmarkBar key={benchmark.operation} benchmark={benchmark} index={index} />
        ))}
      </div>

      <div className={styles.footer}>
        <div className={styles.footerNote}>
          <span className={styles.checkeredFlag}>🏁</span>
          Benchmarked on: AMD Ryzen 9 5950X, 64GB RAM, NVMe SSD
        </div>
        <div className={styles.footerNote}>
          <span className={styles.stopwatch}>⏱️</span>
          Results averaged over 100 runs with warm cache
        </div>
      </div>
    </div>
  );
}
