# CILens - CI/CD Insights Tool

A Rust CLI tool for collecting and analyzing CI/CD insights from GitLab.

## Features

- **Smart Pipeline Clustering** - Groups pipelines by job signature and automatically merges similar types (>80% similarity) to reduce noise
- **Accurate Critical Path Analysis** - Identifies the slowest execution path, correctly handling both explicit dependencies and stage-based execution
- **Flakiness Detection** - Identifies unreliable jobs that fail intermittently and need retries (top 5 flakiest jobs)
- **Success Rate Metrics** - Per-pipeline-type success rates and failure analysis
- **Duration Analytics** - Average duration tracking for pipelines and critical paths

## Installation

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

## Quick Start

```bash
# Get your GitLab token from: https://gitlab.com/-/profile/personal_access_tokens
# Required scope: read_api

export GITLAB_TOKEN="glpat-your-token"

# Analyze a project
cilens gitlab --project "group/project" --limit 20 --pretty
```

## Usage

```bash
# Basic usage
cilens gitlab --project "your/project"

# Save to file
cilens gitlab --project "your/project" --output insights.json --pretty

# Filter by branch
cilens gitlab --project "your/project" --branch main --limit 50

# Self-hosted GitLab
cilens gitlab --url "https://gitlab.example.com" --project "your/project"
```

## Options

**Global:**
- `-o, --output <FILE>` - Output file path (default: stdout)
- `-p, --pretty` - Pretty print JSON

**GitLab:**
- `-t, --token <TOKEN>` - GitLab token (or use `GITLAB_TOKEN` env var)
- `-u, --url <URL>` - GitLab instance URL (default: https://gitlab.com)
- `-P, --project <PROJECT>` - Project ID or path (e.g., "group/project")
- `-l, --limit <LIMIT>` - Number of pipelines to analyze (default: 20)
- `-b, --branch <BRANCH>` - Filter by branch (optional)

## Output Format

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
      "count": 5,
      "percentage": 62.5,
      "jobs": ["lint", "test", "deploy"],
      "ids": ["gid://gitlab/Ci::Pipeline/123", "gid://gitlab/Ci::Pipeline/124"],
      "stages": ["test"],
      "ref_patterns": ["main"],
      "sources": ["push"],
      "metrics": {
        "total_pipelines": 5,
        "successful_pipelines": 2,
        "failed_pipelines": 3,
        "success_rate": 40.0,
        "average_duration_seconds": 648.5,
        "critical_path": {
          "jobs": ["dns-infra plan"],
          "average_duration_seconds": 635.0
        },
        "flaky_jobs": {
          "dns-infra plan": {
            "total_occurrences": 9,
            "retry_count": 4,
            "flakiness_score": 44.44
          },
          "vpc-infra lint": {
            "total_occurrences": 9,
            "retry_count": 4,
            "flakiness_score": 44.44
          },
          "lint": {
            "total_occurrences": 14,
            "retry_count": 4,
            "flakiness_score": 28.57
          }
        }
      }
    }
  ]
}
```

### Key Metrics Explained

- **Pipeline Type Clustering**: Groups pipelines by job signature (exact match), then merges types with >80% job similarity to reduce fragmentation. For example, pipelines differing by only 1-2 optional jobs are merged into a single type.
- **Jobs**: Union of all jobs that appear in this pipeline type (when types are merged, shows all jobs from variants)
- **IDs**: GitLab pipeline IDs for all pipelines in this type (useful for drilling down)
- **Critical Path**: The slowest execution path through the pipeline, considering both explicit job dependencies (`needs`) and stage-based execution
- **Flaky Jobs**: Identifies unreliable jobs with flakiness score (% of runs needing retry), retry count, and total occurrences (only jobs appearing 2+ times, top 5 shown)
- **Success Rate**: Percentage of successful pipeline runs for each type

## Future Work

The following insights would provide additional value for teams analyzing their CI/CD pipelines:

### High-Impact Additions

#### 1. Duration Percentiles (P50, P95, P99)

```json
"duration_percentiles": {
  "p50": 650.0,
  "p95": 1800.0,
  "p99": 2100.0
}
```

**Value**: Shows realistic expectations vs average (which can be skewed by outliers).

#### 2. Waste Metrics

```json
"waste_metrics": {
  "failed_pipeline_time_seconds": 12450.0,
  "retry_overhead_seconds": 3200.0,
  "estimated_cost_wasted": "$XX"
}
```

**Value**: Quantifies the business impact of failures and inefficiencies.

#### 3. Failure Patterns

```json
"most_failing_jobs": [
  {
    "name": "e2e-tests",
    "failure_rate": 35.5,
    "total_runs": 120
  }
]
```

**Value**: Shows jobs with chronic failures (different from flakiness which indicates intermittent issues).

#### 4. Parallelization Efficiency

```json
"parallelization_efficiency": {
  "theoretical_min_duration": 450.0,
  "actual_duration": 650.0,
  "efficiency_score": 0.69,
  "underutilized_stages": ["test", "build"]
}
```

**Value**: Reveals if you're effectively using parallel runners.

#### 5. Time-to-Feedback

```json
"feedback_metrics": {
  "time_to_first_failure_avg": 180.0,
  "time_to_first_failure_p95": 450.0
}
```

**Value**: Critical for developer experience - faster feedback = faster fixes.

#### 6. Stage-Level Insights

```json
"stage_breakdown": [
  {
    "name": "test",
    "avg_duration": 420.0,
    "failure_rate": 15.5,
    "parallelism": 8,
    "percentage_of_total": 35.0
  }
]
```

**Value**: Helps identify which stages are problematic or slow.

#### 7. Trend Indicators

(When analyzing multiple time windows)

```json
"trends": {
  "success_rate_trend": "improving",
  "duration_trend": "stable",
  "retry_rate_trend": "worsening"
}
```

**Value**: Shows if things are getting better or worse over time.

#### 8. Job Dependency Impact

```json
"blocking_jobs": [
  {
    "name": "lint",
    "blocks_count": 25,
    "avg_delay_caused": 45.0
  }
]
```

**Value**: Identifies jobs that, when slow/failing, block the most downstream work.
