use std::{
    borrow::Cow,
    sync::atomic::{AtomicU8, Ordering},
};

use crate::KanbanError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    ZhCn,
    En,
}

static CURRENT_LOCALE: AtomicU8 = AtomicU8::new(Locale::ZhCn as u8);

impl Locale {
    pub const DEFAULT: Self = Self::ZhCn;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
        if normalized.is_empty() || normalized == "auto" || normalized == "system" {
            return None;
        }
        if normalized == "zh" || normalized.starts_with("zh-") {
            return Some(Self::ZhCn);
        }
        if normalized == "en" || normalized.starts_with("en-") {
            return Some(Self::En);
        }
        None
    }

    pub fn explicit_or_system(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            Some("auto") | Some("system") => Ok(Self::system()),
            Some(raw) => Self::parse(raw).ok_or_else(|| unsupported_locale(raw)),
            None => Ok(Self::DEFAULT),
        }
    }

    pub fn system() -> Self {
        Self::from_system_locale_values(
            std::env::var("LC_ALL").ok().as_deref(),
            std::env::var("LC_MESSAGES").ok().as_deref(),
            std::env::var("LANG").ok().as_deref(),
        )
    }

    pub fn from_system_locale_values(
        lc_all: Option<&str>,
        lc_messages: Option<&str>,
        lang: Option<&str>,
    ) -> Self {
        [lc_all, lc_messages, lang]
            .into_iter()
            .flatten()
            .find_map(Self::parse)
            .unwrap_or(Self::DEFAULT)
    }

    pub fn from_accept_language(header: Option<&str>, fallback: Self) -> Self {
        let Some(header) = header else {
            return fallback;
        };
        parse_accept_language(header).unwrap_or(fallback)
    }
}

pub fn current_locale() -> Locale {
    match CURRENT_LOCALE.load(Ordering::Relaxed) {
        value if value == Locale::En as u8 => Locale::En,
        _ => Locale::ZhCn,
    }
}

pub fn set_current_locale(locale: Locale) {
    CURRENT_LOCALE.store(locale as u8, Ordering::Relaxed);
}

pub fn unsupported_locale(value: &str) -> String {
    format!("unsupported locale: {value}; expected auto, zh-CN, or en")
}

pub fn dep_added(locale: Locale, parent_ref: &str, child_ref: &str) -> String {
    match locale {
        Locale::ZhCn => format!("已添加依赖：{parent_ref} -> {child_ref}"),
        Locale::En => format!("Added dependency: {parent_ref} -> {child_ref}"),
    }
}

pub fn dep_removed(locale: Locale, parent_ref: &str, child_ref: &str) -> String {
    match locale {
        Locale::ZhCn => format!("已移除依赖：{parent_ref} -> {child_ref}"),
        Locale::En => format!("Removed dependency: {parent_ref} -> {child_ref}"),
    }
}

pub fn render_error(locale: Locale, error: &KanbanError) -> String {
    match locale {
        Locale::En => error.to_string(),
        Locale::ZhCn => render_error_zh(error),
    }
}

fn render_error_zh(error: &KanbanError) -> String {
    match error {
        KanbanError::InvalidStatus(detail) => format!("无效状态：{}", detail_zh(detail)),
        KanbanError::InvalidTransition(detail) => {
            format!("非法状态转换：{}", detail_zh(detail))
        }
        KanbanError::ExecutionPlanRequired(detail) => {
            format!("需要执行计划：{}", detail_zh(detail))
        }
        KanbanError::StepsIncomplete(detail) => format!("步骤未完成：{}", detail_zh(detail)),
        KanbanError::InvalidInput(detail) => format!("输入无效：{}", detail_zh(detail)),
        KanbanError::Conflict(detail) => format!("冲突：{}", detail_zh(detail)),
        KanbanError::IdempotencyConflict(detail) => {
            format!("幂等冲突：{}", detail_zh(detail))
        }
        KanbanError::FeatureNotAvailable(detail) => {
            format!("功能尚不可用：{}", detail_zh(detail))
        }
        KanbanError::NotFound(detail) => format!("未找到：{}", detail_zh(detail)),
        KanbanError::Storage(detail) => format!("存储错误：{}", detail_zh(detail)),
    }
}

