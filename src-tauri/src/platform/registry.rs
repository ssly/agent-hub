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
            id: "shared-pool".into(),
            display_name: "Shared Pool".into(),
            description: "Shared skill pool for all agents".into(),
            presence_path: home.join(".agents"),
            skill_dir: join_relative(home.clone(), ".agents/skills"),
        },
        PlatformDef {
            id: "codex".into(),
            display_name: "Codex".into(),
            description: "OpenAI Codex agent skills".into(),
            presence_path: home.join(".codex"),
            // Codex officially reads user-level skills only from the shared
            // pool (~/.agents/skills); ~/.codex/skills is a community myth
            // that Codex itself never loads.
            skill_dir: join_relative(home.clone(), ".agents/skills"),
        },
        PlatformDef {
            id: "claude-code".into(),
            display_name: "Claude Code".into(),
            description: "Anthropic Claude Code CLI agent skills".into(),
            presence_path: home.join(".claude"),
            skill_dir: join_relative(home.clone(), ".claude/skills"),
        },
        PlatformDef {
            id: "antigravity".into(),
            display_name: "Antigravity".into(),
            description: "Google Antigravity (agy CLI / 2.0) agent skills".into(),
            presence_path: join_relative(home.clone(), ".gemini/config"),
            skill_dir: join_relative(home.clone(), ".gemini/config/skills"),
        },
        PlatformDef {
            id: "grok-build".into(),
            display_name: "Grok Build".into(),
            description: "xAI Grok Build agent skills".into(),
            presence_path: home.join(".grok"),
            skill_dir: join_relative(home.clone(), ".grok/skills"),
        },
        PlatformDef {
            id: "kimi-code".into(),
            display_name: "Kimi Code".into(),
            description: "Moonshot Kimi Code agent skills".into(),
            presence_path: home.join(".kimi-code"),
            skill_dir: join_relative(home.clone(), ".kimi-code/skills"),
        },
        PlatformDef {
            id: "cursor".into(),
            display_name: "Cursor".into(),
            description: "Cursor IDE custom skills".into(),
            presence_path: home.join(".cursor"),
            skill_dir: join_relative(home.clone(), ".cursor/skills"),
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
    ]
}

/// Resolve the project-scoped skill directory for a built-in platform.
///
/// Agent Hub intentionally mirrors each platform's existing global layout
/// under the selected workspace. Claude Code, for example, maps
/// `~/.claude/skills` to `<workspace>/.claude/skills`.
pub fn workspace_skill_dir(platform_id: &str, workspace: &std::path::Path) -> Option<PathBuf> {
    // Antigravity's project-level skills live in `.agents/skills`, not in a
    // mirror of its global ~/.gemini/config/skills layout.
    if platform_id == "antigravity" {
        return Some(join_relative(workspace.to_path_buf(), ".agents/skills"));
    }
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
        assert_eq!(
            workspace_skill_dir("codex", &root),
            Some(root.join(".agents").join("skills"))
        );
        assert_eq!(
            workspace_skill_dir("grok-build", &root),
            Some(root.join(".grok").join("skills"))
        );
        assert_eq!(
            workspace_skill_dir("kimi-code", &root),
            Some(root.join(".kimi-code").join("skills"))
        );
    }

    #[test]
    fn antigravity_workspace_skills_use_agents_dir_not_gemini_mirror() {
        let root = PathBuf::from("/tmp/example-project");
        assert_eq!(
            workspace_skill_dir("antigravity", &root),
            Some(root.join(".agents").join("skills"))
        );
    }

    #[test]
    fn codex_user_skills_live_in_the_shared_pool() {
        let codex = builtin_platforms()
            .into_iter()
            .find(|platform| platform.id == "codex")
            .expect("codex platform should exist");
        let shared_pool = builtin_platforms()
            .into_iter()
            .find(|platform| platform.id == "shared-pool")
            .expect("shared-pool platform should exist");
        assert_eq!(codex.skill_dir, shared_pool.skill_dir);
    }

    #[test]
    fn builtin_order_puts_curated_platforms_first() {
        let platforms = builtin_platforms();
        let ids: Vec<&str> = platforms
            .iter()
            .map(|platform| platform.id.as_str())
            .collect();
        assert_eq!(
            &ids[..6],
            [
                "shared-pool",
                "codex",
                "claude-code",
                "antigravity",
                "grok-build",
                "kimi-code"
            ]
        );
    }
}
