package session

import (
	"context"
	"fmt"
	"image"
	"image/png"
	"log/slog"
	"os"
	"path/filepath"
	"time"

	"wardenly-go/infrastructure/browser"
)

// ScreenCapture handles screen capture operations for a session.
type ScreenCapture struct {
	driver  browser.Driver
	logger  *slog.Logger
	saveDir string
}

// NewScreenCapture creates a new screen capture service.
func NewScreenCapture(driver browser.Driver, logger *slog.Logger) *ScreenCapture {
	if logger == nil {
		logger = slog.Default()
	}
	return &ScreenCapture{
		driver:  driver,
		logger:  logger,
		saveDir: getDefaultSaveDir(),
	}
}

// getDefaultSaveDir returns the default directory for saving screenshots.
func getDefaultSaveDir() string {
	// Try to use user's Pictures folder
	home, err := os.UserHomeDir()
	if err != nil {
		return "."
	}
	return filepath.Join(home, "Pictures", "snapshot")
}

// SetSaveDir sets the directory for saving screenshots.
func (s *ScreenCapture) SetSaveDir(dir string) {
	s.saveDir = dir
}

// Capture captures the current browser screen.
func (s *ScreenCapture) Capture(ctx context.Context) (image.Image, error) {
	if !s.driver.IsRunning() {
		return nil, fmt.Errorf("browser not running")
	}
	return s.driver.CaptureScreen(ctx)
}

// CaptureAndSave captures the screen and saves it to a file.
func (s *ScreenCapture) CaptureAndSave(ctx context.Context) (image.Image, string, error) {
	img, err := s.Capture(ctx)
	if err != nil {
		return nil, "", err
	}

	filename, err := s.saveImage(img)
	if err != nil {
		return img, "", err
	}

	return img, filename, nil
}

// SaveToFile saves an image to a file.
func (s *ScreenCapture) SaveToFile(img image.Image) error {
	_, err := s.saveImage(img)
	return err
}

// saveImage saves an image to the configured directory.
func (s *ScreenCapture) saveImage(img image.Image) (string, error) {
	// Ensure directory exists
	if err := os.MkdirAll(s.saveDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create save directory: %w", err)
	}

	// Generate filename
	filename := filepath.Join(s.saveDir, fmt.Sprintf("%d.png", time.Now().UnixMilli()))

	// Create file
	f, err := os.Create(filename)
	if err != nil {
		return "", fmt.Errorf("failed to create file: %w", err)
	}
	defer f.Close()

	// Encode and save
	if err := png.Encode(f, img); err != nil {
		return "", fmt.Errorf("failed to encode image: %w", err)
	}

	s.logger.Debug("Screenshot saved", "filename", filename)
	return filename, nil
}

// CropImage crops an image to the specified region.
func (s *ScreenCapture) CropImage(img image.Image, x, y, width, height int) (image.Image, error) {
	subImager, ok := img.(interface {
		SubImage(r image.Rectangle) image.Image
	})
	if !ok {
		return nil, fmt.Errorf("image does not support SubImage")
	}

	rect := image.Rect(x, y, x+width, y+height)
	return subImager.SubImage(rect), nil
}
