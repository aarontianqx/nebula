package presentation

import (
	"testing"

	"wardenly-go/domain/group"
)

func TestMainWindowConfig(t *testing.T) {
	cfg := &MainWindowConfig{}

	if cfg.App != nil {
		t.Error("App should be nil by default")
	}
	if cfg.Bridge != nil {
		t.Error("Bridge should be nil by default")
	}
	if cfg.Logger != nil {
		t.Error("Logger should be nil by default")
	}
}

func TestGroupEntity(t *testing.T) {
	grp := &group.Group{
		ID:         "123",
		Name:       "TestGroup",
		AccountIDs: []string{"acc1", "acc2", "acc3"},
		Ranking:    1,
	}

	if grp.Name != "TestGroup" {
		t.Errorf("Name = %v, want TestGroup", grp.Name)
	}

	if grp.AccountCount() != 3 {
		t.Errorf("AccountCount = %d, want 3", grp.AccountCount())
	}

	if !grp.ContainsAccount("acc1") {
		t.Error("Should contain acc1")
	}

	if grp.ContainsAccount("acc99") {
		t.Error("Should not contain acc99")
	}

	// Test AddAccount
	added := grp.AddAccount("acc4")
	if !added {
		t.Error("Should have added acc4")
	}
	if grp.AccountCount() != 4 {
		t.Errorf("AccountCount = %d, want 4", grp.AccountCount())
	}

	// Test AddAccount duplicate
	added = grp.AddAccount("acc1")
	if added {
		t.Error("Should not add duplicate acc1")
	}

	// Test RemoveAccount
	removed := grp.RemoveAccount("acc2")
	if !removed {
		t.Error("Should have removed acc2")
	}
	if grp.ContainsAccount("acc2") {
		t.Error("Should not contain acc2 after removal")
	}
}
