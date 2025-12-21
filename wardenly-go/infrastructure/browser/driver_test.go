package browser

import "testing"

func TestDefaultDriverConfig(t *testing.T) {
	config := DefaultDriverConfig()

	if config == nil {
		t.Fatal("DefaultDriverConfig returned nil")
	}

	if config.Headless != true {
		t.Errorf("Headless = %v, want true", config.Headless)
	}

	if config.WindowWidth != 1080 {
		t.Errorf("WindowWidth = %d, want 1080", config.WindowWidth)
	}

	if config.WindowHeight != 840 {
		t.Errorf("WindowHeight = %d, want 840", config.WindowHeight)
	}

	if config.ViewportWidth != 1080 {
		t.Errorf("ViewportWidth = %d, want 1080", config.ViewportWidth)
	}

	if config.ViewportHeight != 720 {
		t.Errorf("ViewportHeight = %d, want 720", config.ViewportHeight)
	}

	if config.MuteAudio != true {
		t.Errorf("MuteAudio = %v, want true", config.MuteAudio)
	}

	if config.HideScrollbars != true {
		t.Errorf("HideScrollbars = %v, want true", config.HideScrollbars)
	}

	if config.DisableWebSecurity != true {
		t.Errorf("DisableWebSecurity = %v, want true", config.DisableWebSecurity)
	}
}

func TestNewChromeDPDriver(t *testing.T) {
	t.Run("with nil config", func(t *testing.T) {
		driver := NewChromeDPDriver(nil)
		if driver == nil {
			t.Fatal("NewChromeDPDriver returned nil")
		}
		if driver.config == nil {
			t.Fatal("driver.config is nil")
		}
	})

	t.Run("with custom config", func(t *testing.T) {
		config := &DriverConfig{
			Headless:     false,
			WindowWidth:  1920,
			WindowHeight: 1080,
		}
		driver := NewChromeDPDriver(config)
		if driver == nil {
			t.Fatal("NewChromeDPDriver returned nil")
		}
		if driver.config.Headless != false {
			t.Error("Custom config not applied")
		}
		if driver.config.WindowWidth != 1920 {
			t.Error("Custom config not applied")
		}
	})
}

func TestChromeDPDriver_IsRunning_NotStarted(t *testing.T) {
	driver := NewChromeDPDriver(nil)

	if driver.IsRunning() {
		t.Error("IsRunning() should return false before Start()")
	}
}

func TestChromeDPDriver_Stop_NotStarted(t *testing.T) {
	driver := NewChromeDPDriver(nil)

	// Should not panic or error when stopping a driver that was never started
	err := driver.Stop()
	if err != nil {
		t.Errorf("Stop() returned error: %v", err)
	}
}

func TestPoint(t *testing.T) {
	p := Point{X: 100.5, Y: 200.5}

	if p.X != 100.5 {
		t.Errorf("X = %v, want 100.5", p.X)
	}
	if p.Y != 200.5 {
		t.Errorf("Y = %v, want 200.5", p.Y)
	}
}

func TestCookie(t *testing.T) {
	c := Cookie{
		Name:       "session",
		Value:      "abc123",
		Domain:     ".example.com",
		Path:       "/",
		HTTPOnly:   true,
		Secure:     true,
		SourcePort: 443,
	}

	if c.Name != "session" {
		t.Errorf("Name = %v, want session", c.Name)
	}
	if c.Value != "abc123" {
		t.Errorf("Value = %v, want abc123", c.Value)
	}
	if c.Domain != ".example.com" {
		t.Errorf("Domain = %v, want .example.com", c.Domain)
	}
	if !c.HTTPOnly {
		t.Error("HTTPOnly should be true")
	}
	if !c.Secure {
		t.Error("Secure should be true")
	}
}
