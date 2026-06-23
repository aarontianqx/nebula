//! Pulsar Domain 层（纯域，无 IO / UI / 平台依赖）。
//!
//! 提供工具的统一抽象 (`Tool`)、值与参数类型 (`ToolValue` / `ToolParams`)、
//! 静态元数据 (`ToolDescriptor`)，以及具体工具实现 (`tools`)。

pub mod descriptor;
pub mod detect;
pub mod error;
pub mod tool;
pub mod tools;
pub mod value;

pub use descriptor::{Category, ParamKind, ParamSpec, ToolDescriptor};
pub use detect::{Detector, Rule};
pub use error::{ToolError, ToolResult};
pub use tool::Tool;
pub use value::{IoKind, ParamValue, ToolParams, ToolValue};
