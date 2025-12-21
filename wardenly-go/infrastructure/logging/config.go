// Package logging provides a unified logging setup with build-tag-based
// prod/dev split: prod writes to rotating log files, dev writes to console only.
package logging

import (
	"context"
	"log/slog"
	"os"
	"path/filepath"
)

// Config holds logging configuration options.
type Config struct {
	// Level is the minimum log level to emit.
	Level slog.Level
	// Dir is the directory for log files (prod only).
	// If empty, defaults to os.UserConfigDir()/wardenly/logs.
	Dir string
	// MaxSizeMB is the maximum size in megabytes of a single log file before rotation.
	MaxSizeMB int
	// MaxBackups is the maximum number of old log files to retain.
	MaxBackups int
	// MaxAgeDays is the maximum number of days to retain old log files.
	MaxAgeDays int
	// Compress determines if rotated log files should be compressed.
	Compress bool
	// AddSource adds source file:line to log entries.
	AddSource bool
}

// DefaultConfig returns sensible defaults for production logging.
func DefaultConfig() *Config {
	return &Config{
		Level:      slog.LevelInfo,
		Dir:        "", // will be resolved in Setup
		MaxSizeMB:  50,
		MaxBackups: 10,
		MaxAgeDays: 14,
		Compress:   true,
		AddSource:  false,
	}
}

// DefaultLogDir returns the default log directory path.
// Tries os.UserConfigDir, falls back to os.UserCacheDir, then os.TempDir.
func DefaultLogDir() string {
	dir, err := os.UserConfigDir()
	if err != nil {
		dir, err = os.UserCacheDir()
		if err != nil {
			dir = os.TempDir()
		}
	}
	return filepath.Join(dir, "wardenly", "logs")
}

// --- Global logger access ---

var globalLogger *slog.Logger

// L returns the global logger. If Setup has not been called, returns slog.Default().
func L() *slog.Logger {
	if globalLogger != nil {
		return globalLogger
	}
	return slog.Default()
}

// setGlobal sets the package-level logger and also slog.SetDefault.
func setGlobal(logger *slog.Logger) {
	globalLogger = logger
	slog.SetDefault(logger)
}

// --- Context-based logging ---

type ctxKey struct{}

// With returns a new context that carries the given logger.
func With(ctx context.Context, logger *slog.Logger) context.Context {
	return context.WithValue(ctx, ctxKey{}, logger)
}

// From extracts the logger from context. If none is present, returns L().
func From(ctx context.Context) *slog.Logger {
	if ctx == nil {
		return L()
	}
	if logger, ok := ctx.Value(ctxKey{}).(*slog.Logger); ok && logger != nil {
		return logger
	}
	return L()
}

// WithAttrs returns a new context carrying a logger enriched with the given attributes.
// This is a convenience for logging.With(ctx, logging.From(ctx).With(attrs...)).
func WithAttrs(ctx context.Context, args ...any) context.Context {
	return With(ctx, From(ctx).With(args...))
}
