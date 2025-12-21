package script

import (
	"fmt"
	"io/fs"
	"path/filepath"
	"time"

	"gopkg.in/yaml.v3"
)

// yamlScript is the YAML structure for script definitions.
type yamlScript struct {
	Name        string     `yaml:"name"`
	Description string     `yaml:"description"`
	Version     string     `yaml:"version"`
	Author      string     `yaml:"author"`
	Steps       []yamlStep `yaml:"steps"`
}

type yamlStep struct {
	Scene             string       `yaml:"scene"`
	Timeout           duration     `yaml:"timeout"`
	Actions           []yamlAction `yaml:"actions"`
	ContinueOnFailure bool         `yaml:"continueOnFailure"`
	Loop              *yamlLoop    `yaml:"loop,omitempty"`
	OCRRule           *yamlOCRRule `yaml:"ocrRule,omitempty"`
}

type yamlAction struct {
	Type       string         `yaml:"type"`
	Points     []yamlPoint    `yaml:"points,omitempty"`
	Duration   duration       `yaml:"duration,omitempty"`
	RetryCount int            `yaml:"retryCount,omitempty"`
	Key        string         `yaml:"key,omitempty"`
	Condition  *yamlCondition `yaml:"condition,omitempty"`
}

type yamlPoint struct {
	X float64 `yaml:"x"`
	Y float64 `yaml:"y"`
}

type yamlCondition struct {
	Op    string `yaml:"op"`
	Key   string `yaml:"key"`
	Value int    `yaml:"value"`
}

type yamlLoop struct {
	StartIndex int      `yaml:"startIndex"`
	EndIndex   int      `yaml:"endIndex"`
	Count      int      `yaml:"count,omitempty"`
	Until      string   `yaml:"until,omitempty"`
	Interval   duration `yaml:"interval,omitempty"`
}

type yamlOCRRule struct {
	Name      string  `yaml:"name"`
	ROI       yamlROI `yaml:"roi"`
	Threshold int     `yaml:"threshold"`
}

type yamlROI struct {
	X      int `yaml:"x"`
	Y      int `yaml:"y"`
	Width  int `yaml:"width"`
	Height int `yaml:"height"`
}

// duration is a wrapper for time.Duration that handles YAML parsing.
type duration time.Duration

func (d *duration) UnmarshalYAML(value *yaml.Node) error {
	var s string
	if err := value.Decode(&s); err != nil {
		return err
	}
	parsed, err := time.ParseDuration(s)
	if err != nil {
		return err
	}
	*d = duration(parsed)
	return nil
}

// Loader handles loading script definitions from various sources.
type Loader struct {
	registry *Registry
}

// NewLoader creates a new script loader that populates the given registry.
func NewLoader(registry *Registry) *Loader {
	return &Loader{registry: registry}
}

// LoadFromFS loads script definitions from an embedded or real filesystem.
// It expects YAML files in a "scripts" subdirectory.
func (l *Loader) LoadFromFS(fsys fs.FS) error {
	entries, err := fs.ReadDir(fsys, "scripts")
	if err != nil {
		return fmt.Errorf("failed to read scripts directory: %w", err)
	}

	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".yaml" {
			continue
		}

		if err := l.loadFile(fsys, "scripts/"+entry.Name()); err != nil {
			return err
		}
	}

	return nil
}

// loadFile loads a single script definition file.
func (l *Loader) loadFile(fsys fs.FS, path string) error {
	data, err := fs.ReadFile(fsys, path)
	if err != nil {
		return fmt.Errorf("failed to read script file %s: %w", path, err)
	}

	var ys yamlScript
	if err := yaml.Unmarshal(data, &ys); err != nil {
		return fmt.Errorf("failed to parse script file %s: %w", path, err)
	}

	script := convertYAMLScript(&ys)
	l.registry.Register(script)

	return nil
}

// convertYAMLScript converts a YAML script to a domain Script.
func convertYAMLScript(ys *yamlScript) *Script {
	script := &Script{
		Name:        ys.Name,
		Description: ys.Description,
		Version:     ys.Version,
		Author:      ys.Author,
		Steps:       make([]Step, len(ys.Steps)),
	}

	for i, ystep := range ys.Steps {
		script.Steps[i] = convertYAMLStep(&ystep)
	}

	return script
}

func convertYAMLStep(ys *yamlStep) Step {
	step := Step{
		ExpectedScene:     ys.Scene,
		Timeout:           time.Duration(ys.Timeout),
		ContinueOnFailure: ys.ContinueOnFailure,
		Actions:           make([]Action, len(ys.Actions)),
	}

	for i, ya := range ys.Actions {
		step.Actions[i] = convertYAMLAction(&ya)
	}

	if ys.Loop != nil {
		step.Loop = &Loop{
			StartIndex: ys.Loop.StartIndex, // YAML indices are 0-based (despite old comments saying 1-based)
			EndIndex:   ys.Loop.EndIndex,
			Count:      ys.Loop.Count,
			Until:      ys.Loop.Until,
			Interval:   time.Duration(ys.Loop.Interval),
		}
	}

	if ys.OCRRule != nil {
		step.OCRRule = &OCRRule{
			Name:      ys.OCRRule.Name,
			Threshold: ys.OCRRule.Threshold,
			ROI: ROI{
				X:      ys.OCRRule.ROI.X,
				Y:      ys.OCRRule.ROI.Y,
				Width:  ys.OCRRule.ROI.Width,
				Height: ys.OCRRule.ROI.Height,
			},
		}
	}

	return step
}

func convertYAMLAction(ya *yamlAction) Action {
	action := Action{
		Type:       ActionType(ya.Type),
		Duration:   time.Duration(ya.Duration),
		RetryCount: ya.RetryCount,
		Key:        ya.Key,
		Points:     make([]Point, len(ya.Points)),
	}

	for i, yp := range ya.Points {
		action.Points[i] = Point{X: yp.X, Y: yp.Y}
	}

	if ya.Condition != nil {
		action.Condition = &Condition{
			Op:    ya.Condition.Op,
			Key:   ya.Condition.Key,
			Value: ya.Condition.Value,
		}
	}

	return action
}
