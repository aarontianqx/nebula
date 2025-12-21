package browser

import (
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"image"
	"image/jpeg"
	"image/png"
	"sync"
	"time"

	"github.com/chromedp/cdproto/input"
	"github.com/chromedp/cdproto/network"
	"github.com/chromedp/cdproto/page"
	"github.com/chromedp/cdproto/storage"
	"github.com/chromedp/chromedp"
)

// ChromeDPDriver implements Driver using chromedp.
type ChromeDPDriver struct {
	config      *DriverConfig
	allocCtx    context.Context
	allocCancel context.CancelFunc
	ctx         context.Context
	cancel      context.CancelFunc
	mu          sync.Mutex
	running     bool

	// Screencast state
	screencastChan   chan image.Image
	screencastCancel context.CancelFunc
	screencasting    bool
}

// NewChromeDPDriver creates a new ChromeDP-based browser driver.
func NewChromeDPDriver(config *DriverConfig) *ChromeDPDriver {
	if config == nil {
		config = DefaultDriverConfig()
	}
	return &ChromeDPDriver{
		config: config,
	}
}

// buildExecAllocatorOptions builds chromedp options from config.
func (d *ChromeDPDriver) buildExecAllocatorOptions() []chromedp.ExecAllocatorOption {
	opts := append(chromedp.DefaultExecAllocatorOptions[:],
		chromedp.Flag("headless", d.config.Headless),
		chromedp.Flag("hide-scrollbars", d.config.HideScrollbars),
		chromedp.Flag("mute-audio", d.config.MuteAudio),
		chromedp.Flag("disable-gpu", d.config.DisableGPU),
		chromedp.Flag("disable-web-security", d.config.DisableWebSecurity),
		chromedp.Flag("disable-infobars", true),
		chromedp.Flag("enable-automation", false),
		chromedp.WindowSize(d.config.WindowWidth, d.config.WindowHeight),
	)

	if d.config.UserDataDir != "" {
		opts = append(opts, chromedp.UserDataDir(d.config.UserDataDir))
	}

	return opts
}

// Start initializes the browser instance.
func (d *ChromeDPDriver) Start(ctx context.Context) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if d.running {
		return fmt.Errorf("browser already running")
	}

	// Create allocator context from context.Background() to ensure browser lifecycle
	// is independent of the caller's context
	d.allocCtx, d.allocCancel = chromedp.NewExecAllocator(
		context.Background(),
		d.buildExecAllocatorOptions()...,
	)

	// Create browser context
	d.ctx, d.cancel = chromedp.NewContext(d.allocCtx)

	d.running = true
	return nil
}

// Stop closes the browser and releases resources.
func (d *ChromeDPDriver) Stop() error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if !d.running {
		return nil
	}

	d.cleanup()
	return nil
}

func (d *ChromeDPDriver) cleanup() {
	// Stop screencast if active
	if d.screencasting {
		d.stopScreencastInternal()
	}

	d.running = false
	if d.cancel != nil {
		d.cancel()
		d.cancel = nil
	}
	if d.allocCancel != nil {
		d.allocCancel()
		d.allocCancel = nil
	}
	d.ctx = nil
	d.allocCtx = nil
}

// IsRunning returns true if the browser is active.
func (d *ChromeDPDriver) IsRunning() bool {
	d.mu.Lock()
	defer d.mu.Unlock()
	return d.running
}

// Navigate navigates to the specified URL.
func (d *ChromeDPDriver) Navigate(ctx context.Context, url string) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	return chromedp.Run(browserCtx, chromedp.Navigate(url))
}

// Reload refreshes the current page.
func (d *ChromeDPDriver) Reload(ctx context.Context) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	return chromedp.Run(browserCtx, chromedp.Reload())
}

// Click performs a mouse click at the specified coordinates.
func (d *ChromeDPDriver) Click(ctx context.Context, x, y float64) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	// Add timeout protection
	timeoutCtx, cancel := context.WithTimeout(browserCtx, 5*time.Second)
	defer cancel()

	return chromedp.Run(timeoutCtx,
		chromedp.MouseClickXY(x, y, chromedp.ButtonLeft),
	)
}

// Drag performs a mouse drag from one point to another.
// It interpolates intermediate points for smooth, realistic dragging.
func (d *ChromeDPDriver) Drag(ctx context.Context, fromX, fromY, toX, toY float64) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	return chromedp.Run(browserCtx, chromedp.ActionFunc(func(ctx context.Context) error {
		const frameInterval = time.Second / 60 // ~16.67ms per frame

		// Press at start position
		p := &input.DispatchMouseEventParams{
			Type:       input.MousePressed,
			X:          fromX,
			Y:          fromY,
			Button:     input.Left,
			ClickCount: 1,
		}
		if err := p.Do(ctx); err != nil {
			return err
		}

		// Calculate intermediate points for smooth dragging (10 steps)
		const steps = 10
		deltaX := (toX - fromX) / float64(steps)
		deltaY := (toY - fromY) / float64(steps)

		// Simulate multiple mouse moves with frame-based timing
		for i := 1; i <= steps; i++ {
			p.Type = input.MouseMoved
			p.X = fromX + deltaX*float64(i)
			p.Y = fromY + deltaY*float64(i)

			if err := p.Do(ctx); err != nil {
				return err
			}

			time.Sleep(frameInterval)
		}

		// Release at end position
		p.Type = input.MouseReleased
		return p.Do(ctx)
	}))
}

