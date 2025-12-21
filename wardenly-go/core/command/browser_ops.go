package command

// Click performs a mouse click at the specified coordinates.
type Click struct {
	baseSessionCommand
	X, Y float64
}

func NewClick(sessionID string, x, y float64) *Click {
	return &Click{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		X:                  x,
		Y:                  y,
	}
}

func (c *Click) CommandName() string {
	return "Click"
}

// ClickAll performs a mouse click on all running sessions.
type ClickAll struct {
	X, Y float64
}

func (c *ClickAll) CommandName() string {
	return "ClickAll"
}

// Drag performs a mouse drag operation along a path.
type Drag struct {
	baseSessionCommand
	Points []Point
}

// Point represents a coordinate.
type Point struct {
	X, Y float64
}

func NewDrag(sessionID string, points []Point) *Drag {
	return &Drag{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		Points:             points,
	}
}

func NewDragFromTo(sessionID string, fromX, fromY, toX, toY float64) *Drag {
	return &Drag{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		Points:             []Point{{fromX, fromY}, {toX, toY}},
	}
}

func (c *Drag) CommandName() string {
	return "Drag"
}

// DragAll performs a drag operation on all running sessions.
type DragAll struct {
	Points []Point
}

func (c *DragAll) CommandName() string {
	return "DragAll"
}

// CaptureScreen captures the current browser screen.
type CaptureScreen struct {
	baseSessionCommand
	SaveToFile bool
}

func NewCaptureScreen(sessionID string, saveToFile bool) *CaptureScreen {
	return &CaptureScreen{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		SaveToFile:         saveToFile,
	}
}

func (c *CaptureScreen) CommandName() string {
	return "CaptureScreen"
}

// RefreshPage refreshes the browser page.
type RefreshPage struct {
	baseSessionCommand
}

func NewRefreshPage(sessionID string) *RefreshPage {
	return &RefreshPage{baseSessionCommand{sessionID: sessionID}}
}

func (c *RefreshPage) CommandName() string {
	return "RefreshPage"
}

// SaveCookies saves the current session cookies to the database.
type SaveCookies struct {
	baseSessionCommand
}

func NewSaveCookies(sessionID string) *SaveCookies {
	return &SaveCookies{baseSessionCommand{sessionID: sessionID}}
}

func (c *SaveCookies) CommandName() string {
	return "SaveCookies"
}

// StartScreencast starts frame streaming from the browser.
type StartScreencast struct {
	baseSessionCommand
	Quality int // JPEG quality 0-100
	MaxFPS  int // Max frames per second
}

func NewStartScreencast(sessionID string, quality, maxFPS int) *StartScreencast {
	return &StartScreencast{
		baseSessionCommand: baseSessionCommand{sessionID: sessionID},
		Quality:            quality,
		MaxFPS:             maxFPS,
	}
}

func (c *StartScreencast) CommandName() string {
	return "StartScreencast"
}

// StopScreencast stops frame streaming.
type StopScreencast struct {
	baseSessionCommand
}

func NewStopScreencast(sessionID string) *StopScreencast {
	return &StopScreencast{baseSessionCommand{sessionID: sessionID}}
}

func (c *StopScreencast) CommandName() string {
	return "StopScreencast"
}
