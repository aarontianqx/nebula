package event

// ScriptStarted is published when a script starts executing.
type ScriptStarted struct {
	baseSessionEvent
	ScriptName string
}

func NewScriptStarted(sessionID, scriptName string) *ScriptStarted {
	return &ScriptStarted{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		ScriptName:       scriptName,
	}
}

func (e *ScriptStarted) EventName() string {
	return "ScriptStarted"
}

// StopReason indicates why a script stopped.
type StopReason int

const (
	// StopReasonNormal indicates the script completed normally.
	StopReasonNormal StopReason = iota
	// StopReasonManual indicates the script was stopped by the user.
	StopReasonManual
	// StopReasonError indicates the script stopped due to an error.
	StopReasonError
	// StopReasonResourceExhausted indicates the script stopped because resources were exhausted.
	StopReasonResourceExhausted
	// StopReasonBrowserStopped indicates the script stopped because the browser was stopped.
	StopReasonBrowserStopped
)

func (r StopReason) String() string {
	switch r {
	case StopReasonNormal:
		return "Normal"
	case StopReasonManual:
		return "Manual"
	case StopReasonError:
		return "Error"
	case StopReasonResourceExhausted:
		return "ResourceExhausted"
	case StopReasonBrowserStopped:
		return "BrowserStopped"
	default:
		return "Unknown"
	}
}

// ScriptStopped is published when a script stops executing.
type ScriptStopped struct {
	baseSessionEvent
	ScriptName string
	Reason     StopReason
	Error      error // Non-nil if Reason is StopReasonError
}

func NewScriptStopped(sessionID, scriptName string, reason StopReason, err error) *ScriptStopped {
	return &ScriptStopped{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		ScriptName:       scriptName,
		Reason:           reason,
		Error:            err,
	}
}

func (e *ScriptStopped) EventName() string {
	return "ScriptStopped"
}

// ScriptStepExecuted is published when a script step is executed.
type ScriptStepExecuted struct {
	baseSessionEvent
	StepIndex int
	SceneName string
}

func NewScriptStepExecuted(sessionID string, stepIndex int, sceneName string) *ScriptStepExecuted {
	return &ScriptStepExecuted{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		StepIndex:        stepIndex,
		SceneName:        sceneName,
	}
}

func (e *ScriptStepExecuted) EventName() string {
	return "ScriptStepExecuted"
}

// ScriptSelectionChanged is published when the selected script changes.
type ScriptSelectionChanged struct {
	baseSessionEvent
	ScriptName string
}

func NewScriptSelectionChanged(sessionID, scriptName string) *ScriptSelectionChanged {
	return &ScriptSelectionChanged{
		baseSessionEvent: baseSessionEvent{sessionID: sessionID},
		ScriptName:       scriptName,
	}
}

func (e *ScriptSelectionChanged) EventName() string {
	return "ScriptSelectionChanged"
}
