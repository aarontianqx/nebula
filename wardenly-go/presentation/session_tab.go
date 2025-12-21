package presentation

import (
	"fmt"
	"image/color"
	"log/slog"
	"strconv"
	"strings"
	"sync"

	"wardenly-go/core/state"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

// SessionTab represents a tab for a single session.
type SessionTab struct {
	sessionID   string
	accountName string
	bridge      *UIEventBridge
	logger      *slog.Logger

	// Callbacks
	onStop               func(sessionID string)
	shouldSpreadToAll    func() bool
	isAutoRefreshEnabled func() bool
	onSyncScript         func(scriptName string)
	onStartAllScripts    func()
	onStopAllScripts     func()

	// UI components
	container *fyne.Container

	// Browser control
	stopBtn        *widget.Button
	refreshBtn     *widget.Button
	saveCookiesBtn *widget.Button

	// Script control
	scriptBtn     *widget.Button
	scriptSelect  *widget.Select
	syncScriptBtn *widget.Button
	allScriptsBtn *widget.Button

	// Canvas control
	clickBtn         *widget.Button
	saveScreenshotCb *widget.Check
	xEntry           *widget.Entry
	yEntry           *widget.Entry
	colorEntry       *widget.Entry
	colorRect        *canvas.Rectangle
	pointsArea       *widget.Entry

	// State
	scriptRunning bool
	// suppressScriptSelectSync prevents SetSelected* (programmatic) from triggering
	// immediate script selection sync to backend before session is ready.
	suppressScriptSelectSync bool
	stateMu                  sync.RWMutex
}

// SessionTabConfig holds configuration for SessionTab.
type SessionTabConfig struct {
	SessionID            string
	AccountName          string
	Bridge               *UIEventBridge
	Logger               *slog.Logger
	ScriptNames          []string
	OnStop               func(sessionID string)
	ShouldSpreadToAll    func() bool
	IsAutoRefreshEnabled func() bool
	OnSyncScript         func(scriptName string)
	OnStartAllScripts    func()
	OnStopAllScripts     func()
}

// NewSessionTab creates a new session tab.
func NewSessionTab(cfg *SessionTabConfig) *SessionTab {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	t := &SessionTab{
		sessionID:            cfg.SessionID,
		accountName:          cfg.AccountName,
		bridge:               cfg.Bridge,
		logger:               cfg.Logger,
		onStop:               cfg.OnStop,
		shouldSpreadToAll:    cfg.ShouldSpreadToAll,
		isAutoRefreshEnabled: cfg.IsAutoRefreshEnabled,
		onSyncScript:         cfg.OnSyncScript,
		onStartAllScripts:    cfg.OnStartAllScripts,
		onStopAllScripts:     cfg.OnStopAllScripts,
	}

	// Wrap sections in Cards for visual hierarchy
	browserCard := widget.NewCard("Browser Control", "", t.createBrowserControlBox())
	scriptCard := widget.NewCard("Script Engine", "", t.createScriptControlBox(cfg.ScriptNames))
	inspectorCard := widget.NewCard("Inspector", "", t.createCanvasControlBox())

	t.container = container.NewVBox(
		browserCard,
		scriptCard,
		inspectorCard,
	)

	return t
}

// Container returns the tab's container.
func (t *SessionTab) Container() *fyne.Container {
	return t.container
}

// SessionID returns the session ID.
func (t *SessionTab) SessionID() string {
	return t.sessionID
}

// AccountName returns the account name.
func (t *SessionTab) AccountName() string {
	return t.accountName
}

func (t *SessionTab) createBrowserControlBox() fyne.CanvasObject {
	t.stopBtn = widget.NewButtonWithIcon("Stop", theme.MediaStopIcon(), func() {
		if t.bridge != nil {
			t.bridge.StopSession(t.sessionID)
		}
		if t.onStop != nil {
			t.onStop(t.sessionID)
		}
	})

	t.refreshBtn = widget.NewButtonWithIcon("Refresh", theme.ViewRefreshIcon(), func() {
		if t.bridge != nil {
			if err := t.bridge.RefreshPage(t.sessionID); err != nil {
				t.logger.Error("Failed to refresh", "error", err)
			}
		}
	})
	t.refreshBtn.Disable()

	t.saveCookiesBtn = widget.NewButtonWithIcon("Cookies", theme.DocumentSaveIcon(), func() {
		if t.bridge != nil {
			if err := t.bridge.SaveCookies(t.sessionID); err != nil {
				t.logger.Error("Failed to save cookies", "error", err)
			}
		}
	})
	t.saveCookiesBtn.Disable()

	return container.NewHBox(t.stopBtn, t.refreshBtn, t.saveCookiesBtn)
}

func (t *SessionTab) createScriptControlBox(scriptNames []string) fyne.CanvasObject {
	t.scriptBtn = widget.NewButtonWithIcon("Start", theme.MediaPlayIcon(), func() {
		t.stateMu.RLock()
		running := t.scriptRunning
		t.stateMu.RUnlock()

		if !running {
			t.StartScript()
		} else {
			t.StopScript()
		}
	})
	t.scriptBtn.Disable()

	if scriptNames == nil {
		scriptNames = []string{}
	}
	t.scriptSelect = widget.NewSelect(scriptNames, func(scriptName string) {
		// Sync selection to Session layer (user-initiated only; avoid startup/programmatic sync)
		if t.suppressScriptSelectSync {
			return
		}
		if t.bridge != nil && scriptName != "" {
			if err := t.bridge.SetScriptSelection(t.sessionID, scriptName); err != nil {
				t.logger.Error("Failed to sync script selection", "error", err)
			}
		}
	})
	t.scriptSelect.Disable()
	if len(scriptNames) > 0 {
		t.suppressScriptSelectSync = true
		t.scriptSelect.SetSelectedIndex(0)
		t.suppressScriptSelectSync = false
	}

	t.syncScriptBtn = widget.NewButtonWithIcon("Sync", theme.MediaReplayIcon(), func() {
		if t.onSyncScript != nil && t.scriptSelect.Selected != "" {
			t.onSyncScript(t.scriptSelect.Selected)
		}
	})
	t.syncScriptBtn.Disable()

	t.allScriptsBtn = widget.NewButtonWithIcon("Run All", theme.MediaFastForwardIcon(), func() {
		t.stateMu.RLock()
		running := t.scriptRunning
		t.stateMu.RUnlock()

		if !running {
			if t.onStartAllScripts != nil {
				t.onStartAllScripts()
			}
		} else {
			if t.onStopAllScripts != nil {
				t.onStopAllScripts()
			}
		}
	})
	t.allScriptsBtn.Disable()

	// Two rows for better layout
	row1 := container.NewHBox(t.scriptSelect, t.scriptBtn, t.syncScriptBtn)
	row2 := container.NewHBox(t.allScriptsBtn)

	return container.NewVBox(row1, row2)
}

func (t *SessionTab) createCanvasControlBox() fyne.CanvasObject {
	t.clickBtn = widget.NewButtonWithIcon("Click", theme.MailSendIcon(), func() {
		x, err := strconv.ParseFloat(t.xEntry.Text, 64)
		if err != nil {
			t.logger.Error("Invalid X coordinate", "error", err)
			return
		}
		y, err := strconv.ParseFloat(t.yEntry.Text, 64)
		if err != nil {
			t.logger.Error("Invalid Y coordinate", "error", err)
			return
		}

		if t.bridge == nil {
			return
		}

		if t.shouldSpreadToAll != nil && t.shouldSpreadToAll() {
			t.bridge.ClickAll(x, y)
		} else {
			if err := t.bridge.Click(t.sessionID, x, y); err != nil {
				t.logger.Error("Click failed", "error", err)
			}
		}
	})
	t.clickBtn.Disable()

	t.saveScreenshotCb = widget.NewCheck("Save Screenshot", func(checked bool) {})

	// Coordinate display
	t.xEntry = widget.NewEntry()
	t.xEntry.Disable()
	t.yEntry = widget.NewEntry()
	t.yEntry.Disable()
	t.colorEntry = widget.NewEntry()
	t.colorEntry.Disable()
	t.colorEntry.TextStyle = fyne.TextStyle{Bold: true}

	t.colorRect = canvas.NewRectangle(color.Black)
	t.colorRect.Resize(fyne.NewSize(35, 35))
	t.colorRect.SetMinSize(fyne.NewSize(35, 35))

	coordsColorBox := container.New(layout.NewHBoxLayout(),
		widget.NewLabel("X:"),
		container.NewGridWrap(fyne.NewSize(50, 40), t.xEntry),
		widget.NewLabel("Y:"),
		container.NewGridWrap(fyne.NewSize(50, 40), t.yEntry),
		widget.NewLabel("Color:"),
		container.NewGridWrap(fyne.NewSize(300, 40), t.colorEntry),
		container.NewGridWrap(fyne.NewSize(40, 40), t.colorRect),
	)

	t.pointsArea = widget.NewMultiLineEntry()
	t.pointsArea.Disable()

	return container.NewVBox(
		container.NewHBox(t.clickBtn, t.saveScreenshotCb),
		coordsColorBox,
		container.NewGridWrap(fyne.NewSize(400, 200), t.pointsArea),
	)
}

// HandleCanvasClick returns a handler for canvas click events.
func (t *SessionTab) HandleCanvasClick(canvasWin *CanvasWindow) func(float32, float32) {
	return func(x, y float32) {
		if t.bridge == nil {
			return
		}

		// Update coordinates
		t.xEntry.SetText(fmt.Sprintf("%.0f", x))
		t.yEntry.SetText(fmt.Sprintf("%.0f", y))

		if t.isAutoRefreshEnabled != nil && t.isAutoRefreshEnabled() {
			// Auto refresh mode: click immediately
			if t.shouldSpreadToAll != nil && t.shouldSpreadToAll() {
				t.bridge.ClickAll(float64(x), float64(y))
			} else {
				if err := t.bridge.Click(t.sessionID, float64(x), float64(y)); err != nil {
					t.logger.Error("Click failed", "error", err)
				}
			}

			// Update color from canvas
			t.updateColorFromCanvas(canvasWin, int(x), int(y))
		} else {
			// Manual mode: capture and display
			if err := t.bridge.CaptureScreen(t.sessionID, t.saveScreenshotCb.Checked); err != nil {
				t.logger.Error("Failed to capture screen", "error", err)
				return
			}
			// Color update will happen via event callback
		}
	}
}

// HandleCanvasDrag returns a handler for canvas drag events.
func (t *SessionTab) HandleCanvasDrag(canvasWin *CanvasWindow) func(float32, float32, float32, float32) {
	return func(fromX, fromY, toX, toY float32) {
		if t.bridge == nil {
			return
		}

		if t.shouldSpreadToAll != nil && t.shouldSpreadToAll() {
			t.bridge.DragAll(float64(fromX), float64(fromY), float64(toX), float64(toY))
		} else {
			if err := t.bridge.Drag(t.sessionID, float64(fromX), float64(fromY), float64(toX), float64(toY)); err != nil {
				t.logger.Error("Drag failed", "error", err)
			}
		}
	}
}

func (t *SessionTab) updateColorFromCanvas(canvasWin *CanvasWindow, x, y int) {
	img := canvasWin.GetImage()
	if img == nil {
		return
	}

	bounds := img.Bounds()
	if x < 0 || y < 0 || x >= bounds.Max.X || y >= bounds.Max.Y {
		return
	}

	c := img.At(x, y)
	t.colorRect.FillColor = c
	t.colorRect.Refresh()
	t.colorEntry.SetText(colorToString(c))

	// Update nearby points
	var nearbyPoints strings.Builder
	for dy := -2; dy <= 2; dy++ {
		for dx := -2; dx <= 2; dx++ {
			px, py := x+dx, y+dy
			if px >= 0 && py >= 0 && px < bounds.Max.X && py < bounds.Max.Y {
				pointColor := img.At(px, py)
				nearbyPoints.WriteString(fmt.Sprintf("(%d, %d): %s\n", px, py, colorToString(pointColor)))
			}
		}
	}
	t.pointsArea.SetText(nearbyPoints.String())
}

func colorToString(c color.Color) string {
	r, g, b, a := c.RGBA()
	return fmt.Sprintf("RGBA(%d, %d, %d, %d)", r>>8, g>>8, b>>8, a>>8)
}

// State management

// IsScriptRunning returns whether a script is running.
func (t *SessionTab) IsScriptRunning() bool {
	t.stateMu.RLock()
	defer t.stateMu.RUnlock()
	return t.scriptRunning
}

// SetScriptRunning sets the script running state.
func (t *SessionTab) SetScriptRunning(running bool) {
	t.stateMu.Lock()
	t.scriptRunning = running
	t.stateMu.Unlock()

	if running {
		t.scriptBtn.SetText("Stop")
		t.scriptBtn.SetIcon(theme.MediaStopIcon())
		t.allScriptsBtn.SetText("Stop All")
		t.allScriptsBtn.SetIcon(theme.MediaStopIcon())
	} else {
		t.scriptBtn.SetText("Start")
		t.scriptBtn.SetIcon(theme.MediaPlayIcon())
		t.allScriptsBtn.SetText("Run All")
		t.allScriptsBtn.SetIcon(theme.MediaFastForwardIcon())
	}
	t.scriptBtn.Refresh()
	t.allScriptsBtn.Refresh()
}

// SetScriptSelection sets the selected script.
func (t *SessionTab) SetScriptSelection(scriptName string) {
	if t.scriptSelect != nil {
		t.suppressScriptSelectSync = true
		t.scriptSelect.SetSelected(scriptName)
		t.suppressScriptSelectSync = false
	}
}

// UpdateState updates the tab based on session state.
func (t *SessionTab) UpdateState(newState state.SessionState) {
	switch newState {
	case state.StateReady:
		t.EnableControls()
	case state.StateScriptRunning:
		t.SetScriptRunning(true)
	case state.StateStopped:
		t.DisableControls()
	}
}

// EnableControls enables all control buttons.
func (t *SessionTab) EnableControls() {
	t.stopBtn.Enable()
	t.refreshBtn.Enable()
	t.saveCookiesBtn.Enable()
	t.scriptBtn.Enable()
	t.scriptSelect.Enable()
	t.syncScriptBtn.Enable()
	t.allScriptsBtn.Enable()
	t.clickBtn.Enable()
}

// DisableControls disables all control buttons except stop.
func (t *SessionTab) DisableControls() {
	t.refreshBtn.Disable()
	t.saveCookiesBtn.Disable()
	t.scriptBtn.Disable()
	t.scriptSelect.Disable()
	t.syncScriptBtn.Disable()
	t.allScriptsBtn.Disable()
	t.clickBtn.Disable()
}

// StartScript starts the selected script.
// UI state is updated via OnScriptStarted event callback, not immediately.
func (t *SessionTab) StartScript() {
	if t.bridge == nil || t.scriptSelect.Selected == "" {
		return
	}

	// Just send command; UI state will be updated via event callback (OnScriptStarted)
	if err := t.bridge.StartScript(t.sessionID, t.scriptSelect.Selected); err != nil {
		t.logger.Error("Failed to start script", "error", err)
	}
}

// StopScript stops the running script.
// UI state is updated via OnScriptStopped event callback, not immediately.
func (t *SessionTab) StopScript() {
	if t.bridge == nil {
		return
	}

	// Just send command; UI state will be updated via event callback (OnScriptStopped)
	if err := t.bridge.StopScript(t.sessionID); err != nil {
		t.logger.Error("Failed to stop script", "error", err)
	}
}
