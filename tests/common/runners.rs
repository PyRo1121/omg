//! Test runners for parallel and sequential test execution

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::CommandResult;

/// Test result with metadata
#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration: Duration,
    pub error: Option<String>,
    pub output: Option<CommandResult>,
}

/// Test suite runner with parallel execution support
pub struct TestRunner {
    results: Arc<Mutex<Vec<TestResult>>>,
    parallel: bool,
    max_threads: usize,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::new())),
            parallel: true,
            max_threads: num_cpus::get().min(8),
        }
    }

    pub fn sequential() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::new())),
            parallel: false,
            max_threads: 1,
        }
    }

    /// Run a single test
    pub fn run_test<F>(&self, name: &str, test_fn: F)
    where
        F: FnOnce() -> Result<(), String> + Send + 'static,
    {
        let start = Instant::now();
        let result = test_fn();
        let duration = start.elapsed();

        let test_result = TestResult {
            name: name.to_string(),
            passed: result.is_ok(),
            duration,
            error: result.err(),
            output: None,
        };

        self.results.lock().unwrap().push(test_result);
    }

    /// Run multiple tests in parallel
    pub fn run_parallel<F>(&self, tests: Vec<(&str, F)>)
    where
        F: FnOnce() -> Result<(), String> + Send + 'static,
    {
        let mut handles = Vec::new();
        let results = Arc::clone(&self.results);

        for (name, test_fn) in tests {
            let name = name.to_string();
            let results = Arc::clone(&results);

            let handle = thread::spawn(move || {
                let start = Instant::now();
                let result = test_fn();
                let duration = start.elapsed();

                let test_result = TestResult {
                    name,
                    passed: result.is_ok(),
                    duration,
                    error: result.err(),
                    output: None,
                };

                results.lock().unwrap().push(test_result);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Get all results
    pub fn results(&self) -> Vec<TestResult> {
        self.results.lock().unwrap().clone()
    }

    /// Get summary statistics
    pub fn summary(&self) -> TestSummary {
        let results = self.results.lock().unwrap();
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let total_duration: Duration = results.iter().map(|r| r.duration).sum();

        TestSummary {
            total,
            passed,
            failed,
            total_duration,
        }
    }

    /// Print results to stdout
    pub fn print_results(&self) {
        let results = self.results.lock().unwrap();

        println!("\n═══════════════════════════════════════════════════════════════");
        println!("                        TEST RESULTS                            ");
        println!("═══════════════════════════════════════════════════════════════\n");

        for result in results.iter() {
            let status = if result.passed {
                "✅ PASS"
            } else {
                "❌ FAIL"
            };
            println!("{status} {} ({:?})", result.name, result.duration);
            if let Some(ref err) = result.error {
                println!("       Error: {err}");
            }
        }

        let summary = self.summary();
        println!("\n───────────────────────────────────────────────────────────────");
        println!(
            "Total: {} | Passed: {} | Failed: {} | Duration: {:?}",
            summary.total, summary.passed, summary.failed, summary.total_duration
        );
        println!("───────────────────────────────────────────────────────────────\n");
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub total_duration: Duration,
}

/// Stress test runner for load testing
pub struct StressRunner {
    iterations: usize,
    concurrency: usize,
    results: Arc<Mutex<Vec<Duration>>>,
}

impl StressRunner {
    pub fn new(iterations: usize, concurrency: usize) -> Self {
        Self {
            iterations,
            concurrency,
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Run a stress test
    pub fn run<F>(&self, test_fn: F) -> StressResults
    where
        F: Fn() -> Duration + Send + Sync + Clone + 'static,
    {
        let mut handles = Vec::new();
        let iterations_per_thread = self.iterations / self.concurrency;

        for _ in 0..self.concurrency {
            let results = Arc::clone(&self.results);
            let test_fn = test_fn.clone();
            let iters = iterations_per_thread;

            let handle = thread::spawn(move || {
                for _ in 0..iters {
                    let duration = test_fn();
                    results.lock().unwrap().push(duration);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        self.compute_results()
    }

    fn compute_results(&self) -> StressResults {
        let results = self.results.lock().unwrap();
        let mut durations: Vec<_> = results.iter().copied().collect();
        durations.sort();

        let total: Duration = durations.iter().sum();
        let count = durations.len();
        let avg = total / u32::try_from(count).unwrap_or(1);

        let min = durations.first().copied().unwrap_or_default();
        let max = durations.last().copied().unwrap_or_default();

        let p50 = durations.get(count * 50 / 100).copied().unwrap_or_default();
        let p95 = durations.get(count * 95 / 100).copied().unwrap_or_default();
        let p99 = durations.get(count * 99 / 100).copied().unwrap_or_default();

        StressResults {
            iterations: count,
            min,
            max,
            avg,
            p50,
            p95,
            p99,
        }
    }
}

#[derive(Debug)]
pub struct StressResults {
    pub iterations: usize,
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

impl StressResults {
    pub fn print(&self) {
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("                      STRESS TEST RESULTS                       ");
        println!("═══════════════════════════════════════════════════════════════\n");
        println!("  Iterations: {}", self.iterations);
        println!("  Min:        {:?}", self.min);
        println!("  Max:        {:?}", self.max);
        println!("  Avg:        {:?}", self.avg);
        println!("  P50:        {:?}", self.p50);
        println!("  P95:        {:?}", self.p95);
        println!("  P99:        {:?}", self.p99);
        println!("\n───────────────────────────────────────────────────────────────\n");
    }

    pub fn assert_p99_under(&self, max: Duration) {
        assert!(
            self.p99 < max,
            "P99 latency {:?} exceeds maximum {:?}",
            self.p99,
            max
        );
    }

    pub fn assert_avg_under(&self, max: Duration) {
        assert!(
            self.avg < max,
            "Average latency {:?} exceeds maximum {:?}",
            self.avg,
            max
        );
    }
}

/// Benchmark runner for performance testing
pub struct BenchmarkRunner {
    warmup_iterations: usize,
    measure_iterations: usize,
}

impl BenchmarkRunner {
    pub fn new(warmup: usize, measure: usize) -> Self {
        Self {
            warmup_iterations: warmup,
            measure_iterations: measure,
        }
    }

    pub fn run<F>(&self, name: &str, mut test_fn: F) -> BenchmarkResult
    where
        F: FnMut() -> Duration,
    {
        // Warmup
        for _ in 0..self.warmup_iterations {
            test_fn();
        }

        // Measure
        let mut durations = Vec::with_capacity(self.measure_iterations);
        for _ in 0..self.measure_iterations {
            durations.push(test_fn());
        }

        durations.sort();
        let total: Duration = durations.iter().sum();
        let count = durations.len();
        let avg = total / u32::try_from(count).unwrap_or(1);
        let min = durations.first().copied().unwrap_or_default();
        let max = durations.last().copied().unwrap_or_default();

        BenchmarkResult {
            name: name.to_string(),
            iterations: count,
            min,
            max,
            avg,
        }
    }
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self::new(5, 20)
    }
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
}

impl BenchmarkResult {
    pub fn print(&self) {
        println!(
            "  {} - min: {:?}, max: {:?}, avg: {:?} ({} iterations)",
            self.name, self.min, self.max, self.avg, self.iterations
        );
    }
}

// Dummy num_cpus implementation for test context
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4)
    }
}
