//! HMAC 生成（`generators.hmac`）。
//!
//! 用给定密钥对输入文本计算 HMAC，支持 SHA-1 / SHA-256 / SHA-512，输出十六进制摘要。
//! 密钥经 `key` 参数传入（UTF-8 字节）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::sync::OnceLock;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

const KEY: &str = "key";
const ALGO: &str = "algo";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::string(KEY, "密钥", ""),
    ParamSpec::enumerated(ALGO, "算法", "sha256", &["sha1", "sha256", "sha512"]),
];

pub struct HmacTool;

impl Tool for HmacTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "generators.hmac".to_string(),
            category: Category::Generators,
            name: "HMAC",
            description: "用密钥对文本计算 HMAC（SHA-1 / SHA-256 / SHA-512），输出 hex。",
            keywords: &["hmac", "签名", "mac", "sha256", "密钥"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let key = params.get(KEY).and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() {
            return Err(ToolError::InvalidParam {
                name: KEY.to_string(),
                reason: "HMAC 需要一个非空密钥".to_string(),
            });
        }
        let algo = params
            .get(ALGO)
            .and_then(|v| v.as_str())
            .unwrap_or("sha256");
        let msg = input.as_text();
        let bytes = msg.as_bytes();
        let key_bytes = key.as_bytes();

        // `new_from_slice` 接受任意长度密钥，故 expect 不会触发。
        let digest = match algo {
            "sha1" => {
                let mut mac = HmacSha1::new_from_slice(key_bytes).expect("any key length");
                mac.update(bytes);
                hex::encode(mac.finalize().into_bytes())
            }
            "sha256" => {
                let mut mac = HmacSha256::new_from_slice(key_bytes).expect("any key length");
                mac.update(bytes);
                hex::encode(mac.finalize().into_bytes())
            }
            "sha512" => {
                let mut mac = HmacSha512::new_from_slice(key_bytes).expect("any key length");
                mac.update(bytes);
                hex::encode(mac.finalize().into_bytes())
            }
            other => {
                return Err(ToolError::InvalidParam {
                    name: ALGO.to_string(),
                    reason: format!("未知算法 '{other}'"),
                })
            }
        };
        Ok(ToolValue::text(digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn hmac(input: &str, key: &str, algo: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(KEY.to_string(), ParamValue::Str(key.to_string()));
        p.insert(ALGO.to_string(), ParamValue::Str(algo.to_string()));
        HmacTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn hmac_sha256_known_vector() {
        // RFC 4231 不直接给这个，但与常见在线工具一致的已知向量。
        // key="key", msg="The quick brown fox jumps over the lazy dog"
        let out = hmac(
            "The quick brown fox jumps over the lazy dog",
            "key",
            "sha256",
        );
        assert_eq!(
            out,
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn hmac_sha1_known_vector() {
        let out = hmac("The quick brown fox jumps over the lazy dog", "key", "sha1");
        assert_eq!(out, "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9");
    }

    #[test]
    fn different_keys_differ() {
        let a = hmac("msg", "k1", "sha256");
        let b = hmac("msg", "k2", "sha256");
        assert_ne!(a, b);
    }

    #[test]
    fn empty_key_errors() {
        let mut p = ToolParams::new();
        p.insert(KEY.to_string(), ParamValue::Str(String::new()));
        let err = HmacTool.run(ToolValue::text("msg"), &p).unwrap_err();
        assert!(matches!(err, ToolError::InvalidParam { .. }));
    }
}
