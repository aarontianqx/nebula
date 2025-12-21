package event

import (
	"errors"
	"image"
	"testing"

	"wardenly-go/core/state"
)

func TestEvent_Names(t *testing.T) {
	tests := []struct {
		event    Event
		expected string
	}{
		{NewSessionStarted("s1", "acc1", "Account 1"), "SessionStarted"},
		{NewSessionStopped("s1", nil), "SessionStopped"},
		{NewSessionStateChanged("s1", state.StateIdle, state.StateStarting), "SessionStateChanged"},
		{NewScreenCaptured("s1", nil), "ScreenCaptured"},
		{NewLoginSucceeded("s1"), "LoginSucceeded"},
		{NewLoginFailed("s1", errors.New("test")), "LoginFailed"},
		{NewCookiesSaved("s1"), "CookiesSaved"},
		{NewOperationFailed("s1", "click", errors.New("test")), "OperationFailed"},
		{NewScriptStarted("s1", "test"), "ScriptStarted"},
		{NewScriptStopped("s1", "test", StopReasonNormal, nil), "ScriptStopped"},
		{NewScriptStepExecuted("s1", 0, "main_city"), "ScriptStepExecuted"},
		{NewScriptSelectionChanged("s1", "test"), "ScriptSelectionChanged"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if got := tt.event.EventName(); got != tt.expected {
				t.Errorf("EventName() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionEvent_SessionID(t *testing.T) {
	tests := []struct {
		name     string
		event    SessionEvent
		expected string
	}{
		{"SessionStarted", NewSessionStarted("session-123", "acc1", "Account 1"), "session-123"},
		{"SessionStopped", NewSessionStopped("session-456", nil), "session-456"},
		{"SessionStateChanged", NewSessionStateChanged("session-789", state.StateIdle, state.StateStarting), "session-789"},
		{"ScreenCaptured", NewScreenCaptured("session-abc", nil), "session-abc"},
		{"LoginSucceeded", NewLoginSucceeded("session-def"), "session-def"},
		{"LoginFailed", NewLoginFailed("session-ghi", nil), "session-ghi"},
		{"CookiesSaved", NewCookiesSaved("session-jkl"), "session-jkl"},
		{"OperationFailed", NewOperationFailed("session-mno", "click", nil), "session-mno"},
		{"ScriptStarted", NewScriptStarted("session-pqr", "test"), "session-pqr"},
		{"ScriptStopped", NewScriptStopped("session-stu", "test", StopReasonNormal, nil), "session-stu"},
		{"ScriptStepExecuted", NewScriptStepExecuted("session-vwx", 0, "main_city"), "session-vwx"},
		{"ScriptSelectionChanged", NewScriptSelectionChanged("session-yz", "test"), "session-yz"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.event.SessionID(); got != tt.expected {
				t.Errorf("SessionID() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestStopReason_String(t *testing.T) {
	tests := []struct {
		reason   StopReason
		expected string
	}{
		{StopReasonNormal, "Normal"},
		{StopReasonManual, "Manual"},
		{StopReasonError, "Error"},
		{StopReasonResourceExhausted, "ResourceExhausted"},
		{StopReasonBrowserStopped, "BrowserStopped"},
		{StopReason(99), "Unknown"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if got := tt.reason.String(); got != tt.expected {
				t.Errorf("String() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionStarted_Fields(t *testing.T) {
	e := NewSessionStarted("s1", "acc1", "Test Account")

	if e.AccountID != "acc1" {
		t.Errorf("AccountID = %v, want acc1", e.AccountID)
	}
	if e.AccountName != "Test Account" {
		t.Errorf("AccountName = %v, want Test Account", e.AccountName)
	}
}

func TestSessionStopped_Error(t *testing.T) {
	testErr := errors.New("test error")
	e := NewSessionStopped("s1", testErr)

	if e.Error != testErr {
		t.Errorf("Error = %v, want %v", e.Error, testErr)
	}
}

func TestSessionStateChanged_States(t *testing.T) {
	e := NewSessionStateChanged("s1", state.StateReady, state.StateScriptRunning)

	if e.OldState != state.StateReady {
		t.Errorf("OldState = %v, want Ready", e.OldState)
	}
	if e.NewState != state.StateScriptRunning {
		t.Errorf("NewState = %v, want ScriptRunning", e.NewState)
	}
}

func TestScreenCaptured_Image(t *testing.T) {
	img := image.NewRGBA(image.Rect(0, 0, 100, 100))
	e := NewScreenCaptured("s1", img)

	if e.Image != img {
		t.Error("Image not set correctly")
	}
}

func TestScriptStopped_Fields(t *testing.T) {
	testErr := errors.New("test error")
	e := NewScriptStopped("s1", "test_script", StopReasonError, testErr)

	if e.ScriptName != "test_script" {
		t.Errorf("ScriptName = %v, want test_script", e.ScriptName)
	}
	if e.Reason != StopReasonError {
		t.Errorf("Reason = %v, want Error", e.Reason)
	}
	if e.Error != testErr {
		t.Errorf("Error = %v, want %v", e.Error, testErr)
	}
}
