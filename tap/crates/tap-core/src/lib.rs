//! tap-core: the macro domain model. Pure data and rules: no UI, no OS, no threads.
//!
//! A [`MacroDocument`] holds [`DslAction`]s that may contain `{{ }}` templates; the Resolve stage
//! (in `tap-application`) turns each into a concrete [`Action`] just before it runs.

mod action;
mod condition;
mod document;
mod dsl;
mod expression;
mod keys;
mod schema;
mod variables;

pub use action::{Action, MouseButton, Point};
pub use condition::{
    Color, CompareOp, Condition, ConditionEvaluator, ConditionResult, WaitUntilConfig,
};
pub use document::{
    document_to_yaml, parse_yaml, CoordinateSpace, DslRunConfig, DslTargetWindow, DslTimedAction,
    Humanize, MacroDocument, VariableDefinition, VariableType, YamlError, DSL_VERSION,
    MAX_HUMANIZE_DELAY_MS, MAX_HUMANIZE_POSITION_PX,
};
pub use dsl::{DslAction, DslCondition, DslMouseButton, DslValue, InvalidCondition};
pub use expression::{
    create_expression_engine, resolve_expressions, resolve_typed_value, ExpressionEngine,
    ExpressionEngineHandle, ExpressionError, ExpressionResult,
};
pub use keys::normalize_key;
pub use schema::{validate_document, ValidationError, ValidationResult};
pub use variables::{VariableStore, VariableValue};
