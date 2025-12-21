// Package ocr provides OCR service client infrastructure.
package ocr

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"image"
	"image/png"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"sync"
	"sync/atomic"
	"time"
)

// Client provides OCR recognition services.
type Client interface {
	// RecognizeUsageRatio recognizes a usage ratio (e.g., "1/10") from image bytes.
	RecognizeUsageRatio(ctx context.Context, imageBytes []byte, roi *ROI) (*UsageRatioResult, error)

	// RecognizeUsageRatioFromImage recognizes a usage ratio from an image.Image.
	RecognizeUsageRatioFromImage(ctx context.Context, img image.Image, roi *ROI) (*UsageRatioResult, error)

	// IsHealthy returns true if the OCR service is available.
	IsHealthy() bool

	// Close releases resources.
	Close()
}

// ROI defines the region of interest for OCR.
type ROI struct {
	X      int
	Y      int
	Width  int
	Height int
}

// UsageRatioResult contains the OCR recognition result.
type UsageRatioResult struct {
	Numerator   int
	Denominator int
	RawText     string
	Confidence  float64
	ElapsedMs   float64
}

// ClientConfig contains configuration for the OCR client.
type ClientConfig struct {
	BaseURL        string
	Timeout        time.Duration
	HealthInterval time.Duration
	HealthTimeout  time.Duration
}

// DefaultClientConfig returns default OCR client configuration.
func DefaultClientConfig() *ClientConfig {
	return &ClientConfig{
		BaseURL:        "http://localhost:8000",
		Timeout:        30 * time.Second,
		HealthInterval: 5 * time.Second,
		HealthTimeout:  3 * time.Second,
	}
}

// HTTPClient implements Client using HTTP calls to a FastAPI backend.
type HTTPClient struct {
	config       *ClientConfig
	httpClient   *http.Client
	healthy      atomic.Bool
	healthCtx    context.Context
	healthCancel context.CancelFunc
	healthWg     sync.WaitGroup
}

// NewHTTPClient creates a new HTTP-based OCR client.
func NewHTTPClient(config *ClientConfig) *HTTPClient {
	if config == nil {
		config = DefaultClientConfig()
	}

	ctx, cancel := context.WithCancel(context.Background())

	client := &HTTPClient{
		config: config,
		httpClient: &http.Client{
			Timeout: config.Timeout,
		},
		healthCtx:    ctx,
		healthCancel: cancel,
	}

	// Perform initial health check
	client.performHealthCheck()

	// Start background health check loop
	client.healthWg.Add(1)
	go client.healthCheckLoop()

	return client
}

// RecognizeUsageRatio recognizes a usage ratio from image bytes.
func (c *HTTPClient) RecognizeUsageRatio(ctx context.Context, imageBytes []byte, roi *ROI) (*UsageRatioResult, error) {
	if !c.IsHealthy() {
		return nil, fmt.Errorf("OCR service is currently unavailable")
	}

	// Build request URL
	requestURL := fmt.Sprintf("%s/v1/ratios/usage", c.config.BaseURL)
	if roi != nil {
		params := url.Values{}
		params.Add("x", strconv.Itoa(roi.X))
		params.Add("y", strconv.Itoa(roi.Y))
		params.Add("width", strconv.Itoa(roi.Width))
		params.Add("height", strconv.Itoa(roi.Height))
		requestURL = fmt.Sprintf("%s?%s", requestURL, params.Encode())
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, requestURL, bytes.NewReader(imageBytes))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/octet-stream")

	// Execute request
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	// Read response
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode == http.StatusNotFound {
		return nil, fmt.Errorf("no ratio found in image")
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(body))
	}

	// Parse response
	var apiResp struct {
		Numerator   int `json:"numerator"`
		Denominator int `json:"denominator"`
		Debug       struct {
			RawText    string  `json:"raw_text"`
			Confidence float64 `json:"confidence"`
			ElapsedMs  float64 `json:"elapsed_ms"`
		} `json:"debug"`
	}

	if err := json.Unmarshal(body, &apiResp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return &UsageRatioResult{
		Numerator:   apiResp.Numerator,
		Denominator: apiResp.Denominator,
		RawText:     apiResp.Debug.RawText,
		Confidence:  apiResp.Debug.Confidence,
		ElapsedMs:   apiResp.Debug.ElapsedMs,
	}, nil
}

// RecognizeUsageRatioFromImage recognizes a usage ratio from an image.Image.
func (c *HTTPClient) RecognizeUsageRatioFromImage(ctx context.Context, img image.Image, roi *ROI) (*UsageRatioResult, error) {
	var targetImg image.Image
	var remoteROI *ROI

	// Crop locally if possible to reduce network transfer
	if roi != nil {
		if subImager, ok := img.(interface {
			SubImage(r image.Rectangle) image.Image
		}); ok {
			rect := image.Rect(roi.X, roi.Y, roi.X+roi.Width, roi.Y+roi.Height)
			targetImg = subImager.SubImage(rect)
			remoteROI = nil
		} else {
			targetImg = img
			remoteROI = roi
		}
	} else {
		targetImg = img
		remoteROI = nil
	}

	// Encode to PNG
	buf := new(bytes.Buffer)
	if err := png.Encode(buf, targetImg); err != nil {
		return nil, fmt.Errorf("failed to encode image: %w", err)
	}

	return c.RecognizeUsageRatio(ctx, buf.Bytes(), remoteROI)
}

// IsHealthy returns true if the OCR service is available.
func (c *HTTPClient) IsHealthy() bool {
	return c.healthy.Load()
}

// Close releases resources.
func (c *HTTPClient) Close() {
	if c.healthCancel != nil {
		c.healthCancel()
	}
	c.healthWg.Wait()
}

func (c *HTTPClient) healthCheckLoop() {
	defer c.healthWg.Done()

	ticker := time.NewTicker(c.config.HealthInterval)
	defer ticker.Stop()

	for {
		select {
		case <-c.healthCtx.Done():
			return
		case <-ticker.C:
			c.performHealthCheck()
		}
	}
}

func (c *HTTPClient) performHealthCheck() {
	ctx, cancel := context.WithTimeout(c.healthCtx, c.config.HealthTimeout)
	defer cancel()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, fmt.Sprintf("%s/health", c.config.BaseURL), nil)
	if err != nil {
		c.healthy.Store(false)
		return
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		c.healthy.Store(false)
		return
	}
	defer resp.Body.Close()

	c.healthy.Store(resp.StatusCode == http.StatusOK)
}

// Ensure HTTPClient implements Client
var _ Client = (*HTTPClient)(nil)

// NoOpClient is a no-operation OCR client for testing or when OCR is disabled.
type NoOpClient struct{}

// NewNoOpClient creates a no-operation OCR client.
func NewNoOpClient() *NoOpClient {
	return &NoOpClient{}
}

func (c *NoOpClient) RecognizeUsageRatio(ctx context.Context, imageBytes []byte, roi *ROI) (*UsageRatioResult, error) {
	return nil, fmt.Errorf("OCR is disabled")
}

func (c *NoOpClient) RecognizeUsageRatioFromImage(ctx context.Context, img image.Image, roi *ROI) (*UsageRatioResult, error) {
	return nil, fmt.Errorf("OCR is disabled")
}

func (c *NoOpClient) IsHealthy() bool {
	return false
}

func (c *NoOpClient) Close() {}

var _ Client = (*NoOpClient)(nil)
