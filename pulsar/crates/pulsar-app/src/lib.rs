//! Pulsar Application 层。
//!
//! 当前提供工具注册表 (`ToolRegistry`)。后续阶段在此扩展 Pipeline 执行器、
//! Smart Detection、工作流与剪贴板监听（见 `specs/proposals/architecture.md`）。

pub mod registry;

pub use registry::{build_registry, DetectionResult, ToolRegistry};
