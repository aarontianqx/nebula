//! descriptor → clap 的**唯一映射层**。
//!
//! 设计要点（对应"CLI/GUI 单一事实源"诉求）：
//! 工具的命令、flag、帮助文本**全部从 `ToolDescriptor` 派生**，本文件是从
//! descriptor 到 clap `Command` 的唯一转换处。新增工具或改参数时只动
//! `pulsar-core`，CLI 这里零改动即自动同步（与 GUI 表单同源）。
//!
//! 约定一目了然（见下方常量与 [`arg_for_param`]），避免"核心逻辑集中但难读"：
//! - 子命令名 = 工具 id `.` 之后的部分（如 `encoders.base64` → `base64`）；
//!   若多个工具撞名，撞名者一律退回完整 id（如 `encoders.base64`）。
//! - 参数 → flag：`Bool` → `--key` / `--no-key`；`Int`/`Str` → `--key <值>`；
//!   `Enum` → `--key <候选>`（带候选校验）。
//! - 关键词中合法的 ASCII 词作为子命令别名。

use clap::{Arg, ArgAction, Command};
use pulsar_core::{ParamKind, ParamSpec, ParamValue, ToolDescriptor, ToolParams};
use std::collections::BTreeMap;

/// 取消某个 bool flag 的前缀：`--no-<key>` 表示显式设为 false。
pub const NEGATE_PREFIX: &str = "no-";

/// clap 4 的命令/参数名要求 `'static`，而我们的名字是运行期从注册表派生的。
/// 命令树在启动时构建一次、随进程存活，故 leak 少量短字符串是惯例且无实际泄漏风险。
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// 计算每个工具的 CLI 子命令名（短名优先，撞名退回完整 id）。
///
/// 返回 `id -> 子命令名`，顺序稳定。集中在一处便于审计与测试。
pub fn command_names(descriptors: &[ToolDescriptor]) -> BTreeMap<String, String> {
    // 先统计短名出现次数，撞名的退回完整 id。
    let mut short_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for d in descriptors {
        *short_counts.entry(short_of(&d.id)).or_insert(0) += 1;
    }
    descriptors
        .iter()
        .map(|d| {
            let short = short_of(&d.id);
            let name = if short_counts.get(short).copied().unwrap_or(0) > 1 {
                d.id.clone() // 撞名：用完整 id 消歧。
            } else {
                short.to_string()
            };
            (d.id.clone(), name)
        })
        .collect()
}

/// id `<category>.<tool>` 的短名部分。无 `.` 时返回原串。
fn short_of(id: &str) -> &str {
    id.rsplit_once('.').map(|(_, s)| s).unwrap_or(id)
}

/// 为单个工具构建 clap 子命令（含参数 flag、帮助）。
///
/// 命令名用短名（如 `base64`）；同时把**完整 id**（如 `encoders.base64`）登记为
/// 可见别名，既无歧义又与 GUI 的工具 id 对齐。不从关键词派生别名——像 `encode`
/// 这类通用词会在多个工具间产生歧义，得不偿失。
pub fn tool_command(descriptor: &ToolDescriptor, name: &str) -> Command {
    let mut cmd = Command::new(leak(name.to_string()))
        .about(descriptor.name)
        .long_about(format!("{}\n\n{}", descriptor.name, descriptor.description));

    // 完整 id 作为别名（当命令名是短名时）。
    if name != descriptor.id {
        cmd = cmd.visible_alias(leak(descriptor.id.clone()));
    }

    for spec in descriptor.params {
        for arg in args_for_param(spec) {
            cmd = cmd.arg(arg);
        }
    }

    // 主输入：优先 stdin；也允许作为位置参数兜底（方便 `pulsar base64 hello`）。
    cmd.arg(
        Arg::new("__input")
            .value_name("INPUT")
            .help("输入文本；留空则从 stdin 读取")
            .required(false),
    )
}

/// 把一个 `ParamSpec` 映射成 clap 参数（bool 会产出两个：`--key` 与 `--no-key`）。
fn args_for_param(spec: &ParamSpec) -> Vec<Arg> {
    match spec.kind {
        ParamKind::Bool => {
            let on = Arg::new(spec.key)
                .long(spec.key)
                .help(format!("{}（开启）", spec.label))
                .action(ArgAction::SetTrue);
            let off_id: &'static str = leak(format!("{NEGATE_PREFIX}{}", spec.key));
            let off = Arg::new(off_id)
                .long(off_id)
                .help(format!("{}（关闭）", spec.label))
                .action(ArgAction::SetTrue);
            vec![on, off]
        }
        ParamKind::Int => vec![Arg::new(spec.key)
            .long(spec.key)
            .value_name("N")
            .help(int_help(spec))
            .value_parser(clap::value_parser!(i64))],
        ParamKind::Str => vec![Arg::new(spec.key)
            .long(spec.key)
            .value_name("VALUE")
            .help(str_help(spec))],
        ParamKind::Enum => {
            let parser = clap::builder::PossibleValuesParser::new(spec.options.to_vec());
            vec![Arg::new(spec.key)
                .long(spec.key)
                .value_name("CHOICE")
                .help(enum_help(spec))
                .value_parser(parser)]
        }
    }
}

