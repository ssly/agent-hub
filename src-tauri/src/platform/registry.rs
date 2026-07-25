use std::path::PathBuf;

// Re-export so existing `crate::platform::registry::join_relative` call
// sites keep working.
pub use crate::paths::join_relative;

#[derive(Debug, Clone)]
pub struct PlatformDef {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub presence_path: PathBuf,
    pub skill_dir: PathBuf,
}

pub fn builtin_platforms() -> Vec<PlatformDef> {
    let home = dirs::home_dir().expect("no home directory");
    vec![
        PlatformDef {
            id: "claude-code".into(),
            display_name: "Claude Code".into(),
            description: "Anthropic Claude Code CLI agent skills".into(),
            presence_path: home.join(".claude"),
            skill_dir: join_relative(home.clone(), ".claude/skills"),
        },
        PlatformDef {
            id: "codex".into(),
            display_name: "Codex".into(),
            description: "OpenAI Codex agent skills".into(),
            presence_path: home.join(".codex"),
            skill_dir: join_relative(home.clone(), ".codex/skills"),
        },
        PlatformDef {
            id: "cursor".into(),
            display_name: "Cursor".into(),
            description: "Cursor IDE custom skills".into(),
            presence_path: home.join(".cursor"),
            skill_dir: join_relative(home.clone(), ".cursor/skills-cursor"),
        },
        PlatformDef {
            id: "gemini".into(),
            display_name: "Gemini".into(),
            description: "Google Gemini CLI agent skills".into(),
            presence_path: home.join(".gemini"),
            skill_dir: join_relative(home.clone(), ".gemini/skills"),
        },
        PlatformDef {
            id: "openclaw".into(),
            display_name: "OpenClaw".into(),
            description: "OpenClaw agent skills".into(),
            presence_path: home.join(".openclaw"),
            skill_dir: join_relative(home.clone(), ".openclaw/skills"),
        },
        PlatformDef {
            id: "hermes".into(),
            display_name: "Hermes".into(),
            description: "Hermes agent skills".into(),
            presence_path: home.join(".hermes"),
            skill_dir: join_relative(home.clone(), ".hermes/skills"),
        },
        PlatformDef {
            id: "trae".into(),
            display_name: "Trae".into(),
            description: "ByteDance Trae IDE agent skills".into(),
            presence_path: home.join(".trae"),
            skill_dir: join_relative(home.clone(), ".trae/skills"),
        },
        PlatformDef {
            id: "kiro".into(),
            display_name: "Kiro".into(),
            description: "Amazon Kiro IDE agent skills".into(),
            presence_path: home.join(".kiro"),
            skill_dir: join_relative(home.clone(), ".kiro/skills"),
        },
        PlatformDef {
            id: "shared-pool".into(),
            display_name: "Shared Pool".into(),
            description: "Shared skill pool for all agents".into(),
            presence_path: home.join(".agents"),
            skill_dir: join_relative(home.clone(), ".agents/skills"),
        },
    ]
}

/// Resolve the project-scoped skill directory for a built-in platform.
///
/// Agent Hub intentionally mirrors each platform's existing global layout
/// under the selected workspace. Claude Code, for example, maps
/// `~/.claude/skills` to `<workspace>/.claude/skills`.
pub fn workspace_skill_dir(platform_id: &str, workspace: &std::path::Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let def = builtin_platforms()
        .into_iter()
        .find(|platform| platform.id == platform_id)?;
    let relative = def.skill_dir.strip_prefix(home).ok()?;
    Some(workspace.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_global_skill_layout_into_workspace() {
        let root = PathBuf::from("/tmp/example-project");
        assert_eq!(
            workspace_skill_dir("claude-code", &root),
            Some(root.join(".claude").join("skills"))
        );
        assert_eq!(
            workspace_skill_dir("shared-pool", &root),
            Some(root.join(".agents").join("skills"))
        );
    }
}
