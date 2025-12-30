# 🔍 CILens - CI/CD Insights Tool

A Rust CLI tool for collecting and analyzing CI/CD insights from GitLab.

## ✨ Features

- **🧩 Smart Pipeline Clustering** - Groups pipelines by job signature and filters out rare pipeline types (configurable threshold, default 1%)
- **📊 Duration Percentiles (P50, P95, P99)** - Realistic performance expectations showing typical, planning, and worst-case scenarios instead of misleading averages
- **⏱️ Per-Job Time-to-Feedback** - Shows how long each job takes to complete from pipeline start, revealing actual developer wait times
- **🔍 Dependency Tracking** - Identifies which jobs block others, showing the critical path to each job
- **⚠️ Flakiness Detection** - Identifies unreliable jobs that fail intermittently and need retries
- **✅ Success Rate Metrics** - Per-pipeline-type success rates and failure analysis
- **🎯 Optimization Insights** - Jobs sorted by P95 time-to-feedback to quickly identify highest-impact optimization targets

## 📦 Installation

### Installer Script

Install the latest version for your platform:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/dsalaza4/cilens/releases/download/v0.6.0/cilens-installer.sh | sh
```

### Nix

Install using Nix flakes:

```bash
nix profile install github:dsalaza4/cilens/v0.6.0
```

Or run without installing:

```bash
nix run github:dsalaza4/cilens/v0.6.0 -- --help
```

## 🚀 Quick Start

```bash
# Get your GitLab token from: https://gitlab.com/-/profile/personal_access_tokens
# Required scope: read_api

export GITLAB_TOKEN="glpat-your-token"

cilens gitlab group/project
```

## 💡 Usage

```bash
# Default: Human-readable summary (displays top issues, optimization targets)
cilens gitlab your/project

# Get JSON output for programmatic analysis
cilens gitlab your/project --json > insights.json

# Pretty-printed JSON
cilens gitlab your/project --json --pretty > insights.json

# Fetch fewer pipelines for faster analysis
cilens gitlab your/project --limit 100

# Filter by date range
cilens gitlab your/project --since 2025-01-01 --until 2025-01-31

# Filter by branch/ref
cilens gitlab your/project --ref main

# Self-hosted GitLab
cilens gitlab your/project --base-url "https://gitlab.example.com"

# Custom filtering threshold (only show pipeline types that are ≥5% of total)
cilens gitlab your/project --min-type-percentage 5
```

### 📅 Date Filtering

CILens fetches the most recent pipelines up to the specified limit (default: 500). You can optionally filter by date:

- `--since YYYY-MM-DD`: Start date (optional)
- `--until YYYY-MM-DD`: End date (optional)
- `--limit N`: Maximum pipelines to fetch (default: 500)

**Important:** Date filtering is done server-side by GitLab's API. On very large projects with thousands of pipelines, date-filtered queries may timeout due to GitLab API limitations. If this occurs, use `--limit` instead of relying on date filters.

### 🔄 Reliability & Performance

CILens is designed to handle large-scale pipeline fetches reliably:

- **Automatic Retry**: Network errors, rate limits (429), and server errors (5xx) are automatically retried up to 30 times with 10-second delays
- **Concurrency Limiting**: Maximum 500 concurrent requests to prevent overwhelming GitLab's API
- **Graceful Degradation**: Transient failures are logged and retried transparently

This makes it suitable for fetching thousands of pipelines even from busy GitLab instances.

### ⚡ Caching

CILens automatically caches job data for completed pipelines to dramatically speed up subsequent runs on the same project:

- **90%+ Speedup**: Second runs are typically 10x faster since job data is cached locally
- **Smart Caching**: Only caches completed pipelines (SUCCESS/FAILED status) since their data is immutable
- **Per-Project Cache Files**: Each project gets its own cache file (e.g., `group-project.json`) loaded into memory at startup for fast lookups
- **Platform-Aware**: Uses platform-specific cache locations:
  - Linux: `~/.cache/cilens/gitlab/`
  - macOS: `~/Library/Caches/cilens/gitlab/`
- **Transparent**: Automatically checks cache before making API calls - no configuration needed
- **Validated**: Cache entries are validated against pipeline ID and status to prevent stale data

#### Cache Management

```bash
# Clear cache before running
cilens gitlab your/project --clear-cache

