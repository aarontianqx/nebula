package command

// StartScript starts a script on a session.
type StartScript struct {
	baseSessionCommand
	ScriptName string
}

func NewStartScript(sessionID, scriptName string) *StartScript {
	return &StartScript{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		ScriptName:         scriptName,
	}
}

func (c *StartScript) CommandName() string {
	return "StartScript"
}

// StopScript stops the running script on a session.
type StopScript struct {
	baseSessionCommand
}

func NewStopScript(sessionID string) *StopScript {
	return &StopScript{baseSessionCommand{sessionID: sessionID}}
}

func (c *StopScript) CommandName() string {
	return "StopScript"
}

// StartAllScripts starts scripts on all sessions that are not currently running a script.
// Each session uses its own selected script.
type StartAllScripts struct{}

func (c *StartAllScripts) CommandName() string {
	return "StartAllScripts"
}

// StopAllScripts stops scripts on all sessions that are currently running a script.
type StopAllScripts struct{}

func (c *StopAllScripts) CommandName() string {
	return "StopAllScripts"
}

// SetScriptSelection sets the selected script for a session (without starting it).
type SetScriptSelection struct {
	baseSessionCommand
	ScriptName string
}

func NewSetScriptSelection(sessionID, scriptName string) *SetScriptSelection {
	return &SetScriptSelection{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		ScriptName:         scriptName,
	}
}

func (c *SetScriptSelection) CommandName() string {
	return "SetScriptSelection"
}

// SyncScriptSelection synchronizes script selection to all sessions.
type SyncScriptSelection struct {
	ScriptName string
}

func (c *SyncScriptSelection) CommandName() string {
	return "SyncScriptSelection"
}
