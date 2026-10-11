//! Only explicit, complete user-chat statements are durable memory sources.
pub fn extract(text: &str) -> Option<(String, String)> {
    let t = text.trim().trim_end_matches(['。', '！', '!']);
    if ["吗", "呢", "吧"].iter().any(|x| t.ends_with(x))
        || t.contains(['?', '？', '\n'])
        || [
            "今天", "假如", "如果", "朋友", "他说", "她说", "比如", "例如",
        ]
        .iter()
        .any(|s| t.contains(s))
    {
        return None;
    }
    let name_patterns = [
        r"^(?:以后|今后|从现在起)(?:请)?(?:叫我|称呼我|喊我)([\p{L}\p{N}·_-]{1,16})$",
        r"^我喜欢你叫我([\p{L}\p{N}·_-]{1,16})$",
        r"^我希望你(?:以后|今后)(?:叫我|称呼我|喊我)([\p{L}\p{N}·_-]{1,16})$",
        r"^请记住我的称呼是([\p{L}\p{N}·_-]{1,16})$",
        r"^(?:以后|今后)(?:改)?叫我([\p{L}\p{N}·_-]{1,16})[，,](?:不叫|别再叫)[\p{L}\p{N}·_-]{1,16}了?$",
    ];
    for pattern in name_patterns {
        let re = regex::Regex::new(pattern).expect("fixed expression");
        if let Some(c) = re.captures(t) {
            let name = c[1].to_string();
            if !["吗", "呢", "吧", "好不好", "行不行"]
                .iter()
                .any(|x| name.ends_with(x))
            {
                return Some(("owner_name".into(), name));
            }
        }
    }
    if [
        "我不喜欢你太频繁说话",
        "我平时喜欢安静一点",
        "以后说话安静一点",
        "以后请少打扰我",
    ]
    .contains(&t)
    {
        return Some(("companion_style".into(), "quiet".into()));
    }
    for (key, phrases) in [
        ("brief", &["简短一点", "简短一些", "短一点"][..]),
        ("gentle", &["温柔一点", "温柔一些"][..]),
        ("playful", &["活泼一点", "活泼一些"][..]),
    ] {
        for prefix in ["以后说话", "以后请说话", "我希望你以后说话"] {
            if phrases.iter().any(|p| t == format!("{prefix}{p}")) {
                return Some(("speaking_style".into(), key.into()));
            }
        }
    }
    if let Some(c) =
        regex::Regex::new(r#"^我(?:平时|一直|通常)?(不喜欢|喜欢)([^，,。！!“”"？?]{1,40})$"#)
            .expect("fixed expression")
            .captures(t)
    {
        let topic = c[2].trim();
        if ![
            "今天", "现在", "最近", "这次", "暂时", "以后", "不要", "你", "他", "她", "我", "这个",
            "那个",
        ]
        .iter()
        .any(|s| topic.contains(s))
        {
            return Some((format!("like:{topic}"), c[1].to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_and_conflicting_preferences() {
        for (text, value) in [
            ("我喜欢你叫我小王。", "小王"),
            ("以后叫我老王，不叫小王了", "老王"),
        ] {
            assert_eq!(extract(text), Some(("owner_name".into(), value.into())));
        }
        assert_eq!(
            extract("我平时喜欢安静一点"),
            Some(("companion_style".into(), "quiet".into()))
        );
    }
    #[test]
    fn personal_likes_and_dislikes_have_same_conflict_key() {
        assert_eq!(
            extract("我喜欢喝咖啡。"),
            Some(("like:喝咖啡".into(), "喜欢".into()))
        );
        assert_eq!(
            extract("我不喜欢喝咖啡。"),
            Some(("like:喝咖啡".into(), "不喜欢".into()))
        );
        for t in [
            "我最近喜欢喝咖啡",
            "我喜欢喝咖啡吗",
            "我喜欢他喝咖啡",
            "我喜欢“喝咖啡”这句台词",
            "今天我喜欢喝咖啡",
        ] {
            assert_eq!(extract(t), None, "{t}");
        }
    }
    #[test]
    fn never_infer_quoted_third_party_temporary_or_negated_names() {
        for t in [
            "他说以后叫我小王",
            "‘以后叫我小王’",
            "我不喜欢你叫我小王",
            "如果以后叫我小王",
            "今天叫我小王",
            "以后叫我小王吗",
            "朋友喜欢你温柔一点",
            "我今天想喝咖啡",
            "帮我总结：以后叫我小王",
        ] {
            assert_eq!(extract(t), None, "{t}");
        }
    }
}