# Disable cache for a single run
cilens gitlab your/project --no-cache
```

**When to clear cache**: Clear cache when you need fresh data after pipeline definitions change significantly, or periodically to reclaim disk space.

## 📄 Output Formats

CILens provides two output formats to suit different use cases:

### 📊 Summary Output (Default)

By default, CILens displays a human-readable summary with actionable insights.
The summary includes:

**Overview Section:**

- Pipelines analyzed
- Jobs analyzed (total job executions)
- Overall success rate (color-coded: green >80%, yellow 50-80%, red <50%)
- Pipeline types count

**Analysis Tables:**

- **Pipeline Types**: Overview of all pipeline types with percentage distribution, success rate, P95 duration, slowest job (name + feedback time), and example pipeline URLs for investigation
- **Top 10 Slowest Jobs**: Jobs with highest P95 time-to-feedback (best optimization targets), showing failure rates, flakiness, and critical path dependencies
- **Top 10 Failing Jobs**: Most unreliable jobs sorted by failure rate, showing P95 time-to-feedback
- **Top 10 Flaky Jobs**: Most intermittent jobs sorted by flakiness rate, showing P95 time-to-feedback

All tables use color coding for quick visual analysis:

- 🟢 **Green**: Good values (success >80%, failures <25%, flakiness <5%, durations ≤10min)
- 🟡 **Yellow**: Warning values (success 50-80%, failures 25-50%, flakiness 5-10%, durations 10-15min)
- 🔴 **Red**: Critical values (success <50%, failures ≥50%, flakiness ≥10%, durations >15min)

### 📋 JSON Output

For programmatic analysis or integration with other tools, use the `--json` flag:

```json
{
  "provider": "GitLab",
  "project": "group/project",
  "collected_at": "2025-12-21T17:31:48Z",
  "total_pipelines": 8,
  "total_pipeline_types": 4,
  "pipeline_types": [
    {
      "label": "Development",
      "stages": ["test"],
      "ref_patterns": ["main"],
      "sources": ["push"],
      "metrics": {
        "percentage": 62.5,
        "total_pipelines": 5,
        "successful_pipelines": {
          "count": 2,
          "links": ["https://gitlab.com/group/project/-/pipelines/123", "https://gitlab.com/group/project/-/pipelines/124"]
        },
        "failed_pipelines": {
          "count": 3,
          "links": ["https://gitlab.com/group/project/-/pipelines/125", "https://gitlab.com/group/project/-/pipelines/126", "https://gitlab.com/group/project/-/pipelines/127"]
        },
        "success_rate": 40.0,
        "duration_p50": 620.0,
        "duration_p95": 850.0,
        "duration_p99": 890.0,
        "time_to_feedback_p50": 42.0,
        "time_to_feedback_p95": 58.0,
        "time_to_feedback_p99": 62.0,
        "jobs": [
          {
            "name": "integration-tests",
            "duration_p50": 400.0,
            "duration_p95": 480.0,
            "duration_p99": 520.0,
            "time_to_feedback_p50": 610.0,
            "time_to_feedback_p95": 720.0,
            "time_to_feedback_p99": 780.0,
            "predecessors": [
              {
                "name": "lint",
                "duration_p50": 45.0
              },
              {
                "name": "build",
                "duration_p50": 180.0
              }
            ],
            "flakiness_rate": 0.0,
            "flaky_retries": {
              "count": 0,
              "links": []
            },
            "failed_executions": {
              "count": 0,
              "links": []
            },
            "failure_rate": 0.0,
            "total_executions": 5
          },
          {
            "name": "build",
            "duration_p50": 175.0,
            "duration_p95": 200.0,
            "duration_p99": 210.0,
            "time_to_feedback_p50": 220.0,
            "time_to_feedback_p95": 250.0,
            "time_to_feedback_p99": 265.0,
            "predecessors": [
              {
                "name": "lint",
                "duration_p50": 45.0
              }
            ],
            "flakiness_rate": 0.0,
            "flaky_retries": {
              "count": 0,
              "links": []
            },
            "failed_executions": {
              "count": 0,
              "links": []
            },
            "failure_rate": 0.0,
            "total_executions": 5
          },
          {
            "name": "lint",
            "duration_p50": 42.0,
            "duration_p95": 58.0,
            "duration_p99": 62.0,
            "time_to_feedback_p50": 42.0,
            "time_to_feedback_p95": 58.0,
            "time_to_feedback_p99": 62.0,
            "predecessors": [],
            "flakiness_rate": 44.44,
            "flaky_retries": {
              "count": 4,
              "links": ["https://gitlab.com/group/project/-/jobs/501", "https://gitlab.com/group/project/-/jobs/502", "https://gitlab.com/group/project/-/jobs/503", "https://gitlab.com/group/project/-/jobs/504"]
            },
            "failed_executions": {
              "count": 0,
              "links": []
            },
            "failure_rate": 0.0,
            "total_executions": 9
          }
        ]
      }
    }
  ]
}
```

### 📖 JSON Schema Explained

When using `--json` output, the data structure includes:

- **🧩 Pipeline Type Clustering**: Groups pipelines by job signature (exact match). Pipeline types below the configured threshold (default 1%) are filtered out to reduce noise.
- **📊 Type Metrics** (under `metrics`):
  - **`percentage`**: Percentage of total pipelines that belong to this type
  - **`total_pipelines`**: Total number of pipelines in this type
  - **`successful_pipelines`**: Object with `count` and `links` - clickable GitLab URLs to investigate successful pipeline runs
  - **`failed_pipelines`**: Object with `count` and `links` - clickable GitLab URLs to drill down into failed pipeline runs
  - **`success_rate`**: Percentage of successful pipeline runs
  - **`duration_p50`**: Median pipeline execution time (50% of runs complete within this time)
  - **`duration_p95`**: 95th percentile pipeline execution time (use for capacity planning)
  - **`duration_p99`**: 99th percentile pipeline execution time (captures outliers)
  - **`time_to_feedback_p50`**: Median time until first feedback (from the fastest job)
  - **`time_to_feedback_p95`**: 95th percentile time to first feedback
  - **`time_to_feedback_p99`**: 99th percentile time to first feedback
- **💼 Job Metrics** (under `metrics.jobs`, sorted by `time_to_feedback_p95` descending):
  - **`duration_p50`**: Median job execution time (typical duration)
  - **`duration_p95`**: 95th percentile job duration (for planning SLAs)
  - **`duration_p99`**: 99th percentile job duration (outlier detection)
  - **`time_to_feedback_p50`**: Median time from pipeline start to job completion
  - **`time_to_feedback_p95`**: 95th percentile time to feedback (planning metric)
  - **`time_to_feedback_p99`**: 99th percentile time to feedback (worst-case)
  - **`predecessors`**: Jobs that must complete before this one (on the critical path to this job), with their median durations
  - **`flakiness_rate`**: Percentage of job executions that were retries (0.0 if job never needed retries)
  - **`flaky_retries`**: Object with `count` and `links` - clickable GitLab URLs to investigate specific flaky job runs
  - **`failed_executions`**: Object with `count` and `links` - clickable GitLab URLs to investigate failed job runs
  - **`failure_rate`**: Percentage of executions that failed and stayed failed (indicates how often the job catches real bugs)
  - **`total_executions`**: Total number of times this job executed across all pipelines, including successful runs, flaky retries, and failures
- **✅ Success Rate**: Percentage of successful pipeline runs for each type

**Understanding Percentiles:** Percentiles show the distribution of values rather than just the average, which can be misleading for skewed data. P50 (median) represents typical performance, P95 is better for capacity planning and SLAs (95% of runs complete within this time), and P99 helps identify outliers.

**Finding optimization targets:** Jobs with the highest `time_to_feedback_p95` have the worst time-to-feedback and are the best candidates for optimization. Check their `predecessors` to see if you can parallelize or speed up dependencies. Jobs with high `flakiness_rate` indicate intermittent reliability issues - click the `flaky_retries.links` to investigate specific flaky runs in GitLab. Jobs with high `failure_rate` are successfully catching bugs - click the `failed_executions.links` to see which runs failed and analyze the logs.
