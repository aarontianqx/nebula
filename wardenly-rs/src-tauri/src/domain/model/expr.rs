//! Simple expression evaluator for OCR conditions.
//!
//! Supports basic comparison and logical operators:
//! - Comparisons: >, >=, <, <=, ==, !=
//! - Logical: &&, ||, and, or
//! - Variables: used, total (for ratio mode)

use std::collections::HashMap;
use thiserror::Error;

/// Expression evaluation error
#[derive(Debug, Error)]
pub enum ExprError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unknown variable: {0}")]
    UnknownVariable(String),

    #[error("Invalid operator: {0}")]
    InvalidOperator(String),
}

/// Expression evaluation context with variables
#[derive(Debug, Default)]
pub struct ExprContext {
    variables: HashMap<String, i64>,
}

impl ExprContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variable value
    pub fn set(&mut self, name: &str, value: i64) {
        self.variables.insert(name.to_string(), value);
    }

    /// Evaluate a boolean expression
    ///
    /// Supports expressions like:
    /// - "used > 7"
    /// - "used > total"
    /// - "used >= 10 || used > total"
    /// - "used > 5 && total < 100"
    pub fn evaluate(&self, expr: &str) -> Result<bool, ExprError> {
        let expr = expr.trim();

        // Handle logical OR (lowest precedence)
        if let Some(result) = self.try_split_logical(expr, &["||", " or "])? {
            return Ok(result.0 || result.1);
        }

        // Handle logical AND
        if let Some(result) = self.try_split_logical(expr, &["&&", " and "])? {
            return Ok(result.0 && result.1);
        }

        // Parse as comparison expression
        self.evaluate_comparison(expr)
    }

    /// Try to split expression by logical operators
    fn try_split_logical(
        &self,
        expr: &str,
        operators: &[&str],
    ) -> Result<Option<(bool, bool)>, ExprError> {
        for op in operators {
            // Find the operator, avoiding splits inside parentheses
            if let Some(pos) = self.find_operator_position(expr, op) {
                let left = expr[..pos].trim();
                let right = expr[pos + op.len()..].trim();
                let left_result = self.evaluate(left)?;
                let right_result = self.evaluate(right)?;
                return Ok(Some((left_result, right_result)));
            }
        }
        Ok(None)
    }

    /// Find operator position, respecting parentheses
    fn find_operator_position(&self, expr: &str, op: &str) -> Option<usize> {
        let mut depth = 0;
        let bytes = expr.as_bytes();
        let op_bytes = op.as_bytes();

        for i in 0..expr.len() {
            if bytes[i] == b'(' {
                depth += 1;
            } else if bytes[i] == b')' {
                depth -= 1;
            } else if depth == 0 && i + op.len() <= expr.len() {
                if &bytes[i..i + op.len()] == op_bytes {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Evaluate a simple comparison expression
    fn evaluate_comparison(&self, expr: &str) -> Result<bool, ExprError> {
        let expr = expr.trim();

        // Try each comparison operator
        let operators = [
            (">=", CompOp::Gte),
            ("<=", CompOp::Lte),
            ("!=", CompOp::Neq),
            ("==", CompOp::Eq),
            (">", CompOp::Gt),
            ("<", CompOp::Lt),
        ];

        for (op_str, op) in operators {
            if let Some(pos) = expr.find(op_str) {
                let left = expr[..pos].trim();
                let right = expr[pos + op_str.len()..].trim();

                let left_val = self.resolve_value(left)?;
                let right_val = self.resolve_value(right)?;

                return Ok(match op {
                    CompOp::Gt => left_val > right_val,
                    CompOp::Gte => left_val >= right_val,
                    CompOp::Lt => left_val < right_val,
                    CompOp::Lte => left_val <= right_val,
                    CompOp::Eq => left_val == right_val,
                    CompOp::Neq => left_val != right_val,
                });
            }
        }

        Err(ExprError::ParseError(format!(
            "No valid comparison operator found in: {}",
            expr
        )))
    }

    /// Resolve a value (variable or literal)
    fn resolve_value(&self, token: &str) -> Result<i64, ExprError> {
        let token = token.trim();

        // Try parsing as integer literal
        if let Ok(val) = token.parse::<i64>() {
            return Ok(val);
        }

        // Try as variable
        self.variables
            .get(token)
            .copied()
            .ok_or_else(|| ExprError::UnknownVariable(token.to_string()))
    }
}

#[derive(Debug, Clone, Copy)]
enum CompOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_comparison() {
        let mut ctx = ExprContext::new();
        ctx.set("used", 5);
        ctx.set("total", 10);

        assert!(ctx.evaluate("used > 3").unwrap());
        assert!(!ctx.evaluate("used > 10").unwrap());
        assert!(ctx.evaluate("used < total").unwrap());
        assert!(ctx.evaluate("total >= 10").unwrap());
        assert!(ctx.evaluate("used == 5").unwrap());
        assert!(ctx.evaluate("used != 10").unwrap());
    }

    #[test]
    fn test_logical_or() {
        let mut ctx = ExprContext::new();
        ctx.set("used", 8);
        ctx.set("total", 5);

        assert!(ctx.evaluate("used > 7 || used > total").unwrap());
        assert!(ctx.evaluate("used > 10 || total < 10").unwrap());
        assert!(!ctx.evaluate("used > 10 || total > 10").unwrap());
    }

    #[test]
    fn test_logical_and() {
        let mut ctx = ExprContext::new();
        ctx.set("used", 8);
        ctx.set("total", 10);

        assert!(ctx.evaluate("used > 5 && total > 5").unwrap());
        assert!(!ctx.evaluate("used > 5 && total > 15").unwrap());
    }

    #[test]
    fn test_mixed_operators() {
        let mut ctx = ExprContext::new();
        ctx.set("used", 8);
        ctx.set("total", 5);

        // OR has lower precedence, so this is: (used > 7) || (used > total)
        assert!(ctx.evaluate("used > 7 || used > total").unwrap());
    }

    #[test]
    fn test_unknown_variable() {
        let ctx = ExprContext::new();
        let result = ctx.evaluate("unknown > 5");
        assert!(matches!(result, Err(ExprError::UnknownVariable(_))));
    }
}
