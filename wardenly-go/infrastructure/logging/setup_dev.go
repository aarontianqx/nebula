//go:build !prod

package logging

import (
	"log/slog"
	"os"
)

// Setup initializes logging for development mode.
// Logs are written to os.Stdout only; no file output.
// Returns the configured logger, a no-op close function, and any error.
func Setup(cfg *Config) (*slog.Logger, func() error, error) {
	if cfg == nil {
		cfg = DefaultConfig()
	}

	handler := slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level:     cfg.Level,
		AddSource: cfg.AddSource,
	})

	logger := slog.New(handler)
	setGlobal(logger)

	// No resources to close in dev mode
	closeFn := func() error { return nil }

	return logger, closeFn, nil
}
