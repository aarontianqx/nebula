package scene

import (
	"fmt"
	"image/color"
	"io/fs"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

// yamlSceneDefinition is the YAML structure for scene definitions.
type yamlSceneDefinition struct {
	Category string      `yaml:"category"`
	Scenes   []yamlScene `yaml:"scenes"`
}

type yamlScene struct {
	Name    string                `yaml:"name"`
	Points  []yamlPoint           `yaml:"points"`
	Actions map[string]yamlAction `yaml:"actions"`
}

type yamlPoint struct {
	X     int        `yaml:"x"`
	Y     int        `yaml:"y"`
	Color color.RGBA `yaml:"color"`
}

type yamlAction struct {
	Type  string          `yaml:"type"`
	Point yamlActionPoint `yaml:"point"`
}

type yamlActionPoint struct {
	X float64 `yaml:"x"`
	Y float64 `yaml:"y"`
}

// Loader handles loading scene definitions from various sources.
type Loader struct {
	registry *Registry
}

// NewLoader creates a new scene loader that populates the given registry.
func NewLoader(registry *Registry) *Loader {
	return &Loader{registry: registry}
}

// LoadFromFS loads scene definitions from an embedded or real filesystem.
// It expects YAML files in a "scenes" subdirectory.
func (l *Loader) LoadFromFS(fsys fs.FS) error {
	entries, err := fs.ReadDir(fsys, "scenes")
	if err != nil {
		return fmt.Errorf("failed to read scenes directory: %w", err)
	}

	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".yaml" {
			continue
		}

		if err := l.loadFile(fsys, "scenes/"+entry.Name()); err != nil {
			return err
		}
	}

	return nil
}

// loadFile loads a single scene definition file.
func (l *Loader) loadFile(fsys fs.FS, path string) error {
	data, err := fs.ReadFile(fsys, path)
	if err != nil {
		return fmt.Errorf("failed to read scene file %s: %w", path, err)
	}

	var def yamlSceneDefinition
	if err := yaml.Unmarshal(data, &def); err != nil {
		return fmt.Errorf("failed to parse scene file %s: %w", path, err)
	}

	for _, ys := range def.Scenes {
		scene := convertYAMLScene(&ys, def.Category)
		l.registry.Register(scene)
	}

	return nil
}

// convertYAMLScene converts a YAML scene to a domain Scene.
func convertYAMLScene(ys *yamlScene, category string) *Scene {
	scene := &Scene{
		Name:     ys.Name,
		Category: category,
		Points:   make([]Point, len(ys.Points)),
		Actions:  make(map[string]Action),
	}

	for i, yp := range ys.Points {
		scene.Points[i] = Point{
			X:     yp.X,
			Y:     yp.Y,
			Color: yp.Color,
		}
	}

	for name, ya := range ys.Actions {
		scene.Actions[name] = Action{
			Type: ActionType(ya.Type),
			Point: ActionPoint{
				X: ya.Point.X,
				Y: ya.Point.Y,
			},
		}
	}

	return scene
}
