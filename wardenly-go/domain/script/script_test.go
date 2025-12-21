package script

import "testing"

func TestCondition_Evaluate(t *testing.T) {
	counters := map[string]int{
		"count": 5,
		"zero":  0,
		"ten":   10,
	}

	tests := []struct {
		name      string
		condition *Condition
		expected  bool
	}{
		// eq tests
		{"eq true", &Condition{Op: "eq", Key: "count", Value: 5}, true},
		{"eq false", &Condition{Op: "eq", Key: "count", Value: 3}, false},

		// neq tests
		{"neq true", &Condition{Op: "neq", Key: "count", Value: 3}, true},
		{"neq false", &Condition{Op: "neq", Key: "count", Value: 5}, false},

		// gt tests
		{"gt true", &Condition{Op: "gt", Key: "count", Value: 3}, true},
		{"gt false", &Condition{Op: "gt", Key: "count", Value: 5}, false},
		{"gt boundary", &Condition{Op: "gt", Key: "count", Value: 4}, true},

		// gte tests
		{"gte true equal", &Condition{Op: "gte", Key: "count", Value: 5}, true},
		{"gte true greater", &Condition{Op: "gte", Key: "count", Value: 3}, true},
		{"gte false", &Condition{Op: "gte", Key: "count", Value: 6}, false},

		// lt tests
		{"lt true", &Condition{Op: "lt", Key: "count", Value: 10}, true},
		{"lt false", &Condition{Op: "lt", Key: "count", Value: 5}, false},

		// lte tests
		{"lte true equal", &Condition{Op: "lte", Key: "count", Value: 5}, true},
		{"lte true less", &Condition{Op: "lte", Key: "count", Value: 10}, true},
		{"lte false", &Condition{Op: "lte", Key: "count", Value: 3}, false},

		// missing key (defaults to 0)
		{"missing key eq 0", &Condition{Op: "eq", Key: "missing", Value: 0}, true},
		{"missing key gt 0", &Condition{Op: "gt", Key: "missing", Value: 0}, false},

		// invalid operator
		{"invalid op", &Condition{Op: "invalid", Key: "count", Value: 5}, false},

		// nil condition
		{"nil condition", nil, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.condition.Evaluate(counters); got != tt.expected {
				t.Errorf("Evaluate() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestLoop_IsInfinite(t *testing.T) {
	tests := []struct {
		name     string
		loop     *Loop
		expected bool
	}{
		{"infinite", &Loop{Count: -1}, true},
		{"finite", &Loop{Count: 5}, false},
		{"zero", &Loop{Count: 0}, false},
		{"nil", nil, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.loop.IsInfinite(); got != tt.expected {
				t.Errorf("IsInfinite() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestLoop_HasUntilCondition(t *testing.T) {
	tests := []struct {
		name     string
		loop     *Loop
		expected bool
	}{
		{"with until", &Loop{Until: "main_city"}, true},
		{"without until", &Loop{Until: ""}, false},
		{"nil", nil, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.loop.HasUntilCondition(); got != tt.expected {
				t.Errorf("HasUntilCondition() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestRegistry_Basic(t *testing.T) {
	registry := NewRegistry()

	script1 := &Script{Name: "script1", Description: "Test script 1"}
	script2 := &Script{Name: "script2", Description: "Test script 2"}

	registry.Register(script1)
	registry.Register(script2)

	t.Run("Get", func(t *testing.T) {
		if got := registry.Get("script1"); got != script1 {
			t.Error("Failed to get script1")
		}
		if got := registry.Get("nonexistent"); got != nil {
			t.Error("Expected nil for nonexistent script")
		}
	})

	t.Run("Count", func(t *testing.T) {
		if got := registry.Count(); got != 2 {
			t.Errorf("Count() = %d, want 2", got)
		}
	})

	t.Run("List", func(t *testing.T) {
		names := registry.List()
		if len(names) != 2 {
			t.Errorf("List() returned %d names, want 2", len(names))
		}
	})

	t.Run("Exists", func(t *testing.T) {
		if !registry.Exists("script1") {
			t.Error("Exists(script1) should return true")
		}
		if registry.Exists("nonexistent") {
			t.Error("Exists(nonexistent) should return false")
		}
	})

	t.Run("Clear", func(t *testing.T) {
		registry.Clear()
		if registry.Count() != 0 {
			t.Error("Clear() did not remove all scripts")
		}
	})
}

func TestRegistry_RegisterAll(t *testing.T) {
	registry := NewRegistry()

	scripts := []*Script{
		{Name: "script1"},
		{Name: "script2"},
		{Name: "script3"},
	}

	registry.RegisterAll(scripts)

	if registry.Count() != 3 {
		t.Errorf("RegisterAll: Count() = %d, want 3", registry.Count())
	}
}

func TestRegistry_All(t *testing.T) {
	registry := NewRegistry()

	scripts := []*Script{
		{Name: "script1"},
		{Name: "script2"},
	}

	registry.RegisterAll(scripts)

	all := registry.All()
	if len(all) != 2 {
		t.Errorf("All() returned %d scripts, want 2", len(all))
	}
}

func TestRegistry_Replace(t *testing.T) {
	registry := NewRegistry()

	script1 := &Script{Name: "script1", Description: "Original"}
	script1Updated := &Script{Name: "script1", Description: "Updated"}

	registry.Register(script1)
	registry.Register(script1Updated)

	got := registry.Get("script1")
	if got.Description != "Updated" {
		t.Error("Register should replace existing script with same name")
	}
	if registry.Count() != 1 {
		t.Error("Count should still be 1 after replacing")
	}
}
