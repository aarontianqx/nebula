// Package browser provides browser automation infrastructure.
package browser

import (
	"context"
	"image"
)

// Driver defines the interface for browser automation.
// This abstraction allows for different browser implementations (ChromeDP, Playwright, etc.)
type Driver interface {
	// Start initializes the browser instance.
	Start(ctx context.Context) error

	// Stop closes the browser and releases resources.
	Stop() error

	// IsRunning returns true if the browser is active.
	IsRunning() bool

	// Navigate navigates to the specified URL.
	Navigate(ctx context.Context, url string) error

	// Reload refreshes the current page.
	Reload(ctx context.Context) error

	// Click performs a mouse click at the specified coordinates.
	Click(ctx context.Context, x, y float64) error

	// Drag performs a mouse drag from one point to another.
	Drag(ctx context.Context, fromX, fromY, toX, toY float64) error

	// DragPath performs a mouse drag along a path of points.
	DragPath(ctx context.Context, points []Point) error

	// CaptureScreen captures the current browser screen.
	CaptureScreen(ctx context.Context) (image.Image, error)

	// SetViewport sets the browser viewport size.
	SetViewport(ctx context.Context, width, height int) error

	// WaitVisible waits for an element to become visible.
	WaitVisible(ctx context.Context, selector string) error

	// SendKeys sends keystrokes to an element.
	SendKeys(ctx context.Context, selector, text string) error

	// ClickElement clicks on an element by selector.
	ClickElement(ctx context.Context, selector string) error

	// GetCookies retrieves all browser cookies.
	GetCookies(ctx context.Context) ([]Cookie, error)

	// SetCookies sets browser cookies.
	SetCookies(ctx context.Context, cookies []Cookie) error

	// LoginWithPassword performs a complete login flow with username and password.
	// This executes all steps in a single chromedp.Run call for better reliability.
	LoginWithPassword(url, username, password string, timeoutSeconds int) error

	// LoginWithCookies performs login using stored cookies.
	// This executes all steps in a single chromedp.Run call for better reliability.
	LoginWithCookies(url string, cookies []Cookie, timeoutSeconds int) error

	// StartScreencast starts frame streaming from the browser.
	// Returns a channel that receives decoded frames.
	// quality: JPEG quality 0-100, maxFPS: maximum frames per second
	StartScreencast(ctx context.Context, quality, maxFPS int) (<-chan image.Image, error)

	// StopScreencast stops frame streaming.
	StopScreencast() error

	// IsScreencasting returns true if screencast is active.
	IsScreencasting() bool
}

// Point represents a coordinate.
type Point struct {
	X, Y float64
}

// Cookie represents a browser cookie.
type Cookie struct {
	Name         string
	Value        string
	Domain       string
	Path         string
	HTTPOnly     bool
	Secure       bool
	SourcePort   int
	SourceScheme string
	Priority     string
}

// DriverConfig holds configuration for browser drivers.
type DriverConfig struct {
	// Headless runs the browser without a visible window.
	Headless bool

	// WindowWidth is the browser window width.
	WindowWidth int

	// WindowHeight is the browser window height.
	WindowHeight int

	// ViewportWidth is the viewport width.
	ViewportWidth int

	// ViewportHeight is the viewport height.
	ViewportHeight int

	// DisableGPU disables GPU acceleration.
	DisableGPU bool

	// MuteAudio mutes browser audio.
	MuteAudio bool

	// HideScrollbars hides scrollbars.
	HideScrollbars bool

	// DisableWebSecurity disables web security (allows cross-origin).
	DisableWebSecurity bool

	// UserDataDir specifies a custom user data directory.
	UserDataDir string
}

// DefaultDriverConfig returns default browser configuration.
func DefaultDriverConfig() *DriverConfig {
	return &DriverConfig{
		Headless:           true,
		WindowWidth:        1080,
		WindowHeight:       840,
		ViewportWidth:      1080,
		ViewportHeight:     720,
		DisableGPU:         false,
		MuteAudio:          true,
		HideScrollbars:     true,
		DisableWebSecurity: true,
	}
}
