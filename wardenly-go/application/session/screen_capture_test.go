package session

import (
	"context"
	"image"
	"testing"
)

func TestScreenCapture_Capture(t *testing.T) {
	driver := newMockDriver()
	cap := NewScreenCapture(driver, nil)

	ctx := context.Background()
	img, err := cap.Capture(ctx)

	if err != nil {
		t.Errorf("Capture() error = %v", err)
	}
	if img == nil {
		t.Error("Capture() returned nil image")
	}
}

func TestScreenCapture_Capture_NotRunning(t *testing.T) {
	driver := newMockDriver()
	driver.running = false
	cap := NewScreenCapture(driver, nil)

	ctx := context.Background()
	_, err := cap.Capture(ctx)

	if err == nil {
		t.Error("Expected error when browser not running")
	}
}

func TestScreenCapture_SetSaveDir(t *testing.T) {
	driver := newMockDriver()
	cap := NewScreenCapture(driver, nil)

	cap.SetSaveDir("/tmp/screenshots")

	if cap.saveDir != "/tmp/screenshots" {
		t.Errorf("saveDir = %v, want /tmp/screenshots", cap.saveDir)
	}
}

func TestScreenCapture_CropImage(t *testing.T) {
	driver := newMockDriver()
	cap := NewScreenCapture(driver, nil)

	// Create a test image
	img := image.NewRGBA(image.Rect(0, 0, 1000, 1000))

	cropped, err := cap.CropImage(img, 100, 100, 200, 200)

	if err != nil {
		t.Errorf("CropImage() error = %v", err)
	}
	if cropped == nil {
		t.Error("CropImage() returned nil")
	}

	bounds := cropped.Bounds()
	if bounds.Dx() != 200 || bounds.Dy() != 200 {
		t.Errorf("Cropped size = %dx%d, want 200x200", bounds.Dx(), bounds.Dy())
	}
}
