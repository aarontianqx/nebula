package session

import (
	"testing"

	domainscript "wardenly-go/domain/script"
)

func TestStepResult_Constants(t *testing.T) {
	// Verify step result constants
	if stepResultContinue != 0 {
		t.Errorf("stepResultContinue = %d, want 0", stepResultContinue)
	}
	if stepResultQuit != 1 {
		t.Errorf("stepResultQuit = %d, want 1", stepResultQuit)
	}
	if stepResultResourceExhausted != 2 {
		t.Errorf("stepResultResourceExhausted = %d, want 2", stepResultResourceExhausted)
	}
	if stepResultError != 3 {
		t.Errorf("stepResultError = %d, want 3", stepResultError)
	}
}

func TestCondition_Evaluate(t *testing.T) {
	counters := map[string]int{
		"count": 5,
		"zero":  0,
	}

	tests := []struct {
		name      string
		condition *domainscript.Condition
		expected  bool
	}{
		{"eq true", &domainscript.Condition{Op: "eq", Key: "count", Value: 5}, true},
		{"eq false", &domainscript.Condition{Op: "eq", Key: "count", Value: 3}, false},
		{"gt true", &domainscript.Condition{Op: "gt", Key: "count", Value: 3}, true},
		{"gt false", &domainscript.Condition{Op: "gt", Key: "count", Value: 5}, false},
		{"lt true", &domainscript.Condition{Op: "lt", Key: "count", Value: 10}, true},
		{"lt false", &domainscript.Condition{Op: "lt", Key: "count", Value: 5}, false},
		{"missing key", &domainscript.Condition{Op: "eq", Key: "missing", Value: 0}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.condition.Evaluate(counters); got != tt.expected {
				t.Errorf("Evaluate() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestLoop_Properties(t *testing.T) {
	t.Run("IsInfinite", func(t *testing.T) {
		loop := &domainscript.Loop{Count: -1}
		if !loop.IsInfinite() {
			t.Error("Expected infinite loop")
		}

		loop = &domainscript.Loop{Count: 5}
		if loop.IsInfinite() {
			t.Error("Expected finite loop")
		}
	})

	t.Run("HasUntilCondition", func(t *testing.T) {
		loop := &domainscript.Loop{Until: "main_city"}
		if !loop.HasUntilCondition() {
			t.Error("Expected until condition")
		}

		loop = &domainscript.Loop{Until: ""}
		if loop.HasUntilCondition() {
			t.Error("Expected no until condition")
		}
	})
}

func TestActionType_Values(t *testing.T) {
	// Verify action type constants match
	if domainscript.ActionTypeClick != "click" {
		t.Errorf("ActionTypeClick = %v, want click", domainscript.ActionTypeClick)
	}
	if domainscript.ActionTypeWait != "wait" {
		t.Errorf("ActionTypeWait = %v, want wait", domainscript.ActionTypeWait)
	}
	if domainscript.ActionTypeDrag != "drag" {
		t.Errorf("ActionTypeDrag = %v, want drag", domainscript.ActionTypeDrag)
	}
	if domainscript.ActionTypeQuit != "quit" {
		t.Errorf("ActionTypeQuit = %v, want quit", domainscript.ActionTypeQuit)
	}
	if domainscript.ActionTypeIncr != "incr" {
		t.Errorf("ActionTypeIncr = %v, want incr", domainscript.ActionTypeIncr)
	}
	if domainscript.ActionTypeDecr != "decr" {
		t.Errorf("ActionTypeDecr = %v, want decr", domainscript.ActionTypeDecr)
	}
	if domainscript.ActionTypeCheckScene != "check_scene" {
		t.Errorf("ActionTypeCheckScene = %v, want check_scene", domainscript.ActionTypeCheckScene)
	}
}
