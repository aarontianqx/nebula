//! `Tool` trait —— Pulsar 所有工具的统一抽象。
//!
//! 每个工具是一个纯函数对象：输入 `ToolValue` + 参数，输出 `ToolValue`。
//! 无 IO 副作用（文件 / 剪贴板由上层喂入），便于测试、复用与（未来）流式处理。

use crate::descriptor::ToolDescriptor;
use crate::error::ToolResult;
use crate::value::{ToolParams, ToolValue};

pub trait Tool: Send + Sync {
    /// 静态元数据。
    fn descriptor(&self) -> &ToolDescriptor;

    /// 纯执行。
    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult;
}
