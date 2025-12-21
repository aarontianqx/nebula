package scene

import (
	"image"
	"sync"
)

// Registry manages scene definitions and provides lookup functionality.
type Registry struct {
	scenes map[string]*Scene
	mu     sync.RWMutex
}

// NewRegistry creates a new empty scene registry.
func NewRegistry() *Registry {
	return &Registry{
		scenes: make(map[string]*Scene),
	}
}

// Register adds a scene to the registry.
// If a scene with the same name exists, it will be replaced.
func (r *Registry) Register(scene *Scene) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.scenes[scene.Name] = scene
}

// RegisterAll adds multiple scenes to the registry.
func (r *Registry) RegisterAll(scenes []*Scene) {
	r.mu.Lock()
	defer r.mu.Unlock()
	for _, scene := range scenes {
		r.scenes[scene.Name] = scene
	}
}

// Get retrieves a scene by name.
// Returns nil if not found.
func (r *Registry) Get(name string) *Scene {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.scenes[name]
}

// GetByCategory returns all scenes in a specific category.
func (r *Registry) GetByCategory(category string) []*Scene {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var result []*Scene
	for _, scene := range r.scenes {
		if scene.Category == category {
			result = append(result, scene)
		}
	}
	return result
}

// List returns all registered scene names.
func (r *Registry) List() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	names := make([]string, 0, len(r.scenes))
	for name := range r.scenes {
		names = append(names, name)
	}
	return names
}

// All returns all registered scenes.
func (r *Registry) All() []*Scene {
	r.mu.RLock()
	defer r.mu.RUnlock()

	scenes := make([]*Scene, 0, len(r.scenes))
	for _, scene := range r.scenes {
		scenes = append(scenes, scene)
	}
	return scenes
}

// Count returns the number of registered scenes.
func (r *Registry) Count() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.scenes)
}

// Clear removes all scenes from the registry.
func (r *Registry) Clear() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.scenes = make(map[string]*Scene)
}

// FindMatch searches for a matching scene in the given image.
// If names are provided, only those scenes are checked.
// Returns nil if no match is found.
func (r *Registry) FindMatch(img image.Image, matcher *Matcher, names ...string) *Scene {
	if img == nil || matcher == nil {
		return nil
	}

	r.mu.RLock()
	defer r.mu.RUnlock()

	if len(names) == 0 {
		// Check all scenes
		for _, scene := range r.scenes {
			if matcher.Match(scene, img) {
				return scene
			}
		}
	} else {
		// Check only specified scenes
		for _, name := range names {
			if scene, ok := r.scenes[name]; ok {
				if matcher.Match(scene, img) {
					return scene
				}
			}
		}
	}

	return nil
}

// FindAllMatches returns all scenes that match the given image.
func (r *Registry) FindAllMatches(img image.Image, matcher *Matcher) []*Scene {
	if img == nil || matcher == nil {
		return nil
	}

	r.mu.RLock()
	defer r.mu.RUnlock()

	var matches []*Scene
	for _, scene := range r.scenes {
		if matcher.Match(scene, img) {
			matches = append(matches, scene)
		}
	}
	return matches
}
