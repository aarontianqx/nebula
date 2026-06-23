//! Smart Detection 规则。
//!
//! 每个工具在其 `ToolDescriptor` 上声明若干 [`Detector`] 规则（轻量启发式）。
//! 上层 (`pulsar-app::SmartDetector`) 对输入逐条匹配，按置信度排序给出候选工具。
//!
//! 规则保持声明式且 `const` 友好，匹配逻辑集中在 [`Detector::matches`]，
//! 性能上对长输入只看开头/整体廉价判断（见各分支注释）。

/// 单条检测规则及其命中时的置信度（0–100）。
#[derive(Debug, Clone, Copy)]
pub struct Detector {
    pub rule: Rule,
    pub confidence: u8,
}

impl Detector {
    pub const fn new(rule: Rule, confidence: u8) -> Self {
        Detector { rule, confidence }
    }
}

/// 声明式匹配规则。
#[derive(Debug, Clone, Copy)]
pub enum Rule {
    /// 去空白后能被解析为 JSON。
    JsonParse,
    /// 去空白后是合法整数（用于时间戳/进制等）。
    Integer,
    /// 形如 a.b.c 的三段、且每段是 base64url 的 JWT 形状。
    JwtShape,
    /// 匹配给定正则（锚点由模式自行决定）。
    Regex(&'static str),
    /// 整体由给定字符集组成（去空白后非空）。例如 hex、base64 字符集。
    CharsetOnly(&'static str),
}

impl Rule {
    /// 判断输入是否命中该规则。注意：输入可能很长，分支需保持廉价。
    pub fn matches(&self, input: &str) -> bool {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return false;
        }
        match self {
            Rule::JsonParse => {
                let head = trimmed.as_bytes()[0];
                // 先廉价排除：JSON 文档通常以这些字符开头。
                if !matches!(head, b'{' | b'[' | b'"' | b't' | b'f' | b'n' | b'-')
                    && !head.is_ascii_digit()
                {
                    return false;
                }
                serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
            }
            Rule::Integer => trimmed.parse::<i128>().is_ok(),
            Rule::JwtShape => {
                let parts: Vec<&str> = trimmed.split('.').collect();
                parts.len() == 3
                    && parts[0].starts_with("eyJ")
                    && parts
                        .iter()
                        .take(2)
                        .all(|p| !p.is_empty() && p.bytes().all(is_base64url_byte))
            }
            Rule::Regex(pattern) => regex::Regex::new(pattern)
                .map(|re| re.is_match(trimmed))
                .unwrap_or(false),
            Rule::CharsetOnly(charset) => {
                let cleaned: Vec<char> = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
                !cleaned.is_empty() && cleaned.iter().all(|c| charset.contains(*c))
            }
        }
    }
}

fn is_base64url_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'='
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_parse_rule() {
        assert!(Rule::JsonParse.matches(r#"{"a":1}"#));
        assert!(Rule::JsonParse.matches("[1,2,3]"));
        assert!(!Rule::JsonParse.matches("hello"));
        assert!(!Rule::JsonParse.matches(""));
    }

    #[test]
    fn integer_rule() {
        assert!(Rule::Integer.matches("1609459200"));
        assert!(Rule::Integer.matches("  -42 "));
        assert!(!Rule::Integer.matches("3.14"));
        assert!(!Rule::Integer.matches("0xFF"));
    }

    #[test]
    fn jwt_shape_rule() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.sig";
        assert!(Rule::JwtShape.matches(jwt));
        assert!(!Rule::JwtShape.matches("a.b.c"));
        assert!(!Rule::JwtShape.matches("eyJ.only-two"));
    }

    #[test]
    fn charset_only_rule() {
        assert!(Rule::CharsetOnly("0123456789abcdefABCDEF").matches("deadBEEF"));
        assert!(!Rule::CharsetOnly("0123456789abcdefABCDEF").matches("xyz"));
    }

    #[test]
    fn regex_rule() {
        assert!(Rule::Regex(r"^#[0-9a-fA-F]{6}$").matches("#1a2b3c"));
        assert!(!Rule::Regex(r"^#[0-9a-fA-F]{6}$").matches("blue"));
    }
}
