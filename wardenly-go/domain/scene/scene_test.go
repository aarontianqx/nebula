package scene

import (
	"image"
	"image/color"
	"testing"
)

// mockImage creates a simple image for testing.
type mockImage struct {
	colors map[image.Point]color.Color
}

func newMockImage() *mockImage {
	return &mockImage{colors: make(map[image.Point]color.Color)}
}

func (m *mockImage) SetColor(x, y int, c color.Color) {
	m.colors[image.Point{X: x, Y: y}] = c
}

func (m *mockImage) ColorModel() color.Model { return color.RGBAModel }
func (m *mockImage) Bounds() image.Rectangle { return image.Rect(0, 0, 1000, 1000) }
func (m *mockImage) At(x, y int) color.Color {
	if c, ok := m.colors[image.Point{X: x, Y: y}]; ok {
		return c
	}
	return color.RGBA{0, 0, 0, 255}
}

func TestMatcher_Match(t *testing.T) {
	matcher := NewMatcher(5.0)

	scene := &Scene{
		Name: "test_scene",
		Points: []Point{
			{X: 100, Y: 100, Color: color.RGBA{255, 0, 0, 255}},
			{X: 200, Y: 200, Color: color.RGBA{0, 255, 0, 255}},
		},
	}

	tests := []struct {
		name     string
		setup    func(*mockImage)
		expected bool
	}{
		{
			name: "exact match",
			setup: func(img *mockImage) {
				img.SetColor(100, 100, color.RGBA{255, 0, 0, 255})
				img.SetColor(200, 200, color.RGBA{0, 255, 0, 255})
			},
			expected: true,
		},
		{
			name: "close match within threshold",
			setup: func(img *mockImage) {
				img.SetColor(100, 100, color.RGBA{252, 3, 3, 255})
				img.SetColor(200, 200, color.RGBA{3, 252, 3, 255})
			},
			expected: true,
		},
		{
			name: "no match - too different",
			setup: func(img *mockImage) {
				img.SetColor(100, 100, color.RGBA{0, 0, 255, 255})
				img.SetColor(200, 200, color.RGBA{255, 255, 0, 255})
			},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			img := newMockImage()
			tt.setup(img)

			if got := matcher.Match(scene, img); got != tt.expected {
				t.Errorf("Match() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestMatcher_Match_EdgeCases(t *testing.T) {
	matcher := NewMatcher(5.0)

	t.Run("nil image", func(t *testing.T) {
		scene := &Scene{Name: "test", Points: []Point{{X: 0, Y: 0, Color: color.RGBA{}}}}
		if matcher.Match(scene, nil) {
			t.Error("Expected false for nil image")
		}
	})

	t.Run("empty points", func(t *testing.T) {
		scene := &Scene{Name: "test", Points: []Point{}}
		img := newMockImage()
		if matcher.Match(scene, img) {
			t.Error("Expected false for empty points")
		}
	})
}

func TestMatcher_MatchWithDetails(t *testing.T) {
	matcher := NewMatcher(5.0)

	scene := &Scene{
		Name: "test_scene",
		Points: []Point{
			{X: 100, Y: 100, Color: color.RGBA{255, 0, 0, 255}},
		},
	}

	img := newMockImage()
	img.SetColor(100, 100, color.RGBA{255, 0, 0, 255})

	result := matcher.MatchWithDetails(scene, img)

	if result.Scene != scene {
		t.Error("Scene not set in result")
	}
	if !result.Matched {
		t.Error("Expected match")
	}
	if result.AvgDiff != 0 {
		t.Errorf("Expected 0 avg diff for exact match, got %v", result.AvgDiff)
	}
	if len(result.PointDiffs) != 1 {
		t.Errorf("Expected 1 point diff, got %d", len(result.PointDiffs))
	}
}

func TestNewMatcher_DefaultThreshold(t *testing.T) {
	matcher := NewMatcher(0)
	if matcher.Threshold != 5.0 {
		t.Errorf("Expected default threshold 5.0, got %v", matcher.Threshold)
	}

	matcher = NewMatcher(-1)
	if matcher.Threshold != 5.0 {
		t.Errorf("Expected default threshold 5.0 for negative input, got %v", matcher.Threshold)
	}
}

func TestRegistry_Basic(t *testing.T) {
	registry := NewRegistry()

	scene1 := &Scene{Name: "scene1", Category: "battle"}
	scene2 := &Scene{Name: "scene2", Category: "city"}
	scene3 := &Scene{Name: "scene3", Category: "battle"}

	registry.Register(scene1)
	registry.Register(scene2)
	registry.Register(scene3)

	t.Run("Get", func(t *testing.T) {
		if got := registry.Get("scene1"); got != scene1 {
			t.Error("Failed to get scene1")
		}
		if got := registry.Get("nonexistent"); got != nil {
			t.Error("Expected nil for nonexistent scene")
		}
	})

	t.Run("Count", func(t *testing.T) {
		if got := registry.Count(); got != 3 {
			t.Errorf("Count() = %d, want 3", got)
		}
	})

	t.Run("List", func(t *testing.T) {
		names := registry.List()
		if len(names) != 3 {
			t.Errorf("List() returned %d names, want 3", len(names))
		}
	})

	t.Run("GetByCategory", func(t *testing.T) {
		battleScenes := registry.GetByCategory("battle")
		if len(battleScenes) != 2 {
			t.Errorf("GetByCategory(battle) returned %d scenes, want 2", len(battleScenes))
		}
	})

	t.Run("Clear", func(t *testing.T) {
		registry.Clear()
		if registry.Count() != 0 {
			t.Error("Clear() did not remove all scenes")
		}
	})
}

func TestRegistry_RegisterAll(t *testing.T) {
	registry := NewRegistry()

	scenes := []*Scene{
		{Name: "scene1"},
		{Name: "scene2"},
		{Name: "scene3"},
	}

	registry.RegisterAll(scenes)

	if registry.Count() != 3 {
		t.Errorf("RegisterAll: Count() = %d, want 3", registry.Count())
	}
}

func TestRegistry_FindMatch(t *testing.T) {
	registry := NewRegistry()
	matcher := NewMatcher(5.0)

	scene1 := &Scene{
		Name:   "scene1",
		Points: []Point{{X: 100, Y: 100, Color: color.RGBA{255, 0, 0, 255}}},
	}
	scene2 := &Scene{
		Name:   "scene2",
		Points: []Point{{X: 200, Y: 200, Color: color.RGBA{0, 255, 0, 255}}},
	}

	registry.Register(scene1)
	registry.Register(scene2)

	img := newMockImage()
	img.SetColor(100, 100, color.RGBA{255, 0, 0, 255})

	t.Run("find in all", func(t *testing.T) {
		found := registry.FindMatch(img, matcher)
		if found != scene1 {
			t.Error("Expected to find scene1")
		}
	})

	t.Run("find in specific", func(t *testing.T) {
		found := registry.FindMatch(img, matcher, "scene1")
		if found != scene1 {
			t.Error("Expected to find scene1")
		}

		found = registry.FindMatch(img, matcher, "scene2")
		if found != nil {
			t.Error("Expected nil when scene2 doesn't match")
		}
	})

	t.Run("nil image", func(t *testing.T) {
		found := registry.FindMatch(nil, matcher)
		if found != nil {
			t.Error("Expected nil for nil image")
		}
	})
}
