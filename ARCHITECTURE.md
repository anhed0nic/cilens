# Architecture

CILens is a CLI tool for collecting and analyzing CI/CD pipeline insights. This document explains the high-level design.

## Design Philosophy

### Core Principles

1. **Simplicity first** - Search for ways to make things simple while generating maximum value. Avoid over-engineering.
2. **Unit tests win** - Unit tests are faster, simpler, and more reproducible than integration tests. Prefer them.
3. **Minimum configurability** - Correctly opinionated tool > very configurable tool. Make good default choices.
4. **Simple tools that work** - cargo-dist for releases, nix flakes for reproducible builds. No complex CI/CD pipelines.
5. **Strictest linting** - `cargo lint` = pedantic clippy. Catch issues early.

### Domain Principles

1. **Percentiles over averages** - P50/P95/P99 give realistic expectations; averages hide outliers
2. **Time-to-feedback matters** - Developers care about "when will I get results", not just "how long did the job run"
3. **Detect flakiness automatically** - Track retries and intermittent failures
4. **Optimize the critical path** - Show job dependencies to identify blockers
5. **Cache aggressively** - Completed pipelines don't change; cache them

## Module Structure

```text
cilens/
├── cli.rs              # Command-line interface (clap)
├── auth.rs             # Token wrapper with secure Debug impl
├── error.rs            # Error types (thiserror)
├── insights.rs         # Domain model (CIInsights, JobMetrics, etc.)
├── output/             # Display layer
│   ├── summary.rs      # Human-readable tables
│   ├── progress.rs     # 3-phase progress spinner
│   ├── tables.rs       # Color-coded table helpers
│   └── styling.rs      # Terminal styling functions
└── providers/
    └── gitlab/
        ├── provider.rs         # Main entry point
        ├── client/             # GraphQL API client
        ├── pipeline_types.rs   # Group pipelines by job signature
        ├── pipeline_metrics.rs # Calculate P50/P95/P99 for pipeline types
        ├── job_metrics.rs      # Calculate time-to-feedback per job
        ├── job_reliability.rs  # Track failures and flakiness
        ├── cache.rs            # Persistent job cache
        └── types.rs            # GitLab-specific data models
```

## Data Flow

```text
1. CLI parses arguments
   └─> GitLabProvider.collect_insights()

2. Fetch pipelines (GraphQL)
   ├─> Check cache for job data
   ├─> Fetch missing jobs (GraphQL, batched)
   └─> Save to cache

3. Transform GitLab data → Domain model
   ├─> Group pipelines by job signature (pipeline_types.rs)
   ├─> Calculate pipeline metrics (pipeline_metrics.rs)
   │   └─> Calculate job metrics (job_metrics.rs)
   │   └─> Calculate reliability (job_reliability.rs)
   └─> Return CIInsights

4. Display results
   ├─> JSON output (--json)
   └─> Human-readable summary (output/summary.rs)
```

## Key Design Decisions

### 1. Percentiles (P50/P95/P99)

**Why:** Averages are misleading for skewed distributions. A job that takes 5min 99% of the time but 60min 1% of the time has a 5.5min average (useless for planning).

**Where:** `pipeline_metrics.rs::calculate_percentiles()`

### 2. Time-to-Feedback vs Duration

**Why:** Developers care about "when do I get feedback" more than "how long did the job run". A 2-minute job that waits 10 minutes for dependencies has 12min time-to-feedback.

**Where:** `job_metrics.rs::calculate_finish_time()` - recursively calculates when each job completes based on dependencies.

### 3. Job Signature Grouping

**Why:** Pipelines with the same set of jobs are the same "type" (e.g., all "Production" pipelines run the same jobs). Group them to get meaningful statistics.

**Where:** `pipeline_types.rs::group_pipeline_types()` - groups by sorted job names, filters by minimum percentage threshold.

### 4. Flakiness Detection

**Why:** Intermittent failures waste CI resources. Jobs that fail then succeed on retry are "flaky" and need fixing.

**Where:** `job_reliability.rs::calculate_job_reliability()` - tracks failed-then-retried vs failed-and-stayed-failed.

### 5. Smart Caching

**Why:** Completed pipelines don't change. Fetching jobs is expensive (1 API call per pipeline). Cache reduces 500 pipelines from ~500 API calls to ~5-10 on subsequent runs.

**Where:** `cache.rs` - per-project JSON cache in platform-specific cache directory.

**Design:**

- Cache key: pipeline ID
- Cache value: job data
- Immutable: loaded at startup, written on completion
- Only cache "success" and "failed" (not "running" or "canceled")

### 6. Deterministic Sampling

**Why:** When fetching 500 pipelines, we want a balanced sample (not just 500 most recent failures). GitLab API returns most recent first.

**Where:** `client/pipelines.rs::fetch_pipelines()` - fetch 50% SUCCESS, 50% FAILED to get representative sample.

## Extension Points

### Adding a New Provider (e.g., GitHub Actions)

1. Create `providers/github/`
2. Implement data fetching (REST/GraphQL)
3. Transform to `CIInsights` domain model
4. Add CLI subcommand `cilens github ...`

**Key:** The `insights.rs` domain model is provider-agnostic. New providers just need to produce `CIInsights`.

### Adding New Metrics

1. Add fields to `insights.rs` (e.g., `cost_per_pipeline: f64`)
2. Calculate in `pipeline_metrics.rs` or `job_metrics.rs`
3. Display in `output/summary.rs`

### Adding Export Formats

1. Domain model already has `#[derive(Serialize)]`
2. Add format in `cli.rs::execute_gitlab()`:
   - CSV: serialize to CSV
   - HTML: template engine
   - Prometheus: `/metrics` endpoint

## Performance Characteristics

- **First run:** ~30-60 seconds for 500 pipelines (network bound)
- **Cached run:** ~5 seconds for 500 pipelines (90%+ cache hit rate)
- **Concurrency:** Max 500 parallel requests (configurable)
- **Retry logic:** Up to 30 retries with 10s delay (handles rate limits)
- **Memory:** ~50-100MB peak (all data in memory during processing)

## Testing Strategy

- **Unit tests:** Inline with `#[cfg(test)]` (181 tests)
- **Test fixtures:** Helper functions in each test module
