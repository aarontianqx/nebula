package session

import (
	"context"
	"fmt"
	"log/slog"

	"wardenly-go/infrastructure/browser"
)

// BrowserController handles browser operations for a session.
type BrowserController struct {
	driver browser.Driver
	logger *slog.Logger
}

// NewBrowserController creates a new browser controller.
func NewBrowserController(driver browser.Driver, logger *slog.Logger) *BrowserController {
	if logger == nil {
		logger = slog.Default()
	}
	return &BrowserController{
		driver: driver,
		logger: logger,
	}
}

// Click performs a mouse click at the specified coordinates.
func (c *BrowserController) Click(ctx context.Context, x, y float64) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.Click(ctx, x, y)
}

// Drag performs a mouse drag from one point to another.
func (c *BrowserController) Drag(ctx context.Context, fromX, fromY, toX, toY float64) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.Drag(ctx, fromX, fromY, toX, toY)
}

// DragPath performs a mouse drag along a path of points.
func (c *BrowserController) DragPath(ctx context.Context, points []browser.Point) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.DragPath(ctx, points)
}

// Refresh refreshes the current page.
func (c *BrowserController) Refresh(ctx context.Context) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.Reload(ctx)
}

// Navigate navigates to the specified URL.
func (c *BrowserController) Navigate(ctx context.Context, url string) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.Navigate(ctx, url)
}

// GetCookies retrieves all browser cookies.
func (c *BrowserController) GetCookies(ctx context.Context) ([]browser.Cookie, error) {
	if !c.driver.IsRunning() {
		return nil, fmt.Errorf("browser not running")
	}
	return c.driver.GetCookies(ctx)
}

// SetCookies sets browser cookies.
func (c *BrowserController) SetCookies(ctx context.Context, cookies []browser.Cookie) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.SetCookies(ctx, cookies)
}

// WaitVisible waits for an element to become visible.
func (c *BrowserController) WaitVisible(ctx context.Context, selector string) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.WaitVisible(ctx, selector)
}

// SendKeys sends keystrokes to an element.
func (c *BrowserController) SendKeys(ctx context.Context, selector, text string) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.SendKeys(ctx, selector, text)
}

// ClickElement clicks on an element by selector.
func (c *BrowserController) ClickElement(ctx context.Context, selector string) error {
	if !c.driver.IsRunning() {
		return fmt.Errorf("browser not running")
	}
	return c.driver.ClickElement(ctx, selector)
}

// IsRunning returns true if the browser is active.
func (c *BrowserController) IsRunning() bool {
	return c.driver.IsRunning()
}