// DragPath performs a mouse drag along a path of points.
// It uses frame-based timing (60fps) to simulate smooth, realistic dragging
// that games can properly interpret for movement calculation.
func (d *ChromeDPDriver) DragPath(ctx context.Context, points []Point) error {
	if len(points) < 2 {
		return fmt.Errorf("drag requires at least 2 points")
	}

	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	// Use ActionFunc for fine-grained control with delays
	return chromedp.Run(browserCtx, chromedp.ActionFunc(func(ctx context.Context) error {
		const frameInterval = time.Second / 60 // ~16.67ms per frame

		// Press at start position
		p := &input.DispatchMouseEventParams{
			Type:       input.MousePressed,
			X:          points[0].X,
			Y:          points[0].Y,
			Button:     input.Left,
			ClickCount: 1,
		}
		if err := p.Do(ctx); err != nil {
			return err
		}

		// Move through all intermediate points with frame-based timing
		for i := 1; i < len(points); i++ {
			p.Type = input.MouseMoved
			p.X = points[i].X
			p.Y = points[i].Y

			if err := p.Do(ctx); err != nil {
				return err
			}

			// Add frame delay between moves for smooth, realistic dragging
			time.Sleep(frameInterval)
		}

		// Release at end position
		p.Type = input.MouseReleased
		return p.Do(ctx)
	}))
}

// CaptureScreen captures the current browser screen.
func (d *ChromeDPDriver) CaptureScreen(ctx context.Context) (image.Image, error) {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return nil, fmt.Errorf("browser not running")
	}

	// Add timeout protection
	timeoutCtx, cancel := context.WithTimeout(browserCtx, 3*time.Second)
	defer cancel()

	var buf []byte
	if err := chromedp.Run(timeoutCtx, chromedp.CaptureScreenshot(&buf)); err != nil {
		return nil, fmt.Errorf("failed to capture screenshot: %w", err)
	}

	img, err := png.Decode(bytes.NewReader(buf))
	if err != nil {
		return nil, fmt.Errorf("failed to decode screenshot: %w", err)
	}

	return img, nil
}

// SetViewport sets the browser viewport size.
func (d *ChromeDPDriver) SetViewport(ctx context.Context, width, height int) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	return chromedp.Run(browserCtx,
		chromedp.EmulateViewport(int64(width), int64(height)),
	)
}

// WaitVisible waits for an element to become visible.
// The ctx parameter is used for timeout/cancellation - if it has a deadline,
// we create a derived context from browserCtx with that deadline.
func (d *ChromeDPDriver) WaitVisible(ctx context.Context, selector string) error {
	// Check if the provided context is already cancelled
	if ctx.Err() != nil {
		return ctx.Err()
	}

	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	// If the provided context has a deadline, create a timeout context from browserCtx
	execCtx := browserCtx
	if deadline, ok := ctx.Deadline(); ok {
		timeout := time.Until(deadline)
		if timeout <= 0 {
			return context.DeadlineExceeded
		}
		var cancel context.CancelFunc
		execCtx, cancel = context.WithTimeout(browserCtx, timeout)
		defer cancel()
	}

	// Run in a goroutine so we can also monitor the provided context for cancellation
	done := make(chan error, 1)
	go func() {
		done <- chromedp.Run(execCtx,
			chromedp.WaitVisible(selector, chromedp.ByID),
		)
	}()

	select {
	case err := <-done:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}

// SendKeys sends keystrokes to an element.
func (d *ChromeDPDriver) SendKeys(ctx context.Context, selector, text string) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	return chromedp.Run(browserCtx,
		chromedp.SendKeys(selector, text, chromedp.ByID),
	)
}

// ClickElement clicks on an element by selector.
func (d *ChromeDPDriver) ClickElement(ctx context.Context, selector string) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	return chromedp.Run(browserCtx,
		chromedp.Click(selector, chromedp.ByID),
	)
}

