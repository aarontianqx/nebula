// Package scene defines scene recognition types for visual automation.
package scene

import (
	"image"
	"image/color"
)

// Scene represents a recognizable game state defined by color points.
type Scene struct {
	// Name is the unique identifier for this scene
	Name string

	// Category groups related scenes (e.g., "city", "battle", "loading")
	Category string

	// Points are the color checkpoints used to identify this scene
	Points []Point

	// Actions are predefined actions available in this scene
	Actions map[string]Action
}

// Point represents a coordinate with an expected color for scene matching.
type Point struct {
	X     int
	Y     int
	Color color.RGBA
}

// Action represents a predefined action within a scene.
type Action struct {
	Type  ActionType
	Point ActionPoint
}

// ActionType represents the type of action.
type ActionType string

const (
	ActionTypeClick      ActionType = "click"
	ActionTypeWait       ActionType = "wait"
	ActionTypeDrag       ActionType = "drag"
	ActionTypeQuit       ActionType = "quit"
	ActionTypeIncr       ActionType = "incr"
	ActionTypeDecr       ActionType = "decr"
	ActionTypeCheckScene ActionType = "check_scene"
)

// ActionPoint represents coordinates for an action.
type ActionPoint struct {
	X float64
	Y float64
}

// Matcher provides scene matching functionality.
type Matcher struct {
	// Threshold is the maximum average color difference allowed for a match.
	// Lower values require more precise matches.
	Threshold float64
}

// NewMatcher creates a new scene matcher with the specified threshold.
func NewMatcher(threshold float64) *Matcher {
	if threshold <= 0 {
		threshold = 5.0 // Default threshold
	}
	return &Matcher{Threshold: threshold}
}

// Match checks if the given image matches this scene.
func (m *Matcher) Match(scene *Scene, img image.Image) bool {
	if len(scene.Points) == 0 || img == nil {
		return false
	}

	var totalDiff float64
	for _, point := range scene.Points {
		diff := colorDiff(img.At(point.X, point.Y), point.Color)
		totalDiff += diff
	}

	avgDiff := totalDiff / float64(len(scene.Points))
	return avgDiff <= m.Threshold
}

// MatchResult contains details about a scene match attempt.
type MatchResult struct {
	Scene      *Scene
	Matched    bool
	AvgDiff    float64
	PointDiffs []float64
}

// MatchWithDetails performs matching and returns detailed results.
func (m *Matcher) MatchWithDetails(scene *Scene, img image.Image) *MatchResult {
	result := &MatchResult{
		Scene:      scene,
		PointDiffs: make([]float64, len(scene.Points)),
	}

	if len(scene.Points) == 0 || img == nil {
		return result
	}

	var totalDiff float64
	for i, point := range scene.Points {
		diff := colorDiff(img.At(point.X, point.Y), point.Color)
		result.PointDiffs[i] = diff
		totalDiff += diff
	}

	result.AvgDiff = totalDiff / float64(len(scene.Points))
	result.Matched = result.AvgDiff <= m.Threshold

	return result
}

// colorDiff calculates the color difference between two colors.
// Returns a value between 0 (identical) and higher values for more difference.
func colorDiff(c1, c2 color.Color) float64 {
	r1, g1, b1, _ := c1.RGBA()
	r2, g2, b2, _ := c2.RGBA()

	// Convert from 16-bit to 8-bit
	r1, g1, b1 = r1>>8, g1>>8, b1>>8
	r2, g2, b2 = r2>>8, g2>>8, b2>>8

	// Calculate absolute differences
	dr := absDiff(r1, r2)
	dg := absDiff(g1, g2)
	db := absDiff(b1, b2)

	// Return average difference
	return float64(dr+dg+db) / 3.0
}

func absDiff(a, b uint32) uint32 {
	if a > b {
		return a - b
	}
	return b - a
}
