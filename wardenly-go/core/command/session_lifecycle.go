package command

// StartSession starts a new browser session for an account.
type StartSession struct {
	AccountID string
	RoleName  string // In-game character name
	ServerID  int
	UserName  string
	Password  string
	Cookies   []Cookie // Optional: for cookie-based login
}

func (c *StartSession) CommandName() string {
	return "StartSession"
}

// Cookie represents a browser cookie for session restoration.
type Cookie struct {
	Name       string
	Value      string
	Domain     string
	Path       string
	HTTPOnly   bool
	Secure     bool
	SourcePort int
}

// StopSession stops a running session.
type StopSession struct {
	baseSessionCommand
}

func NewStopSession(sessionID string) *StopSession {
	return &StopSession{baseSessionCommand{sessionID: sessionID}}
}

func (c *StopSession) CommandName() string {
	return "StopSession"
}

// StopAllSessions stops all running sessions.
type StopAllSessions struct{}

func (c *StopAllSessions) CommandName() string {
	return "StopAllSessions"
}
