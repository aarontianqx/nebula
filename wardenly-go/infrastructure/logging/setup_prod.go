//go:build prod

package logging

import (
	"log/slog"
	"os"
	"path/filepath"

	"gopkg.in/natefinch/lumberjack.v2"
)

// Setup initializes logging for production mode.
// Logs are written to rotating files via lumberjack; no console output.
// Returns the configured logger, a close function to flush/close the log file, and any error.
func Setup(cfg *Config) (*slog.Logger, func() error, error) {
	if cfg == nil {
		cfg = DefaultConfig()
	}

	dir := cfg.Dir
	if dir == "" {
		dir = DefaultLogDir()
	}

	// Ensure log directory exists
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, nil, err
	}

	logPath := filepath.Join(dir, "wardenly.log")

	lj := &lumberjack.Logger{
		Filename:   logPath,
		MaxSize:    cfg.MaxSizeMB,
		MaxBackups: cfg.MaxBackups,
		MaxAge:     cfg.MaxAgeDays,
		Compress:   cfg.Compress,
		LocalTime:  true,
	}

	handler := slog.NewTextHandler(lj, &slog.HandlerOptions{
		Level:     cfg.Level,
		AddSource: cfg.AddSource,
	})

	logger := slog.New(handler)
	setGlobal(logger)

	closeFn := func() error {
		return lj.Close()
	}

	return logger, closeFn, nil
}
