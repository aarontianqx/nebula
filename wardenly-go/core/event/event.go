// Package event defines all events that can be published by the application.
// Events represent state changes and are consumed by the presentation layer.
package event

import "wardenly-go/core/state"

// Event is the base interface for all events.
// Events are published by the application layer and consumed by subscribers.
type Event interface {
	// EventName returns the name of the event for logging/debugging
	EventName() string
}

// SessionEvent is an event that originates from a specific session.
type SessionEvent interface {
	Event
	// SessionID returns the source session ID
	SessionID() string
}

// baseSessionEvent provides common implementation for session events.
type baseSessionEvent struct {
	sessionID string
}

func (e *baseSessionEvent) SessionID() string {
	return e.sessionID
}

// SessionStarted is published when a session starts successfully.
type SessionStarted struct {
	baseSessionEvent
	AccountID   string
	AccountName string
}

func NewSessionStarted(sessionID, accountID, accountName string) *SessionStarted {
	return &SessionStarted{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		AccountID:        accountID,
		AccountName:      accountName,
	}
}

func (e *SessionStarted) EventName() string {
	return "SessionStarted"
}

// SessionStopped is published when a session stops.
type SessionStopped struct {
	baseSessionEvent
	Error error // nil if stopped normally
}

func NewSessionStopped(sessionID string, err error) *SessionStopped {
	return &SessionStopped{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		Error:            err,
	}
}

func (e *SessionStopped) EventName() string {
	return "SessionStopped"
}

// SessionStateChanged is published when a session's state changes.
type SessionStateChanged struct {
	baseSessionEvent
	OldState state.SessionState
	NewState state.SessionState
}

func NewSessionStateChanged(sessionID string, oldState, newState state.SessionState) *SessionStateChanged {
	return &SessionStateChanged{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		OldState:         oldState,
		NewState:         newState,
	}
}

func (e *SessionStateChanged) EventName() string {
	return "SessionStateChanged"
}
