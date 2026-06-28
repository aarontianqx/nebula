//! Pulsar CLI 适配层（Adapter）。
//!
//! 形态："薄壳" —— 解析参数 → 调 `pulsar-app` 注册表 → 打印结果。所有工具逻辑
//! 都在 `pulsar-core`，与 GUI 共享同一份；本 crate 不含任何"每工具一份"的代码，
//! 子命令由 [`dispatch`] 从注册表的 `ToolDescriptor` **动态生成**。
//!
//! I/O 契约（对脚本 / CI / agent 友好）：
//! - 主输入：stdin（管道）；也可作为位置参数 `pulsar <tool> <INPUT>`。
//! - 结果 → stdout；错误 → stderr，并以**非零退出码**结束。
//! - `pulsar list [--json]` 列出全部工具；`pulsar detect [--json]` 智能识别。

mod dispatch;

use clap::{Arg, ArgAction, ArgMatches, Command};
use clap_complete::{generate, Shell};
use pulsar_app::{build_registry, ToolRegistry};
use pulsar_core::{ToolDescriptor, ToolValue};
use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Write};
use std::process::ExitCode;

/// 保留给内置子命令，工具不得占用（撞名则退回完整 id）。
const RESERVED: &[&str] = &["list", "detect", "completions", "help"];

fn main() -> ExitCode {
    let registry = build_registry();
    let descriptors = registry.descriptors();
    let names = resolve_names(&descriptors);

    let cmd = build_cli(&descriptors, &names);
    let matches = cmd.get_matches();

    match matches.subcommand() {
        Some(("list", sub)) => cmd_list(&descriptors, sub),
        Some(("detect", sub)) => cmd_detect(&registry, sub),
        Some(("completions", sub)) => cmd_completions(&descriptors, &names, sub),
        Some((name, sub)) => cmd_run(&registry, &descriptors, &names, name, sub),
        None => {
            // 无子命令：打印帮助（已由 arg_required_else_help 兜底，这里防御）。
            eprintln!("用法：pulsar <工具|list|detect> [选项]，见 `pulsar --help`");
            ExitCode::FAILURE
        }
    }
}

/// 工具 id → 子命令名（短名优先；与保留字撞名时退回完整 id）。
fn resolve_names(descriptors: &[ToolDescriptor]) -> BTreeMap<String, String> {
    let mut names = dispatch::command_names(descriptors);
    for (id, name) in names.iter_mut() {
        if RESERVED.contains(&name.as_str()) {
            *name = id.clone();
        }
    }
    names
}

/// 组装顶层命令树：内置 `list` / `detect` + 每个工具一个动态子命令。
fn build_cli(descriptors: &[ToolDescriptor], names: &BTreeMap<String, String>) -> Command {
    let mut cli = Command::new("pulsar")
        .about("Pulsar —— 开发者本地工具箱（CLI）")
        .long_about(
            "在终端 / 脚本 / CI 中运行 Pulsar 的工具。\n\
             工具子命令与 GUI 同源（均由工具注册表派生）。\n\n\
             示例：\n  \
             echo aGVsbG8= | pulsar base64 --mode decode\n  \
             cat data.json | pulsar json --pretty\n  \
             pulsar uuid --count 5\n  \
             pulsar list",
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("list").about("列出全部工具").arg(json_flag()))
        .subcommand(
            Command::new("detect")
                .about("智能识别输入内容并推荐工具")
                .arg(json_flag())
                .arg(
                    Arg::new("__input")
                        .value_name("INPUT")
                        .help("待识别文本；留空则从 stdin 读取")
                        .required(false),
                ),
        )
        .subcommand(
            Command::new("completions")
                .about("生成 shell 自动补全脚本（写入补全目录或 source 之）")
                .long_about(
                    "为指定 shell 生成补全脚本。脚本含全部工具子命令与 flag。\n\
                     新增工具后重新生成一次即可。\n\n\
                     例：\n  \
                     pulsar completions zsh  > ~/.zfunc/_pulsar\n  \
                     pulsar completions bash > /usr/local/etc/bash_completion.d/pulsar",
                )
                .arg(
                    Arg::new("shell")
                        .value_name("SHELL")
                        .required(true)
                        .help("目标 shell")
                        .value_parser(clap::value_parser!(Shell)),
                ),
        );

    // 工具子命令（按命令名稳定排序，帮助里更易读）。
    let by_name: BTreeMap<&str, &ToolDescriptor> = descriptors
        .iter()
        .map(|d| (names[&d.id].as_str(), d))
        .collect();
    for (name, descriptor) in by_name {
        cli = cli.subcommand(dispatch::tool_command(descriptor, name));
    }
    cli
}

