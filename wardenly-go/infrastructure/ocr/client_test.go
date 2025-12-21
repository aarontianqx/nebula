package ocr

import "testing"

func TestDefaultClientConfig(t *testing.T) {
	config := DefaultClientConfig()

	if config == nil {
		t.Fatal("DefaultClientConfig returned nil")
	}

	if config.BaseURL != "http://localhost:8000" {
		t.Errorf("BaseURL = %v, want http://localhost:8000", config.BaseURL)
	}

	if config.Timeout != 30*1e9 {
		t.Errorf("Timeout = %v, want 30s", config.Timeout)
	}

	if config.HealthInterval != 5*1e9 {
		t.Errorf("HealthInterval = %v, want 5s", config.HealthInterval)
	}

	if config.HealthTimeout != 3*1e9 {
		t.Errorf("HealthTimeout = %v, want 3s", config.HealthTimeout)
	}
}

func TestROI(t *testing.T) {
	roi := ROI{
		X:      100,
		Y:      200,
		Width:  300,
		Height: 400,
	}

	if roi.X != 100 {
		t.Errorf("X = %d, want 100", roi.X)
	}
	if roi.Y != 200 {
		t.Errorf("Y = %d, want 200", roi.Y)
	}
	if roi.Width != 300 {
		t.Errorf("Width = %d, want 300", roi.Width)
	}
	if roi.Height != 400 {
		t.Errorf("Height = %d, want 400", roi.Height)
	}
}

func TestUsageRatioResult(t *testing.T) {
	result := UsageRatioResult{
		Numerator:   1,
		Denominator: 10,
		RawText:     "1/10",
		Confidence:  0.95,
		ElapsedMs:   50.5,
	}

	if result.Numerator != 1 {
		t.Errorf("Numerator = %d, want 1", result.Numerator)
	}
	if result.Denominator != 10 {
		t.Errorf("Denominator = %d, want 10", result.Denominator)
	}
	if result.RawText != "1/10" {
		t.Errorf("RawText = %s, want 1/10", result.RawText)
	}
	if result.Confidence != 0.95 {
		t.Errorf("Confidence = %v, want 0.95", result.Confidence)
	}
}

func TestNoOpClient(t *testing.T) {
	client := NewNoOpClient()

	t.Run("IsHealthy", func(t *testing.T) {
		if client.IsHealthy() {
			t.Error("NoOpClient.IsHealthy() should return false")
		}
	})

	t.Run("RecognizeUsageRatio", func(t *testing.T) {
		_, err := client.RecognizeUsageRatio(nil, nil, nil)
		if err == nil {
			t.Error("NoOpClient.RecognizeUsageRatio() should return error")
		}
	})

	t.Run("RecognizeUsageRatioFromImage", func(t *testing.T) {
		_, err := client.RecognizeUsageRatioFromImage(nil, nil, nil)
		if err == nil {
			t.Error("NoOpClient.RecognizeUsageRatioFromImage() should return error")
		}
	})

	t.Run("Close", func(t *testing.T) {
		// Should not panic
		client.Close()
	})
}
