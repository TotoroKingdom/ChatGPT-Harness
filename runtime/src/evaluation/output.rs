use super::aggregate::{AggregateReport, BaselineVerdict};
use std::io::{self, Write};

pub(crate) fn print_human_summary(
    report: &AggregateReport,
    mut output: impl Write,
) -> io::Result<()> {
    let metrics = &report.metrics;
    writeln!(
        output,
        "M04 evaluation: formal={} adapterClass={}",
        report.formal_baseline, report.adapter_class
    )?;
    writeln!(
        output,
        "runs: total={} scoreable={} infra={} limit={} security={} contract={}",
        metrics.total_runs,
        metrics.scoreable_runs,
        metrics.infra_error_runs,
        metrics.limit_exceeded_runs,
        metrics.security_violation_runs,
        metrics.harness_contract_error_runs
    )?;
    print_metric(&mut output, "tool_selection_accuracy", &metrics.selection)?;
    print_metric(&mut output, "argument_validity", &metrics.argument_validity)?;
    print_metric(
        &mut output,
        "runtime_outcome_match_rate",
        &metrics.runtime_outcome_match,
    )?;
    print_metric(
        &mut output,
        "task_completion_rate",
        &metrics.task_completion,
    )?;
    print_metric(
        &mut output,
        "unnecessary_call_rate",
        &metrics.unnecessary_calls,
    )?;
    writeln!(
        output,
        "tool calls per scoreable run: mean={:.2} median={:.2}",
        metrics.mean_tool_calls, metrics.median_tool_calls
    )?;
    for category in &metrics.categories {
        writeln!(
            output,
            "category {:<28} scoreable={}/{} sel={:.3} arg={:.3} out={:.3} task={:.3} unnec={:.3}",
            category.category,
            category.scoreable_runs,
            category.total_runs,
            category.selection.rate(),
            category.argument_validity.rate(),
            category.runtime_outcome_match.rate(),
            category.task_completion.rate(),
            category.unnecessary_calls.rate()
        )?;
    }
    for case in &metrics.cases {
        writeln!(
            output,
            "case {} scoreable={}/{} selection={}/{} task={}/{}",
            case.case_id,
            case.scoreable_runs,
            case.total_runs,
            case.selection_passes,
            case.scoreable_runs,
            case.task_completion_passes,
            case.scoreable_runs
        )?;
    }
    writeln!(
        output,
        "baseline validity: {:?}",
        report
            .validity
            .verdict
            .unwrap_or(BaselineVerdict::Inconclusive)
    )?;
    for reason in &report.validity.reasons {
        writeln!(output, "  - {reason}")?;
    }
    Ok(())
}

fn print_metric(
    mut output: impl Write,
    name: &str,
    metric: &super::aggregate::Metric,
) -> io::Result<()> {
    writeln!(
        output,
        "{name}: {:.4} ({}/{})",
        metric.rate(),
        metric.numerator,
        metric.denominator
    )
}

pub(crate) fn report_json(report: &AggregateReport) -> serde_json::Result<String> {
    serde_json::to_string_pretty(report)
}
