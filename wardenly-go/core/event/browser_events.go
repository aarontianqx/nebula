package event

import "image"

// ScreenCaptured is published when a screenshot is captured.
type ScreenCaptured struct {
	baseSessionEvent
	Image image.Image
}

func NewScreenCaptured(sessionID string, img image.Image) *ScreenCaptured {
	return &ScreenCaptured{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		Image:            img,
	}
}

func (e *ScreenCaptured) EventName() string {
	return "ScreenCaptured"
}

// LoginSucceeded is published when login completes successfully.
type LoginSucceeded struct {
	baseSessionEvent
}

func NewLoginSucceeded(sessionID string) *LoginSucceeded {
	return &LoginSucceeded{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
	}
}

func (e *LoginSucceeded) EventName() string {
	return "LoginSucceeded"
}

// LoginFailed is published when login fails.
type LoginFailed struct {
	baseSessionEvent
	Error error
}

func NewLoginFailed(sessionID string, err error) *LoginFailed {
	return &LoginFailed{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		Error:            err,
	}
}

func (e *LoginFailed) EventName() string {
	return "LoginFailed"
}

// CookiesSaved is published when cookies are saved successfully.
type CookiesSaved struct {
	baseSessionEvent
}

func NewCookiesSaved(sessionID string) *CookiesSaved {
	return &CookiesSaved{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
	}
}

func (e *CookiesSaved) EventName() string {
	return "CookiesSaved"
}

// OperationFailed is published when a browser operation fails.
type OperationFailed struct {
	baseSessionEvent
	Operation string
	Error     error
}

func NewOperationFailed(sessionID, operation string, err error) *OperationFailed {
	return &OperationFailed{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		Operation:        operation,
		Error:            err,
	}
}

func (e *OperationFailed) EventName() string {
	return "OperationFailed"
}

// ScreencastStarted is published when screencast actually starts on a session.
type ScreencastStarted struct {
	baseSessionEvent
	Quality int
	MaxFPS  int
}

func NewScreencastStarted(sessionID string, quality, maxFPS int) *ScreencastStarted {
	return &ScreencastStarted{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		Quality:          quality,
		MaxFPS:           maxFPS,
	}
}

func (e *ScreencastStarted) EventName() string {
	return "ScreencastStarted"
}

// ScreencastStopped is published when screencast stops on a session.
type ScreencastStopped struct {
	baseSessionEvent
}

func NewScreencastStopped(sessionID string) *ScreencastStopped {
	return &ScreencastStopped{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
	}
}

func (e *ScreencastStopped) EventName() string {
	return "ScreencastStopped"
}

// DriverStarted is published when browser driver starts successfully.
// This happens before login begins, indicating browser is ready to render frames.
type DriverStarted struct {
	baseSessionEvent
}

func NewDriverStarted(sessionID string) *DriverStarted {
	return &DriverStarted{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
	}
}

func (e *DriverStarted) EventName() string {
	return "DriverStarted"
}
