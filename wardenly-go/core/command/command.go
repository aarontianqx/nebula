// Package command defines all commands that can be sent to the application.
// Commands represent user intentions and are processed by the application layer.
package command

// Command is the base interface for all commands.
// Commands are sent from the presentation layer to the application layer.
type Command interface {
	// CommandName returns the name of the command for logging/debugging
	CommandName() string
}

// SessionCommand is a command that targets a specific session.
type SessionCommand interface {
	Command
	// SessionID returns the target session ID
	SessionID() string
}

// baseSessionCommand provides common implementation for session commands.
type baseSessionCommand struct {
	sessionID string
}

func (c *baseSessionCommand) SessionID() string {
	return c.sessionID
}