// GetCookies retrieves all browser cookies.
func (d *ChromeDPDriver) GetCookies(ctx context.Context) ([]Cookie, error) {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return nil, fmt.Errorf("browser not running")
	}

	var networkCookies []*network.Cookie
	if err := chromedp.Run(browserCtx,
		chromedp.ActionFunc(func(ctx context.Context) error {
			var err error
			networkCookies, err = storage.GetCookies().Do(ctx)
			return err
		}),
	); err != nil {
		return nil, fmt.Errorf("failed to get cookies: %w", err)
	}

	cookies := make([]Cookie, len(networkCookies))
	for i, nc := range networkCookies {
		cookies[i] = Cookie{
			Name:         nc.Name,
			Value:        nc.Value,
			Domain:       nc.Domain,
			Path:         nc.Path,
			HTTPOnly:     nc.HTTPOnly,
			Secure:       nc.Secure,
			SourcePort:   int(nc.SourcePort),
			SourceScheme: string(nc.SourceScheme),
			Priority:     string(nc.Priority),
		}
	}

	return cookies, nil
}

// SetCookies sets browser cookies.
func (d *ChromeDPDriver) SetCookies(ctx context.Context, cookies []Cookie) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	actions := make([]chromedp.Action, len(cookies))
	for i, c := range cookies {
		cookie := c // capture for closure
		actions[i] = chromedp.ActionFunc(func(ctx context.Context) error {
			setCookie := network.SetCookie(cookie.Name, cookie.Value).
				WithDomain(cookie.Domain).
				WithPath(cookie.Path).
				WithHTTPOnly(cookie.HTTPOnly).
				WithSecure(cookie.Secure).
				WithSourcePort(int64(cookie.SourcePort))

			// Add optional fields if present
			if cookie.Priority != "" {
				setCookie = setCookie.WithPriority(network.CookiePriority(cookie.Priority))
			}
			if cookie.SourceScheme != "" {
				setCookie = setCookie.WithSourceScheme(network.CookieSourceScheme(cookie.SourceScheme))
			}

			return setCookie.Do(ctx)
		})
	}

	return chromedp.Run(browserCtx, actions...)
}

// Context returns the underlying chromedp context.
// This is useful for advanced operations not covered by the Driver interface.
func (d *ChromeDPDriver) Context() context.Context {
	d.mu.Lock()
	defer d.mu.Unlock()
	return d.ctx
}

// LoginWithPassword performs a complete login flow with username and password.
// This executes all steps in a single chromedp.Run call for better reliability,
// matching the behavior of the original implementation.
func (d *ChromeDPDriver) LoginWithPassword(url, username, password string, timeoutSeconds int) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	// Step 1: Set viewport and navigate (without timeout, let it complete)
	err := chromedp.Run(browserCtx,
		chromedp.EmulateViewport(1080, 720, chromedp.EmulateScale(1)),
		chromedp.Navigate(url),
	)
	if err != nil {
		return fmt.Errorf("start page failure: %w", err)
	}

	// Step 2: Wait for login form, enter credentials, and submit (with timeout)
	ctx, cancel := context.WithTimeout(browserCtx, time.Duration(timeoutSeconds)*time.Second)
	defer cancel()

	err = chromedp.Run(ctx,
		chromedp.EmulateViewport(1080, 720, chromedp.EmulateScale(1)),
		chromedp.Navigate(url),
		chromedp.WaitVisible(`#username`, chromedp.ByID),
		chromedp.SendKeys(`#username`, username, chromedp.ByID),
		chromedp.SendKeys(`#userpwd`, password, chromedp.ByID),
		chromedp.Click(`#form1 > div.r06 > div.login_box3 > p > input`, chromedp.ByID),
		chromedp.WaitVisible(`#S_Iframe`, chromedp.ByID),
	)
	if err != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return fmt.Errorf("login timeout after %ds (server may be down or in maintenance)", timeoutSeconds)
		}
		return fmt.Errorf("login failure: %w", err)
	}

	return nil
}

