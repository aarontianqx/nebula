// Package script defines automation script types and execution logic.
package script

import (
	"fmt"
	"time"
)

// Script represents an automation script with metadata and execution steps.
type Script struct {
	// Name is the unique identifier for this script
	Name string

	// Description provides a human-readable explanation of what the script does
	Description string

	// Version is the script version for compatibility tracking
	Version string

	// Author is the script creator
	Author string

	// Steps are the ordered execution steps
	Steps []Step
}

// Step represents a single step in script execution.
type Step struct {
	// ExpectedScene is the scene name this step expects to match
	ExpectedScene string

	// Timeout is the maximum time to wait for the expected scene
	Timeout time.Duration

	// Actions are the actions to perform when the scene matches
	Actions []Action

	// ContinueOnFailure determines if execution continues when this step fails
	ContinueOnFailure bool

	// Loop defines optional loop behavior for this step
	Loop *Loop

	// OCRRule defines optional OCR-based resource checking
	OCRRule *OCRRule
}

// Action represents a single action within a step.
type Action struct {
	// Type is the action type (click, wait, drag, etc.)
	Type ActionType

	// Points are the coordinates for the action
	Points []Point

	// Duration is the time for the action (e.g., wait duration)
	Duration time.Duration

	// RetryCount is the number of retries on failure
	RetryCount int

	// Key is used for counter operations (incr/decr)
	Key string

	// Condition is used for conditional actions (quit)
	Condition *Condition
}

// ActionType represents the type of action.
type ActionType string

const (
	ActionTypeClick      ActionType = "click"
	ActionTypeWait       ActionType = "wait"
	ActionTypeDrag       ActionType = "drag"
	ActionTypeQuit       ActionType = "quit"
	ActionTypeIncr       ActionType = "incr"
	ActionTypeDecr       ActionType = "decr"
	ActionTypeCheckScene ActionType = "check_scene"
)

// Point represents coordinates for actions.
type Point struct {
	X float64
	Y float64
}

// Condition defines a condition check for script control.
type Condition struct {
	// Op is the comparison operator (eq, gt, lt, neq, gte, lte)
	Op string

	// Key is the counter key to check
	Key string

	// Value is the value to compare against
	Value int
}

// Loop defines how a sequence of actions should be repeated.
type Loop struct {
	// StartIndex is the index of the first action in the loop (0-based)
	StartIndex int

	// EndIndex is the index of the last action in the loop (0-based)
	EndIndex int

	// Count is the number of iterations (-1 for infinite)
	Count int

	// Until is the scene name that stops the loop when matched
	Until string

	// Interval is the time between loop iterations
	Interval time.Duration
}

// OCRRule defines OCR-based resource check behavior.
type OCRRule struct {
	// Name identifies the rule (e.g., "quit_when_exhausted")
	Name string

	// ROI is the region of interest for OCR
	ROI ROI

	// Threshold is the numerator threshold for the quit condition
	Threshold int
}

// ROI defines a rectangular region of interest for OCR.
type ROI struct {
	X      int
	Y      int
	Width  int
	Height int
}

// Evaluate checks if the condition is satisfied.
func (c *Condition) Evaluate(counters map[string]int) bool {
	if c == nil {
		return false
	}

	value, exists := counters[c.Key]
	if !exists {
		value = 0
	}

	switch c.Op {
	case "eq":
		return value == c.Value
	case "neq":
		return value != c.Value
	case "gt":
		return value > c.Value
	case "gte":
		return value >= c.Value
	case "lt":
		return value < c.Value
	case "lte":
		return value <= c.Value
	default:
		return false
	}
}

// IsInfinite returns true if the loop runs indefinitely.
func (l *Loop) IsInfinite() bool {
	return l != nil && l.Count < 0
}

// HasUntilCondition returns true if the loop has a scene-based stop condition.
func (l *Loop) HasUntilCondition() bool {
	return l != nil && l.Until != ""
}

// ValidateIndices checks if the loop indices are valid for the given number of actions.
// Returns an error if indices are invalid.
func (l *Loop) ValidateIndices(actionCount int) error {
	if l == nil {
		return nil
	}
	if l.StartIndex < 0 {
		return fmt.Errorf("loop startIndex (%d) cannot be negative", l.StartIndex)
	}
	if l.EndIndex < 0 {
		return fmt.Errorf("loop endIndex (%d) cannot be negative", l.EndIndex)
	}
	if l.StartIndex > l.EndIndex {
		return fmt.Errorf("loop startIndex (%d) cannot be greater than endIndex (%d)", l.StartIndex, l.EndIndex)
	}
	if l.EndIndex >= actionCount {
		return fmt.Errorf("loop endIndex (%d) exceeds action count (%d)", l.EndIndex, actionCount)
	}
	return nil
}
