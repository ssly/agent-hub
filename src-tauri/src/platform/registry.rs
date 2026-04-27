use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PlatformDef {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub skill_dir: PathBuf,
}

pub fn builtin_platforms() -> Vec<PlatformDef> {
    let home = dirs::home_dir().expect("no home directory");
    vec![
        PlatformDef { id: "claude-code".into(), display_name: "Claude Code".into(), description: "Anthropic Claude Code CLI agent skills".into(), skill_dir: home.join(".claude/skills") },
        PlatformDef { id: "codex-cli".into(), display_name: "Codex CLI".into(), description: "OpenAI Codex CLI agent skills".into(), skill_dir: home.join(".codex/skills") },
        PlatformDef { id: "cursor".into(), display_name: "Cursor".into(), description: "Cursor IDE custom skills".into(), skill_dir: home.join(".cursor/skills-cursor") },
        PlatformDef { id: "gemini".into(), display_name: "Gemini".into(), description: "Google Gemini CLI agent skills".into(), skill_dir: home.join(".gemini/skills") },
        PlatformDef { id: "openclaw".into(), display_name: "OpenClaw".into(), description: "OpenClaw agent skills".into(), skill_dir: home.join(".openclaw/skills") },
        PlatformDef { id: "hermes".into(), display_name: "Hermes".into(), description: "Hermes agent skills".into(), skill_dir: home.join(".hermes/skills") },
        PlatformDef { id: "trae".into(), display_name: "Trae".into(), description: "ByteDance Trae IDE agent skills".into(), skill_dir: home.join(".trae/skills") },
        PlatformDef { id: "kiro".into(), display_name: "Kiro".into(), description: "Amazon Kiro IDE agent skills".into(), skill_dir: home.join(".kiro/skills") },
        PlatformDef { id: "shared-pool".into(), display_name: "Shared Pool".into(),
            description: "Shared skill pool symlinked into multiple platforms (~/.agents/skills)".into(),
            skill_dir: home.join(".agents/skills") },
    ]
}
