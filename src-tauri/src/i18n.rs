#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

impl Locale {
    pub fn tag(&self) -> &'static str {
        match self {
            Locale::ZhCn => "zh-CN",
            Locale::En => "en",
        }
    }

    pub fn detect() -> Self {
        let locale_str = sys_locale::get_locale().unwrap_or_default();
        if locale_str.starts_with("zh") { Locale::ZhCn } else { Locale::En }
    }

    pub fn from_config(config_val: Option<&str>) -> Self {
        config_val
            .and_then(|v| match v {
                "zh-CN" | "zh_CN" | "zh" => Some(Locale::ZhCn),
                _ => None,
            })
            .unwrap_or_else(Self::detect)
    }
}