// LoginWithCookies performs login using stored cookies.
// This executes all steps in a single chromedp.Run call for better reliability,
// matching the behavior of the original implementation.
func (d *ChromeDPDriver) LoginWithCookies(url string, cookies []Cookie, timeoutSeconds int) error {
	d.mu.Lock()
	browserCtx := d.ctx
	running := d.running
	d.mu.Unlock()

	if !running || browserCtx == nil {
		return fmt.Errorf("browser not running")
	}

	// Build cookie actions
	cookieActions := make([]chromedp.Action, len(cookies))
	for i, c := range cookies {
		cookie := c // capture for closure
		cookieActions[i] = chromedp.ActionFunc(func(ctx context.Context) error {
			setCookie := network.SetCookie(cookie.Name, cookie.Value).
				WithDomain(cookie.Domain).
				WithPath(cookie.Path).
				WithHTTPOnly(cookie.HTTPOnly).
				WithSecure(cookie.Secure).
				WithSourcePort(int64(cookie.SourcePort))

			if cookie.Priority != "" {
				setCookie = setCookie.WithPriority(network.CookiePriority(cookie.Priority))
			}
			if cookie.SourceScheme != "" {
				setCookie = setCookie.WithSourceScheme(network.CookieSourceScheme(cookie.SourceScheme))
			}

			return setCookie.Do(ctx)
		})
	}

	// Step 1: Set cookies and navigate (without timeout)
	actions := append(cookieActions,
		chromedp.EmulateViewport(1080, 720, chromedp.EmulateScale(1)),
		chromedp.Navigate(url),
	)
	err := chromedp.Run(browserCtx, actions...)
	if err != nil {
		return fmt.Errorf("start page failure: %w", err)
	}

	// Step 2: Wait for game iframe (with timeout)
	ctx, cancel := context.WithTimeout(browserCtx, time.Duration(timeoutSeconds)*time.Second)
	defer cancel()

	err = chromedp.Run(ctx,
		chromedp.WaitVisible(`#S_Iframe`, chromedp.ByID),
	)
	if err != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return fmt.Errorf("login timeout after %ds (server may be down or in maintenance)", timeoutSeconds)
		}
		return fmt.Errorf("login failure: %w", err)
	}

	return nil
}

// StartScreencast starts frame streaming from the browser.
// Returns a channel that receives decoded frames.
// quality: JPEG quality 0-100, maxFPS: maximum frames per second
func (d *ChromeDPDriver) StartScreencast(ctx context.Context, quality, maxFPS int) (<-chan image.Image, error) {
	d.mu.Lock()
	defer d.mu.Unlock()

	if !d.running || d.ctx == nil {
		return nil, fmt.Errorf("browser not running")
	}

	if d.screencasting {
		return nil, fmt.Errorf("screencast already active")
	}

	// Create screencast channel and cancellation context
	d.screencastChan = make(chan image.Image, 5) // Buffer a few frames
	screencastCtx, screencastCancel := context.WithCancel(d.ctx)
	d.screencastCancel = screencastCancel
	d.screencasting = true

	browserCtx := d.ctx
	frameChan := d.screencastChan

	// Start listener for screencast frames
	chromedp.ListenTarget(screencastCtx, func(ev interface{}) {
		switch e := ev.(type) {
		case *page.EventScreencastFrame:
			// Decode base64 frame data
			frameData, err := base64.StdEncoding.DecodeString(e.Data)
			if err != nil {
				return
			}

			// Decode JPEG image
			img, err := jpeg.Decode(bytes.NewReader(frameData))
			if err != nil {
				return
			}

			// Send frame to channel (non-blocking)
			select {
			case frameChan <- img:
			default:
				// Channel full, drop frame
			}

			// Acknowledge the frame to receive the next one
			go func() {
				_ = chromedp.Run(screencastCtx,
					chromedp.ActionFunc(func(ctx context.Context) error {
						return page.ScreencastFrameAck(e.SessionID).Do(ctx)
					}),
				)
			}()
		}
	})

	// Start screencast
	err := chromedp.Run(browserCtx,
		chromedp.ActionFunc(func(ctx context.Context) error {
			return page.StartScreencast().
				WithFormat(page.ScreencastFormatJpeg).
				WithQuality(int64(quality)).
				WithMaxWidth(int64(d.config.ViewportWidth)).
				WithMaxHeight(int64(d.config.ViewportHeight)).
				WithEveryNthFrame(int64(60 / maxFPS)). // Convert FPS to frame skip
				Do(ctx)
		}),
	)
	if err != nil {
		d.stopScreencastInternal()
		return nil, fmt.Errorf("failed to start screencast: %w", err)
	}

	return d.screencastChan, nil
}

// StopScreencast stops frame streaming.
func (d *ChromeDPDriver) StopScreencast() error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if !d.screencasting {
		return nil
	}

	return d.stopScreencastInternal()
}

// stopScreencastInternal stops screencast without locking (must be called with lock held).
func (d *ChromeDPDriver) stopScreencastInternal() error {
	if !d.screencasting {
		return nil
	}

	// Stop screencast on browser
	if d.ctx != nil {
		_ = chromedp.Run(d.ctx,
			chromedp.ActionFunc(func(ctx context.Context) error {
				return page.StopScreencast().Do(ctx)
			}),
		)
	}

	// Cancel listener context
	if d.screencastCancel != nil {
		d.screencastCancel()
		d.screencastCancel = nil
	}

	// Close channel
	if d.screencastChan != nil {
		close(d.screencastChan)
		d.screencastChan = nil
	}

	d.screencasting = false
	return nil
}

// IsScreencasting returns true if screencast is active.
func (d *ChromeDPDriver) IsScreencasting() bool {
	d.mu.Lock()
	defer d.mu.Unlock()
	return d.screencasting
}

// Ensure ChromeDPDriver implements Driver
var _ Driver = (*ChromeDPDriver)(nil)
