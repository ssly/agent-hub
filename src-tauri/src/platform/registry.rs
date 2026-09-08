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
            id: "shared".into(),
            display_name: "Shared".into(),
            description: "Shared skill directory for agents".into(),
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
            id: "cursor".into(),
            display_name: "Cursor".into(),
            description: "Cursor IDE custom skills".into(),
            presence_path: home.join(".cursor"),
            skill_dir: join_relative(home.clone(), ".cursor/skills"),
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
            id: "qwen".into(),
            display_name: "Qwen Code".into(),
            description: "Alibaba Qwen Code agent skills".into(),
            presence_path: home.join(".qwen"),
            skill_dir: join_relative(home.clone(), ".qwen/skills"),
        },
        PlatformDef {
            id: "zcode".into(),
            display_name: "ZCode".into(),
            description: "Z.ai ZCode agent skills".into(),
            presence_path: home.join(".zcode"),
            skill_dir: join_relative(home.clone(), ".zcode/skills"),
        },
        PlatformDef {
            id: "workbuddy".into(),
            display_name: "WorkBuddy".into(),
            description: "Tencent WorkBuddy agent skills".into(),
            presence_path: home.join(".workbuddy"),
            skill_dir: join_relative(home.clone(), ".workbuddy/skills"),
        },
        PlatformDef {
            id: "kiro".into(),
            display_name: "Kiro".into(),
            description: "Amazon Kiro IDE agent skills".into(),
            presence_path: home.join(".kiro"),
            skill_dir: join_relative(home.clone(), ".kiro/skills"),
        },
        PlatformDef {
            id: "dsh".into(),
            display_name: "DeepSeek Harness".into(),
            description: "DeepSeek Harness (dsh CLI) agent skills".into(),
            presence_path: home.join(".dsh"),
            // DSH reads user-level skills from ~/.dsh/skills (and, via the
            // Shared platform, ~/.agents/skills). Project level is
            // <workspace>/.dsh/skills, which the default mirror already maps.
            skill_dir: join_relative(home.clone(), ".dsh/skills"),
        },
        PlatformDef {
            id: "omp".into(),
            display_name: "Oh My Pi".into(),
            description: "Oh My Pi (omp) coding agent skills".into(),
            presence_path: home.join(".omp"),
            // omp keeps user-level skills inside its agent directory
            // (~/.omp/agent/skills); the project level lives at
            // <workspace>/.omp/skills (see workspace_skill_dir below).
            skill_dir: join_relative(home.clone(), ".omp/agent/skills"),
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
    // omp mirrors its agent dir globally (~/.omp/agent/skills) but drops the
    // `agent` segment at project level (<workspace>/.omp/skills).
    if platform_id == "omp" {
        return Some(join_relative(workspace.to_path_buf(), ".omp/skills"));
    }
    let home = dirs::home_dir()?;
    let def = builtin_platforms()
        .into_iter()
        .find(|platform| platform.id == platform_id)?;
    let relative = def.skill_dir.strip_prefix(home).ok()?;
    Some(workspace.join(relative))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SupportedAgentInfo {
    pub id: String,
    pub display_name: String,
    pub user_dir_display: String,
    pub user_dir_resolved: String,
    pub exists: bool,
    pub enabled: bool,
}

pub fn get_supported_agents(config: &crate::config::Config) -> Vec<SupportedAgentInfo> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut agents = Vec::new();

    let builtin = builtin_platforms();
    for p in builtin {
        // "shared" is the shared skill directory across agents, not an
        // individual configurable agent. It is always enabled and omitted
        // from the Settings agent toggle list.
        if p.id == "shared" {
            continue;
        }

        let (display_path, candidates) = match p.id.as_str() {
            "shared" => ("~/.agents", vec![home.join(".agents")]),
            "codex" => ("~/.codex", vec![home.join(".codex")]),
            "claude-code" => ("~/.claude", vec![home.join(".claude"), home.join(".claude.json")]),
            "cursor" => ("~/.cursor", vec![home.join(".cursor")]),
            "antigravity" => (
                "~/.gemini",
                vec![
                    join_relative(home.clone(), ".gemini/config"),
                    home.join(".gemini"),
                ],
            ),
            "grok-build" => ("~/.grok", vec![home.join(".grok")]),
            "kimi-code" => ("~/.kimi-code", vec![home.join(".kimi-code")]),
            "qwen" => ("~/.qwen", vec![home.join(".qwen")]),
            "zcode" => ("~/.zcode", vec![home.join(".zcode")]),
            "workbuddy" => (
                "~/.workbuddy",
                vec![home.join(".workbuddy"), home.join(".codebuddy")],
            ),
            "kiro" => ("~/.kiro", vec![home.join(".kiro")]),
            "dsh" => ("~/.dsh", vec![home.join(".dsh")]),
            "omp" => ("~/.omp", vec![home.join(".omp")]),
            _ => ("~", vec![p.presence_path.clone()]),
        };

        let existing_candidate = candidates.iter().find(|c| c.exists());
        let exists = existing_candidate.is_some();
        let user_dir_resolved = existing_candidate
            .cloned()
            .unwrap_or_else(|| candidates[0].clone())
            .display()
            .to_string();

        let enabled = if let Some(ref list) = config.general.enabled_platforms {
            list.contains(&p.id)
        } else {
            exists
        };

        agents.push(SupportedAgentInfo {
            id: p.id,
            display_name: p.display_name,
            user_dir_display: display_path.to_string(),
            user_dir_resolved,
            exists,
            enabled,
        });
    }

    for custom in &config.platforms {
        let presence_path = join_relative(home.clone(), &custom.skill_dir);
        let exists = presence_path.exists();
        let enabled = if let Some(ref list) = config.general.enabled_platforms {
            list.contains(&custom.id)
        } else {
            exists
        };
        agents.push(SupportedAgentInfo {
            id: custom.id.clone(),
            display_name: custom.display_name.clone(),
            user_dir_display: custom.skill_dir.clone(),
            user_dir_resolved: presence_path.display().to_string(),
            exists,
            enabled,
        });
    }

    agents
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
            workspace_skill_dir("shared", &root),
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
        assert_eq!(
            workspace_skill_dir("qwen", &root),
            Some(root.join(".qwen").join("skills"))
        );
        assert_eq!(
            workspace_skill_dir("zcode", &root),
            Some(root.join(".zcode").join("skills"))
        );
        assert_eq!(
            workspace_skill_dir("workbuddy", &root),
            Some(root.join(".workbuddy").join("skills"))
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
    fn codex_user_skills_live_in_shared() {
        let codex = builtin_platforms()
            .into_iter()
            .find(|platform| platform.id == "codex")
            .expect("codex platform should exist");
        let shared = builtin_platforms()
            .into_iter()
            .find(|platform| platform.id == "shared")
            .expect("shared platform should exist");
        assert_eq!(codex.skill_dir, shared.skill_dir);
    }

    #[test]
    fn builtin_order_puts_curated_platforms_first() {
        let platforms = builtin_platforms();
        let ids: Vec<&str> = platforms
            .iter()
            .map(|platform| platform.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "shared",
                "codex",
                "claude-code",
                "cursor",
                "antigravity",
                "grok-build",
                "kimi-code",
                "qwen",
                "zcode",
                "workbuddy",
                "kiro",
                "dsh",
                "omp",
            ]
        );
    }

    #[test]
    fn omp_workspace_skills_drop_the_agent_segment() {
        let root = PathBuf::from("/tmp/example-project");
        assert_eq!(
            workspace_skill_dir("omp", &root),
            Some(root.join(".omp").join("skills"))
        );
    }

    #[test]
    fn get_supported_agents_lists_all_builtins_with_paths() {
        let config = crate::config::Config::default();
        let agents = get_supported_agents(&config);
        // "shared" is omitted as it is the shared skill directory, not a configurable agent.
        assert_eq!(agents.len(), 12);
        assert_eq!(agents[0].id, "codex");
        assert_eq!(agents[0].user_dir_display, "~/.codex");
        assert_eq!(agents[1].id, "claude-code");
        assert_eq!(agents[1].user_dir_display, "~/.claude");

        // When enabled_platforms is None, enabled matches exists (e.g. codex enabled if ~/.codex exists)
        for agent in &agents {
            assert_eq!(agent.enabled, agent.exists);
            assert!(!agent.user_dir_resolved.is_empty());
        }

        // When enabled_platforms is Some, enabled matches the list
        let mut custom_config = crate::config::Config::default();
        custom_config.general.enabled_platforms = Some(vec!["codex".into(), "claude-code".into()]);
        let custom_agents = get_supported_agents(&custom_config);
        for agent in &custom_agents {
            if agent.id == "codex" || agent.id == "claude-code" {
                assert!(agent.enabled);
            } else {
                assert!(!agent.enabled);
            }
        }
    }
}