fn detail_zh(detail: &str) -> Cow<'_, str> {
    if let Some(max) = detail.strip_prefix("limit must be <= ") {
        return Cow::Owned(format!("limit 必须小于等于 {max}"));
    }
    if let Some(max) = detail.strip_prefix("offset must be <= ") {
        return Cow::Owned(format!("offset 必须小于等于 {max}"));
    }
    if let Some(value) = detail.strip_prefix("unsupported sort: ") {
        return Cow::Owned(format!("不支持的排序：{value}"));
    }
    if let Some(value) = detail.strip_prefix("unsupported task list sort: ") {
        return Cow::Owned(format!("不支持的任务列表排序：{value}"));
    }
    if let Some(value) = detail.strip_prefix("invalid priority filter: ") {
        return Cow::Owned(format!("无效 priority 过滤值：{value}"));
    }
    if let Some(path) = detail.strip_prefix("database file is missing: ") {
        return Cow::Owned(format!("数据库文件不存在：{path}"));
    }
    Cow::Borrowed(detail)
}

fn parse_accept_language(header: &str) -> Option<Locale> {
    header
        .split(',')
        .enumerate()
        .filter_map(|(index, part)| {
            let mut sections = part.trim().split(';');
            let tag = sections.next()?.trim();
            let q = sections
                .find_map(|section| {
                    section
                        .trim()
                        .strip_prefix("q=")
                        .and_then(|raw| raw.parse::<f32>().ok())
                })
                .unwrap_or(1.0);
            if q <= 0.0 {
                return None;
            }
            Locale::parse(tag).map(|locale| (locale, q, index))
        })
        .max_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.2.cmp(&left.2))
        })
        .map(|(locale, _, _)| locale)
}

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn locale_parse_accepts_supported_language_tags() {
        assert_eq!(Locale::parse("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse("zh_Hans_CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse("en-US"), Some(Locale::En));
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("fr-FR"), None);
        assert_eq!(Locale::parse("auto"), None);
    }

    #[test]
    fn locale_system_values_use_precedence_and_default_to_chinese() {
        assert_eq!(
            Locale::from_system_locale_values(Some("en_US.UTF-8"), Some("zh_CN"), None),
            Locale::En
        );
        assert_eq!(
            Locale::from_system_locale_values(None, Some("zh_CN.UTF-8"), Some("en_US")),
            Locale::ZhCn
        );
        assert_eq!(
            Locale::from_system_locale_values(None, None, Some("C")),
            Locale::ZhCn
        );
    }

    #[test]
    fn accept_language_uses_best_supported_q_value() {
        assert_eq!(
            Locale::from_accept_language(Some("en-US,en;q=0.8,zh-CN;q=0.2"), Locale::ZhCn),
            Locale::En
        );
        assert_eq!(
            Locale::from_accept_language(Some("fr-FR,zh-CN;q=0.9,en;q=0.1"), Locale::En),
            Locale::ZhCn
        );
        assert_eq!(
            Locale::from_accept_language(Some("fr-FR"), Locale::En),
            Locale::En
        );
    }

    #[test]
    fn accept_language_ignores_zero_q_values() {
        assert_eq!(
            Locale::from_accept_language(Some("en;q=0,zh-CN;q=0.5"), Locale::En),
            Locale::ZhCn
        );
        assert_eq!(
            Locale::from_accept_language(Some("zh-CN;q=0,en;q=0"), Locale::En),
            Locale::En
        );
    }

    #[test]
    fn accept_language_preserves_header_order_when_q_ties() {
        assert_eq!(
            Locale::from_accept_language(Some("zh-CN;q=0.7,en;q=0.7"), Locale::En),
            Locale::ZhCn
        );
        assert_eq!(
            Locale::from_accept_language(Some("en;q=0.7,zh-CN;q=0.7"), Locale::ZhCn),
            Locale::En
        );
    }
}
