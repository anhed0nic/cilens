# 🔍 CILens - CI/CD Insights Tool

A Rust CLI tool for collecting and analyzing CI/CD insights from GitLab.

## ✨ Features

- **🧩 Smart Pipeline Clustering** - Groups pipelines by job signature and filters out rare pipeline types (configurable threshold, default 1%)
- **⏱️ Per-Job Time-to-Feedback** - Shows how long each job takes to complete from pipeline start, revealing actual developer wait times
- **🔍 Dependency Tracking** - Identifies which jobs block others, showing the critical path to each job
- **⚠️ Flakiness Detection** - Identifies unreliable jobs that fail intermittently and need retries
- **📊 Success Rate Metrics** - Per-pipeline-type success rates and failure analysis
- **🎯 Optimization Insights** - Jobs sorted by total duration to quickly identify highest-impact optimization targets

## 📦 Installation

### Installer Script

Install the latest version for your platform:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/dsalaza4/cilens/releases/download/v0.1.0/cilens-installer.sh | sh
```

### Nix

Install using Nix flakes:

```bash
nix profile install github:dsalaza4/cilens
```

Or run without installing:

```bash
nix run github:dsalaza4/cilens -- --help
```

## 🚀 Quick Start

```bash
# Get your GitLab token from: https://gitlab.com/-/profile/personal_access_tokens
# Required scope: read_api

export GITLAB_TOKEN="glpat-your-token"

# Analyze a project
cilens gitlab --project-path "group/project" --limit 20 --pretty
```

## 💡 Usage

```bash
# Basic usage
cilens gitlab --project-path "your/project"

# Save to file
cilens gitlab --project-path "your/project" --output insights.json --pretty

# Filter by branch/ref
cilens gitlab --project-path "your/project" --ref main --limit 50

# Self-hosted GitLab
cilens gitlab --base-url "https://gitlab.example.com" --project-path "your/project"