fn json_flag() -> Arg {
    Arg::new("json")
        .long("json")
        .help("以 JSON 输出（便于脚本 / agent 消费）")
        .action(ArgAction::SetTrue)
}

// ── 子命令实现 ──────────────────────────────────────────────

fn cmd_run(
    registry: &ToolRegistry,
    descriptors: &[ToolDescriptor],
    names: &BTreeMap<String, String>,
    name: &str,
    sub: &ArgMatches,
) -> ExitCode {
    // 反查命令名对应的工具 id。
    let id = match names.iter().find(|(_, n)| n.as_str() == name) {
        Some((id, _)) => id.clone(),
        None => {
            eprintln!("未知工具：{name}");
            return ExitCode::FAILURE;
        }
    };
    let descriptor = descriptors.iter().find(|d| d.id == id).expect("descriptor");

    let input = match read_input(sub) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取输入失败：{e}");
            return ExitCode::FAILURE;
        }
    };

    let params = dispatch::collect_params(descriptor, sub);
    match registry.run(&id, ToolValue::text(input), &params) {
        Ok(out) => {
            print_stdout(&out.as_text());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_list(descriptors: &[ToolDescriptor], sub: &ArgMatches) -> ExitCode {
    if sub.get_flag("json") {
        // 直接复用 descriptor 的 Serialize（与前端 list_tools 同源数据）。
        match serde_json::to_string_pretty(descriptors) {
            Ok(s) => {
                print_stdout(&s);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("序列化失败：{e}");
                ExitCode::FAILURE
            }
        }
    } else {
        // 人类可读：按分类分组。
        let mut by_cat: BTreeMap<&str, Vec<&ToolDescriptor>> = BTreeMap::new();
        for d in descriptors {
            by_cat.entry(d.category.label()).or_default().push(d);
        }
        let mut out = String::new();
        for (cat, tools) in &by_cat {
            out.push_str(&format!("\n{cat}\n"));
            for d in tools {
                out.push_str(&format!("  {:<22} {}\n", d.id, d.name));
            }
        }
        out.push_str(&format!("\n共 {} 个工具\n", descriptors.len()));
        print_stdout(&out);
        ExitCode::SUCCESS
    }
}

fn cmd_detect(registry: &ToolRegistry, sub: &ArgMatches) -> ExitCode {
    let input = match read_input(sub) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取输入失败：{e}");
            return ExitCode::FAILURE;
        }
    };
    let results = registry.detect(&input);
    if sub.get_flag("json") {
        match serde_json::to_string_pretty(&results) {
            Ok(s) => {
                print_stdout(&s);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("序列化失败：{e}");
                ExitCode::FAILURE
            }
        }
    } else if results.is_empty() {
        eprintln!("未识别出候选工具");
        ExitCode::SUCCESS
    } else {
        let mut out = String::new();
        for r in &results {
            out.push_str(&format!(
                "{:>3}%  {:<22} {}\n",
                r.confidence, r.tool_id, r.tool_name
            ));
        }
        print_stdout(&out);
        ExitCode::SUCCESS
    }
}

fn cmd_completions(
    descriptors: &[ToolDescriptor],
    names: &BTreeMap<String, String>,
    sub: &ArgMatches,
) -> ExitCode {
    let shell = *sub.get_one::<Shell>("shell").expect("required arg");
    // 用同一棵动态命令树渲染补全——补全里自然含全部工具与 flag（单一事实源）。
    let mut cmd = build_cli(descriptors, names);
    let bin = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin, &mut std::io::stdout());
    ExitCode::SUCCESS
}

// ── I/O 辅助 ────────────────────────────────────────────────

/// 取主输入：位置参数优先；否则在 stdin 为管道/重定向时读取它；
/// 若 stdin 是终端（交互态）则视为空输入——避免无管道运行生成类工具时卡住等待 EOF。
fn read_input(sub: &ArgMatches) -> std::io::Result<String> {
    if let Some(arg) = sub.get_one::<String>("__input") {
        return Ok(arg.clone());
    }
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(String::new());
    }
    let mut buf = String::new();
    stdin.lock().read_to_string(&mut buf)?;
    Ok(buf)
}

/// 打印到 stdout：补足结尾换行（终端友好），但不重复添加。
fn print_stdout(s: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(s.as_bytes());
    if !s.ends_with('\n') {
        let _ = stdout.write_all(b"\n");
    }
}
