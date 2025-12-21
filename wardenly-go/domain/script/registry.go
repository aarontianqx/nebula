package script

import (
	"sort"
	"sync"
)

// Registry manages script definitions and provides lookup functionality.
type Registry struct {
	scripts map[string]*Script
	mu      sync.RWMutex
}

// NewRegistry creates a new empty script registry.
func NewRegistry() *Registry {
	return &Registry{
		scripts: make(map[string]*Script),
	}
}

// Register adds a script to the registry.
// If a script with the same name exists, it will be replaced.
func (r *Registry) Register(script *Script) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.scripts[script.Name] = script
}

// RegisterAll adds multiple scripts to the registry.
func (r *Registry) RegisterAll(scripts []*Script) {
	r.mu.Lock()
	defer r.mu.Unlock()
	for _, script := range scripts {
		r.scripts[script.Name] = script
	}
}

// Get retrieves a script by name.
// Returns nil if not found.
func (r *Registry) Get(name string) *Script {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.scripts[name]
}

// List returns all registered script names, sorted alphabetically.
func (r *Registry) List() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	names := make([]string, 0, len(r.scripts))
	for name := range r.scripts {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// All returns all registered scripts.
func (r *Registry) All() []*Script {
	r.mu.RLock()
	defer r.mu.RUnlock()

	scripts := make([]*Script, 0, len(r.scripts))
	for _, script := range r.scripts {
		scripts = append(scripts, script)
	}
	return scripts
}

// Count returns the number of registered scripts.
func (r *Registry) Count() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.scripts)
}

// Clear removes all scripts from the registry.
func (r *Registry) Clear() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.scripts = make(map[string]*Script)
}

// Exists checks if a script with the given name exists.
func (r *Registry) Exists(name string) bool {
	r.mu.RLock()
	defer r.mu.RUnlock()
	_, ok := r.scripts[name]
	return ok
}