# Custom filtering threshold (only show pipeline types that are ≥5% of total)
cilens gitlab --project-path "your/project" --min-type-percentage 5
```

## 📄 Output Format

The tool outputs detailed insights grouped by pipeline type:

```json
{
  "provider": "GitLab",
  "project": "group/project",
  "collected_at": "2025-12-21T17:31:48Z",
  "total_pipelines": 8,
  "total_pipeline_types": 4,
  "pipeline_types": [
    {
      "label": "Test Pipeline",
      "pipeline_ids": ["gid://gitlab/Ci::Pipeline/123", "gid://gitlab/Ci::Pipeline/124"],
      "stages": ["test"],
      "ref_patterns": ["main"],
      "sources": ["push"],
      "metrics": {
        "percentage": 62.5,
        "total_pipelines": 5,
        "successful_pipelines": 2,
        "failed_pipelines": 3,
        "success_rate": 40.0,
        "avg_duration_seconds": 648.5,
        "avg_time_to_feedback_seconds": 45.0,
        "jobs": [
          {
            "name": "integration-tests",
            "avg_duration_seconds": 410.0,
            "avg_time_to_feedback_seconds": 635.0,
            "predecessors": [
              {
                "name": "lint",
                "avg_duration_seconds": 45.0
              },
              {
                "name": "build",
                "avg_duration_seconds": 180.0
              }
            ],
            "flakiness_rate": 0.0,
            "flaky_retries": 0,
            "failed_executions": 0,
            "failure_rate": 0.0,
            "total_executions": 5
          },
          {
            "name": "build",
            "avg_duration_seconds": 180.0,
            "avg_time_to_feedback_seconds": 225.0,
            "predecessors": [
              {
                "name": "lint",
                "avg_duration_seconds": 45.0
              }
            ],
            "flakiness_rate": 0.0,
            "flaky_retries": 0,
            "failed_executions": 0,
            "failure_rate": 0.0,
            "total_executions": 5
          },
          {
            "name": "lint",
            "avg_duration_seconds": 45.0,
            "avg_time_to_feedback_seconds": 45.0,
            "predecessors": [],
            "flakiness_rate": 44.44,
            "flaky_retries": 4,
            "failed_executions": 0,
            "failure_rate": 0.0,
            "total_executions": 9
          }
        ]
      }
    }
  ]
}
```

### 📖 Key Metrics Explained

- **🧩 Pipeline Type Clustering**: Groups pipelines by job signature (exact match). Pipeline types below the configured threshold (default 1%) are filtered out to reduce noise.
- **🔑 Pipeline IDs**: GitLab pipeline IDs for all pipelines in this type (useful for drilling down)
- **📊 Type Metrics** (under `metrics`):
  - **`percentage`**: Percentage of total pipelines that belong to this type
  - **`total_pipelines`**: Number of pipelines in this type
  - **`successful_pipelines`**: Number of successful pipeline runs
  - **`failed_pipelines`**: Number of failed pipeline runs
  - **`success_rate`**: Percentage of successful pipeline runs
  - **`avg_duration_seconds`**: Average pipeline execution time
  - **`avg_time_to_feedback_seconds`**: Average time until first feedback (from the fastest job)
- **💼 Job Metrics** (under `metrics.jobs`, sorted by `avg_time_to_feedback_seconds` descending):
  - **`avg_duration_seconds`**: How long the job itself takes to run
  - **`avg_time_to_feedback_seconds`**: Time from pipeline start to job completion (when developers get feedback)
  - **`predecessors`**: Jobs that must complete before this one (on the critical path to this job), with their durations
  - **`flakiness_rate`**: Percentage of job executions that were retries (0.0 if job never needed retries)
  - **`flaky_retries`**: Total number of retry attempts across all pipelines (only counts retries that eventually succeeded, 0 if never retried)
  - **`failed_executions`**: Number of times this job failed and stayed failed (did not eventually succeed, 0 if never failed)
  - **`failure_rate`**: Percentage of executions that failed and stayed failed (indicates how often the job catches real bugs)
  - **`total_executions`**: Total number of times this job executed across all pipelines, including successful runs, flaky retries, and failures
- **✅ Success Rate**: Percentage of successful pipeline runs for each type

**Finding optimization targets:** Jobs with the highest `avg_time_to_feedback_seconds` have the worst time-to-feedback and are the best candidates for optimization. Check their `predecessors` to see if you can parallelize or speed up dependencies. Jobs with high `flakiness_rate` indicate intermittent reliability issues that need investigation. Jobs with high `failure_rate` are successfully catching bugs and stopping developers from committing mistakes.

## 🔮 Future Work

The following insights would provide additional value for teams analyzing their CI/CD pipelines:

### 🚀 High-Impact Additions

#### 📈 Duration Percentiles (P50, P95, P99)

```json
"duration_percentiles": {
  "p50": 650.0,
  "p95": 1800.0,
  "p99": 2100.0
}
```

**Value**: Shows realistic expectations vs average (which can be skewed by outliers).

#### 💸 Waste Metrics

```json
"waste_metrics": {
  "failed_pipeline_time_seconds": 12450.0,
  "retry_overhead_seconds": 3200.0,
  "estimated_cost_wasted": "$XX"
}
```

**Value**: Quantifies the business impact of failures and inefficiencies.

#### ⚡ Parallelization Efficiency

```json
"parallelization_efficiency": {
  "theoretical_min_duration": 450.0,
  "actual_duration": 650.0,
  "efficiency_score": 0.69,
  "underutilized_stages": ["test", "build"]
}
```

**Value**: Reveals if you're effectively using parallel runners.

#### 🎭 Stage-Level Insights

```json
"stage_breakdown": [
  {
    "name": "test",
    "avg_duration_seconds": 420.0,
    "failure_rate": 15.5,
    "parallelism": 8,
    "percentage_of_total": 35.0
  }
]
```

**Value**: Helps identify which stages are problematic or slow.

#### 📊 Trend Indicators

(When analyzing multiple time windows)

```json
"trends": {
  "success_rate_trend": "improving",
  "duration_trend": "stable",
  "retry_rate_trend": "worsening"
}
```

**Value**: Shows if things are getting better or worse over time.
