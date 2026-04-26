use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::PathBuf;

use similar::{ChangeTag, TextDiff};

use crate::skill::Skill;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffResult {
    pub source_platform: String,
    pub target_platform: String,
    pub skill_name: String,
    pub file_diffs: Vec<FileDiff>,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileDiff {
    pub file_path: String,
    pub lines: Vec<DiffLine>,
    pub stats: DiffStats,
    pub source_only: bool,
    pub target_only: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffStats {
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
    FileHeader(String),
}

pub fn diff_skills(source: &Skill, target: &Skill) -> DiffResult {
    let mut file_diffs = Vec::new();
    let source_files: HashSet<PathBuf> = source.files.iter().cloned().collect();
    let target_files: HashSet<PathBuf> = target.files.iter().cloned().collect();
    let all_files: BTreeSet<PathBuf> = source_files.union(&target_files).cloned().collect();

    for file in all_files {
        let in_source = source_files.contains(&file);
        let in_target = target_files.contains(&file);

        match (in_source, in_target) {
            (true, false) => {
                let line_count = read_line_count(&source.path.join(&file));
                file_diffs.push(FileDiff {
                    file_path: file.display().to_string(),
                    lines: vec![DiffLine::FileHeader(format!("Only in {}", source.platform_id))],
                    stats: DiffStats { added: line_count, removed: 0 },
                    source_only: true, target_only: false,
                });
            }
            (false, true) => {
                let line_count = read_line_count(&target.path.join(&file));
                file_diffs.push(FileDiff {
                    file_path: file.display().to_string(),
                    lines: vec![DiffLine::FileHeader(format!("Only in {}", target.platform_id))],
                    stats: DiffStats { added: 0, removed: line_count },
                    source_only: false, target_only: true,
                });
            }
            (false, false) => {}
            (true, true) => {
                let src_content = fs::read_to_string(source.path.join(&file)).unwrap_or_default();
                let tgt_content = fs::read_to_string(target.path.join(&file)).unwrap_or_default();
                if src_content != tgt_content {
                    let lines = unified_diff(&src_content, &tgt_content);
                    let stats = count_stats(&lines);
                    file_diffs.push(FileDiff {
                        file_path: file.display().to_string(), lines, stats,
                        source_only: false, target_only: false,
                    });
                }
            }
        }
    }

    let stats = file_diffs.iter().fold(DiffStats { added: 0, removed: 0 }, |acc, d| {
        DiffStats { added: acc.added + d.stats.added, removed: acc.removed + d.stats.removed }
    });

    DiffResult { source_platform: source.platform_id.clone(), target_platform: target.platform_id.clone(),
        skill_name: source.name.clone(), file_diffs, stats }
}

fn unified_diff(source: &str, target: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(source, target);
    diff.iter_all_changes().map(|change| {
        let line = change.to_string_lossy().into_owned();
        match change.tag() {
            ChangeTag::Equal => DiffLine::Context(line),
            ChangeTag::Insert => DiffLine::Added(line),
            ChangeTag::Delete => DiffLine::Removed(line),
        }
    }).collect()
}

fn count_stats(lines: &[DiffLine]) -> DiffStats {
    let mut added = 0; let mut removed = 0;
    for line in lines { match line { DiffLine::Added(_) => added += 1, DiffLine::Removed(_) => removed += 1, _ => {} } }
    DiffStats { added, removed }
}

fn read_line_count(path: &std::path::Path) -> usize {
    fs::read_to_string(path).map(|s| s.lines().count()).unwrap_or(0)
}
