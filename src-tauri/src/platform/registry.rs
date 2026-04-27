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
        PlatformDef { id: "claude-code".into(), display_name: "Claude Code".into(), description: String::new(), skill_dir: home.join(".claude/skills") },
        PlatformDef { id: "codex-cli".into(), display_name: "Codex CLI".into(), description: String::new(), skill_dir: home.join(".codex/skills") },
        PlatformDef { id: "cursor".into(), display_name: "Cursor".into(), description: String::new(), skill_dir: home.join(".cursor/skills-cursor") },
        PlatformDef { id: "openclaw".into(), display_name: "OpenClaw".into(), description: String::new(), skill_dir: home.join(".openclaw/skills") },
        PlatformDef { id: "hermes".into(), display_name: "Hermes".into(), description: String::new(), skill_dir: home.join(".hermes/skills") },
        PlatformDef { id: "trae".into(), display_name: "Trae".into(), description: String::new(), skill_dir: home.join(".trae/skills") },
        PlatformDef { id: "kiro".into(), display_name: "Kiro".into(), description: String::new(), skill_dir: home.join(".kiro/skills") },
        PlatformDef { id: "shared-pool".into(), display_name: "Shared Pool".into(),
            description: "Shared skill pool symlinked into multiple platforms (~/.agents/skills)".into(),
            skill_dir: home.join(".agents/skills") },
    ]
}
