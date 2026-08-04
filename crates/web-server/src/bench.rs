//! No-HTTP benchmark for the read path (WP-01 step 6).
//!
//! Ignored by default; it measures wall-clock time, which must never gate CI.
//! The deterministic regression guards are the counter assertions in
//! `read_model::tests` and `kanban_core::repository::read_path_budget_tests`.
//!
//! Run with:
//!
//! ```sh
//! cargo test -p kanban-web-server --release -- --ignored --nocapture read_path_bench
//! ```

use std::time::{Duration, Instant};

use kanban_core::testsupport::{BacklogFixture, FixtureSpec, generate_backlog_fixture};

use crate::read_model::WebReadModel;

const RUNS: usize = 20;

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let index = (((sorted.len() - 1) as f64) * fraction).round() as usize;
    sorted[index]
}

fn measure(label: &str, runs: usize, mut operation: impl FnMut()) {
    // One warmup so page cache state is comparable across labels.
    operation();
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed());
    }
    samples.sort();
    println!(
        "{label:<44} n={runs:<3} min={:>8.2?} median={:>8.2?} p95={:>8.2?} max={:>8.2?}",
        samples[0],
        percentile(&samples, 0.5),
        percentile(&samples, 0.95),
        samples[samples.len() - 1],
    );
}

fn bench_fixture(name: &str, fixture: &BacklogFixture) {
    let root = fixture.root().to_path_buf();
    println!("\n-- {name}: {:?}", fixture.spec());

    measure("read-model build (B5 <= 250 ms p95)", RUNS, || {
        let model = WebReadModel::build(&root).expect("build read model");
        std::hint::black_box(&model.snapshot.stories.len());
    });

    let model = WebReadModel::build(&root).unwrap();
    measure("  derive metrics from built model", RUNS, || {
        std::hint::black_box(model.metrics());
    });
    measure("  derive report from built model", RUNS, || {
        std::hint::black_box(model.report());
    });
    measure("  serialize repository snapshot", RUNS, || {
        let body = serde_json::to_vec(&model.snapshot).unwrap();
        std::hint::black_box(body.len());
    });
    println!(
        "  repository snapshot body: {} bytes",
        serde_json::to_vec(&model.snapshot).unwrap().len()
    );

    measure("kanban doctor (B6 <= 500 ms)", RUNS.min(10), || {
        std::hint::black_box(kanban_core::doctor_repository(&root).expect("doctor"));
    });
    measure("kanban validate", RUNS.min(10), || {
        std::hint::black_box(kanban_core::validate_repository(&root).expect("validate"));
    });
}

#[test]
#[ignore = "wall-clock benchmark; run explicitly with --ignored --nocapture"]
fn read_path_bench() {
    bench_fixture(
        "representative (250 stories, 30 epics, 5 sprints)",
        &generate_backlog_fixture(&FixtureSpec::representative()),
    );
    bench_fixture(
        "minimal (sprints disabled)",
        &generate_backlog_fixture(&FixtureSpec::minimal()),
    );
}

/// Materialize a benchmark fixture on disk and print its path, so the HTTP
/// harness (`scripts/benchmark_web_load.py`) and manual Chrome traces can run
/// `kanban web serve --repo-root <path>` against a reproducible repository.
///
/// The directory is intentionally leaked; delete it when finished.
///
/// ```sh
/// cargo test -p kanban-web-server --release -- --ignored --nocapture materialize_fixture
/// ```
#[test]
#[ignore = "creates a directory the caller must clean up"]
fn materialize_fixture() {
    let spec = match std::env::var("KANBAN_FIXTURE").as_deref() {
        Ok("minimal") => FixtureSpec::minimal(),
        _ => FixtureSpec::representative(),
    };
    let fixture = generate_backlog_fixture(&spec);
    println!("KANBAN_FIXTURE_ROOT={}", fixture.keep().display());
}
