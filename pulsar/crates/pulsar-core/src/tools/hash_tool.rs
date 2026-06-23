//! 哈希生成器（`generators.hash`）。
//!
//! 对输入文本计算 MD5 / SHA-1 / SHA-256 / SHA-512 / CRC32，一次性输出全部。

use crate::descriptor::{Category, ToolDescriptor};
use crate::error::ToolResult;
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::sync::OnceLock;

pub struct HashTool;

impl Tool for HashTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "generators.hash".to_string(),
            category: Category::Generators,
            name: "哈希生成",
            description: "对文本计算 MD5 / SHA-1 / SHA-256 / SHA-512 / CRC32。",
            keywords: &[
                "hash", "md5", "sha", "sha256", "crc32", "checksum", "哈希", "校验",
            ],
            params: &[],
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, _params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let bytes = text.as_bytes();

        let md5 = hex::encode(Md5::digest(bytes));
        let sha1 = hex::encode(Sha1::digest(bytes));
        let sha256 = hex::encode(Sha256::digest(bytes));
        let sha512 = hex::encode(Sha512::digest(bytes));
        let crc32 = format!("{:08x}", crc32fast::hash(bytes));

        let out = format!(
            "MD5:    {md5}\nSHA-1:  {sha1}\nSHA-256: {sha256}\nSHA-512: {sha512}\nCRC32:  {crc32}",
        );
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors_for_abc() {
        let out = HashTool
            .run(ToolValue::text("abc"), &ToolParams::new())
            .unwrap();
        let text = out.as_text();
        // 已知 "abc" 的标准散列值。
        assert!(text.contains("MD5:    900150983cd24fb0d6963f7d28e17f72"));
        assert!(text.contains("SHA-1:  a9993e364706816aba3e25717850c26c9cd0d89d"));
        assert!(text
            .contains("SHA-256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert!(text.contains("CRC32:  352441c2"));
    }

    #[test]
    fn empty_input_is_ok() {
        let out = HashTool
            .run(ToolValue::text(""), &ToolParams::new())
            .unwrap();
        // 空串的 MD5。
        assert!(out.as_text().contains("d41d8cd98f00b204e9800998ecf8427e"));
    }
}
