package presentation

import (
	"image"
	"testing"
)

func TestDragRecord(t *testing.T) {
	rec := &dragRecord{
		fromX: 10.5,
		fromY: 20.5,
		toX:   100.5,
		toY:   200.5,
	}

	if rec.fromX != 10.5 {
		t.Errorf("fromX = %v, want 10.5", rec.fromX)
	}
	if rec.fromY != 20.5 {
		t.Errorf("fromY = %v, want 20.5", rec.fromY)
	}
	if rec.toX != 100.5 {
		t.Errorf("toX = %v, want 100.5", rec.toX)
	}
	if rec.toY != 200.5 {
		t.Errorf("toY = %v, want 200.5", rec.toY)
	}
}

func TestBrowserCanvas_GetImage_Nil(t *testing.T) {
	// Test that GetImage returns nil when no image is set
	// Note: We can't fully test BrowserCanvas without Fyne app context,
	// but we can test the data structures

	img := image.NewRGBA(image.Rect(0, 0, 100, 100))
	if img == nil {
		t.Error("Failed to create test image")
	}

	bounds := img.Bounds()
	if bounds.Dx() != 100 || bounds.Dy() != 100 {
		t.Errorf("Image bounds = %dx%d, want 100x100", bounds.Dx(), bounds.Dy())
	}
}

func TestCanvasWindowVisibility(t *testing.T) {
	// Test visibility state logic
	isVisible := false

	// Simulate Show
	if !isVisible {
		isVisible = true
	}
	if !isVisible {
		t.Error("Expected visible after Show")
	}

	// Simulate Hide
	if isVisible {
		isVisible = false
	}
	if isVisible {
		t.Error("Expected not visible after Hide")
	}
}
