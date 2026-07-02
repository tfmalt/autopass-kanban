use std::path::Path;

use serde::Serialize;

use crate::{DoctorFinding, ValidationReport};

use super::rel_to_root;

/// DTO for a single validation issue, used in `validate` responses.
#[derive(Debug, Clone, Serialize)]
pub struct IssueDto {
    pub path: String,
    pub rule: String,
    pub message: String,
}

/// DTO for a `validate` response.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateDto {
    pub valid: bool,
    pub story_count: usize,
    pub issue_count: usize,
    pub issues: Vec<IssueDto>,
}

impl ValidateDto {
    pub fn from_report(report: &ValidationReport, repo_root: &Path) -> Self {
        let valid = report.issues.is_empty();
        let issues: Vec<IssueDto> = report
            .issues
            .iter()
            .map(|i| IssueDto {
                path: rel_to_root(repo_root, &i.file_path),
                rule: i.rule.clone(),
                message: i.message.clone(),
            })
            .collect();
        Self {
            valid,
            story_count: report.stories.len(),
            issue_count: issues.len(),
            issues,
        }
    }
}

/// DTO for a single doctor finding.
#[derive(Debug, Clone, Serialize)]
pub struct FindingDto {
    pub severity: String,
    pub scope: String,
    pub message: String,
}

/// Summary counts of doctor findings by severity.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DoctorSummary {
    pub error: usize,
    pub warning: usize,
    pub info: usize,
}

/// DTO for a `doctor` response.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorDto {
    pub healthy: bool,
    pub findings: Vec<FindingDto>,
    pub summary: DoctorSummary,
}

impl DoctorDto {
    pub fn from_findings(findings: &[DoctorFinding]) -> Self {
        let mut summary = DoctorSummary::default();
        for f in findings {
            match f.severity.to_ascii_lowercase().as_str() {
                "error" => summary.error += 1,
                "warning" => summary.warning += 1,
                _ => summary.info += 1,
            }
        }
        let healthy = findings.is_empty();
        Self {
            healthy,
            findings: findings
                .iter()
                .map(|f| FindingDto {
                    severity: f.severity.clone(),
                    scope: f.scope.clone(),
                    message: f.message.clone(),
                })
                .collect(),
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn validate_dto_reports_counts_and_validity() {
        let report = crate::ValidationReport {
            repo_root: PathBuf::from("/repo"),
            stories: vec![],
            issues: vec![crate::ValidationIssue {
                file_path: PathBuf::from("/repo/delivery/backlog/x/US-F1-009.md"),
                rule: "missing_frontmatter_field".to_string(),
                message: "missing status".to_string(),
            }],
        };
        let dto = ValidateDto::from_report(&report, std::path::Path::new("/repo"));
        assert!(!dto.valid);
        assert_eq!(dto.issue_count, 1);
        assert_eq!(dto.story_count, 0);
        assert_eq!(dto.issues[0].rule, "missing_frontmatter_field");
        assert_eq!(dto.issues[0].path, "delivery/backlog/x/US-F1-009.md");

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["valid"], false);
        assert_eq!(json["issue_count"], 1);
        assert_eq!(json["story_count"], 0);
        assert_eq!(json["issues"][0]["rule"], "missing_frontmatter_field");
        assert_eq!(json["issues"][0]["path"], "delivery/backlog/x/US-F1-009.md");
    }

    #[test]
    fn doctor_dto_summarizes_findings_by_severity() {
        let findings = vec![
            crate::DoctorFinding {
                severity: "warning".to_string(),
                scope: "US-F1-001".to_string(),
                message: "story has no sprint".to_string(),
            },
            crate::DoctorFinding {
                severity: "warning".to_string(),
                scope: "US-F1-002".to_string(),
                message: "story has no sprint".to_string(),
            },
        ];
        let dto = DoctorDto::from_findings(&findings);
        assert!(!dto.healthy);
        assert_eq!(dto.summary.warning, 2);
        assert_eq!(dto.summary.error, 0);
        assert_eq!(dto.summary.info, 0);
        assert_eq!(dto.findings[0].scope, "US-F1-001");

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["healthy"], false);
        assert_eq!(json["summary"]["warning"], 2);
        assert_eq!(json["summary"]["error"], 0);
        assert_eq!(json["findings"][0]["scope"], "US-F1-001");
    }
}
