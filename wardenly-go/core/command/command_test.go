package command

import "testing"

func TestCommand_Names(t *testing.T) {
	tests := []struct {
		cmd      Command
		expected string
	}{
		{&StartSession{}, "StartSession"},
		{NewStopSession("s1"), "StopSession"},
		{&StopAllSessions{}, "StopAllSessions"},
		{NewClick("s1", 100, 200), "Click"},
		{&ClickAll{X: 100, Y: 200}, "ClickAll"},
		{NewDrag("s1", []Point{{0, 0}, {100, 100}}), "Drag"},
		{&DragAll{}, "DragAll"},
		{NewCaptureScreen("s1", true), "CaptureScreen"},
		{NewRefreshPage("s1"), "RefreshPage"},
		{NewSaveCookies("s1"), "SaveCookies"},
		{NewStartScript("s1", "test"), "StartScript"},
		{NewStopScript("s1"), "StopScript"},
		{&StartAllScripts{}, "StartAllScripts"},
		{&StopAllScripts{}, "StopAllScripts"},
		{NewSetScriptSelection("s1", "test"), "SetScriptSelection"},
		{&SyncScriptSelection{ScriptName: "test"}, "SyncScriptSelection"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if got := tt.cmd.CommandName(); got != tt.expected {
				t.Errorf("CommandName() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionCommand_SessionID(t *testing.T) {
	tests := []struct {
		name     string
		cmd      SessionCommand
		expected string
	}{
		{"StopSession", NewStopSession("session-123"), "session-123"},
		{"Click", NewClick("session-456", 100, 200), "session-456"},
		{"Drag", NewDrag("session-789", nil), "session-789"},
		{"CaptureScreen", NewCaptureScreen("session-abc", false), "session-abc"},
		{"RefreshPage", NewRefreshPage("session-def"), "session-def"},
		{"SaveCookies", NewSaveCookies("session-ghi"), "session-ghi"},
		{"StartScript", NewStartScript("session-jkl", "test"), "session-jkl"},
		{"StopScript", NewStopScript("session-mno"), "session-mno"},
		{"SetScriptSelection", NewSetScriptSelection("session-pqr", "test"), "session-pqr"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.cmd.SessionID(); got != tt.expected {
				t.Errorf("SessionID() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestNewDragFromTo(t *testing.T) {
	cmd := NewDragFromTo("s1", 10, 20, 30, 40)

	if cmd.SessionID() != "s1" {
		t.Errorf("SessionID() = %v, want s1", cmd.SessionID())
	}

	if len(cmd.Points) != 2 {
		t.Fatalf("Expected 2 points, got %d", len(cmd.Points))
	}

	if cmd.Points[0].X != 10 || cmd.Points[0].Y != 20 {
		t.Errorf("First point = (%v, %v), want (10, 20)", cmd.Points[0].X, cmd.Points[0].Y)
	}

	if cmd.Points[1].X != 30 || cmd.Points[1].Y != 40 {
		t.Errorf("Second point = (%v, %v), want (30, 40)", cmd.Points[1].X, cmd.Points[1].Y)
	}
}
