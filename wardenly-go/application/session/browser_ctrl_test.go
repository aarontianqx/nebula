package session

import (
	"context"
	"image"
	"testing"

	"wardenly-go/infrastructure/browser"
)

// mockDriver is a mock implementation of browser.Driver for testing.
type mockDriver struct {
	running        bool
	clickCalled    bool
	lastClickX     float64
	lastClickY     float64
	dragCalled     bool
	reloadCalled   bool
	navigateCalled bool
	lastURL        string
}

func newMockDriver() *mockDriver {
	return &mockDriver{running: true}
}

func (m *mockDriver) Start(ctx context.Context) error { return nil }
func (m *mockDriver) Stop() error                     { m.running = false; return nil }
func (m *mockDriver) IsRunning() bool                 { return m.running }
func (m *mockDriver) Navigate(ctx context.Context, url string) error {
	m.navigateCalled = true
	m.lastURL = url
	return nil
}
func (m *mockDriver) Reload(ctx context.Context) error {
	m.reloadCalled = true
	return nil
}
func (m *mockDriver) Click(ctx context.Context, x, y float64) error {
	m.clickCalled = true
	m.lastClickX = x
	m.lastClickY = y
	return nil
}
func (m *mockDriver) Drag(ctx context.Context, fromX, fromY, toX, toY float64) error {
	m.dragCalled = true
	return nil
}
func (m *mockDriver) DragPath(ctx context.Context, points []browser.Point) error {
	m.dragCalled = true
	return nil
}
func (m *mockDriver) CaptureScreen(ctx context.Context) (image.Image, error) {
	return image.NewRGBA(image.Rect(0, 0, 100, 100)), nil
}
func (m *mockDriver) SetViewport(ctx context.Context, width, height int) error  { return nil }
func (m *mockDriver) WaitVisible(ctx context.Context, selector string) error    { return nil }
func (m *mockDriver) SendKeys(ctx context.Context, selector, text string) error { return nil }
func (m *mockDriver) ClickElement(ctx context.Context, selector string) error   { return nil }
func (m *mockDriver) GetCookies(ctx context.Context) ([]browser.Cookie, error) {
	return []browser.Cookie{{Name: "test", Value: "value"}}, nil
}
func (m *mockDriver) SetCookies(ctx context.Context, cookies []browser.Cookie) error { return nil }
func (m *mockDriver) LoginWithPassword(url, username, password string, timeoutSeconds int) error {
	return nil
}
func (m *mockDriver) LoginWithCookies(url string, cookies []browser.Cookie, timeoutSeconds int) error {
	return nil
}
func (m *mockDriver) StartScreencast(ctx context.Context, quality, maxFPS int) (<-chan image.Image, error) {
	ch := make(chan image.Image)
	close(ch)
	return ch, nil
}
func (m *mockDriver) StopScreencast() error { return nil }
func (m *mockDriver) IsScreencasting() bool { return false }

func TestBrowserController_Click(t *testing.T) {
	driver := newMockDriver()
	ctrl := NewBrowserController(driver, nil)

	ctx := context.Background()
	err := ctrl.Click(ctx, 100.5, 200.5)

	if err != nil {
		t.Errorf("Click() error = %v", err)
	}
	if !driver.clickCalled {
		t.Error("Click was not called on driver")
	}
	if driver.lastClickX != 100.5 || driver.lastClickY != 200.5 {
		t.Errorf("Click coordinates = (%v, %v), want (100.5, 200.5)", driver.lastClickX, driver.lastClickY)
	}
}

func TestBrowserController_Click_NotRunning(t *testing.T) {
	driver := newMockDriver()
	driver.running = false
	ctrl := NewBrowserController(driver, nil)

	ctx := context.Background()
	err := ctrl.Click(ctx, 100, 200)

	if err == nil {
		t.Error("Expected error when browser not running")
	}
}

func TestBrowserController_Refresh(t *testing.T) {
	driver := newMockDriver()
	ctrl := NewBrowserController(driver, nil)

	ctx := context.Background()
	err := ctrl.Refresh(ctx)

	if err != nil {
		t.Errorf("Refresh() error = %v", err)
	}
	if !driver.reloadCalled {
		t.Error("Reload was not called on driver")
	}
}

func TestBrowserController_Navigate(t *testing.T) {
	driver := newMockDriver()
	ctrl := NewBrowserController(driver, nil)

	ctx := context.Background()
	err := ctrl.Navigate(ctx, "http://example.com")

	if err != nil {
		t.Errorf("Navigate() error = %v", err)
	}
	if !driver.navigateCalled {
		t.Error("Navigate was not called on driver")
	}
	if driver.lastURL != "http://example.com" {
		t.Errorf("URL = %v, want http://example.com", driver.lastURL)
	}
}

func TestBrowserController_GetCookies(t *testing.T) {
	driver := newMockDriver()
	ctrl := NewBrowserController(driver, nil)

	ctx := context.Background()
	cookies, err := ctrl.GetCookies(ctx)

	if err != nil {
		t.Errorf("GetCookies() error = %v", err)
	}
	if len(cookies) != 1 {
		t.Errorf("Cookies count = %d, want 1", len(cookies))
	}
	if cookies[0].Name != "test" {
		t.Errorf("Cookie name = %v, want test", cookies[0].Name)
	}
}

func TestBrowserController_IsRunning(t *testing.T) {
	driver := newMockDriver()
	ctrl := NewBrowserController(driver, nil)

	if !ctrl.IsRunning() {
		t.Error("IsRunning() = false, want true")
	}

	driver.running = false
	if ctrl.IsRunning() {
		t.Error("IsRunning() = true, want false")
	}
}