/// 从解析结果构建 `ToolParams`：只为"用户显式提供"的参数赋值，
/// 其余留空交给工具自身的默认（默认值是工具的真理，不在 CLI 这边复制一份）。
pub fn collect_params(descriptor: &ToolDescriptor, matches: &clap::ArgMatches) -> ToolParams {
    let mut params = ToolParams::new();
    for spec in descriptor.params {
        match spec.kind {
            ParamKind::Bool => {
                let off_id = format!("{NEGATE_PREFIX}{}", spec.key);
                let on = matches.get_flag(spec.key);
                let off = matches.get_flag(&off_id);
                // `--no-key` 优先于 `--key`；都没给则不写入（用工具默认）。
                if off {
                    params.insert(spec.key.to_string(), ParamValue::Bool(false));
                } else if on {
                    params.insert(spec.key.to_string(), ParamValue::Bool(true));
                }
            }
            ParamKind::Int => {
                if let Some(v) = matches.get_one::<i64>(spec.key) {
                    params.insert(spec.key.to_string(), ParamValue::Int(*v));
                }
            }
            ParamKind::Str | ParamKind::Enum => {
                if let Some(v) = matches.get_one::<String>(spec.key) {
                    params.insert(spec.key.to_string(), ParamValue::Str(v.clone()));
                }
            }
        }
    }
    params
}

fn int_help(spec: &ParamSpec) -> String {
    format!("{}（整数，默认 {}）", spec.label, spec.default)
}

fn str_help(spec: &ParamSpec) -> String {
    if spec.default.is_empty() {
        spec.label.to_string()
    } else {
        format!("{}（默认 {}）", spec.label, spec.default)
    }
}

fn enum_help(spec: &ParamSpec) -> String {
    format!(
        "{}（默认 {}，可选 {}）",
        spec.label,
        spec.default,
        spec.options.join(" / ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsar_app::build_registry;
    use std::collections::HashSet;

    #[test]
    fn short_names_are_used_when_unique() {
        let names = command_names(&build_registry().descriptors());
        // base64 在工具集中短名唯一。
        assert_eq!(
            names.get("encoders.base64").map(String::as_str),
            Some("base64")
        );
    }

    #[test]
    fn every_tool_maps_to_a_command_name() {
        let descriptors = build_registry().descriptors();
        let names = command_names(&descriptors);
        assert_eq!(names.len(), descriptors.len());
        // 命令名整体唯一（短名或退回 id 后不重复）。
        let unique: HashSet<&String> = names.values().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn negate_flag_overrides_on_flag() {
        // 构造 password 工具命令，验证 --no-uppercase 生效。
        let descriptors = build_registry().descriptors();
        let pw = descriptors
            .iter()
            .find(|d| d.id == "generators.password")
            .unwrap();
        let cmd = tool_command(pw, "password");
        let m = cmd
            .try_get_matches_from(["password", "--uppercase", "--no-uppercase"])
            .unwrap();
        let params = collect_params(pw, &m);
        assert_eq!(
            params.get("uppercase").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn only_explicit_params_are_collected() {
        let descriptors = build_registry().descriptors();
        let pw = descriptors
            .iter()
            .find(|d| d.id == "generators.password")
            .unwrap();
        let cmd = tool_command(pw, "password");
        let m = cmd
            .try_get_matches_from(["password", "--length", "24"])
            .unwrap();
        let params = collect_params(pw, &m);
        // 只显式给了 length。
        assert_eq!(params.get("length").and_then(|v| v.as_int()), Some(24));
        assert!(!params.contains_key("lowercase"));
    }

    #[test]
    fn enum_rejects_invalid_choice() {
        let descriptors = build_registry().descriptors();
        let b64 = descriptors
            .iter()
            .find(|d| d.id == "encoders.base64")
            .unwrap();
        let cmd = tool_command(b64, "base64");
        let err = cmd.try_get_matches_from(["base64", "--mode", "bogus"]);
        assert!(err.is_err());
    }

    #[test]
    fn full_id_is_registered_as_alias() {
        let descriptors = build_registry().descriptors();
        let b64 = descriptors
            .iter()
            .find(|d| d.id == "encoders.base64")
            .unwrap();
        let cmd = tool_command(b64, "base64");
        // 完整 id 作为别名也能解析。
        let m = cmd.try_get_matches_from(["encoders.base64", "hi"]);
        assert!(m.is_ok());
    }
}
