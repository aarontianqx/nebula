// Package repository provides data access implementations.
package repository

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"
)

// MongoDB holds the MongoDB client and provides access to collections.
type MongoDB struct {
	client   *mongo.Client
	database *mongo.Database
	logger   *slog.Logger
}

// MongoDBConfig contains configuration for MongoDB connection.
type MongoDBConfig struct {
	URI            string
	Database       string
	ConnectTimeout time.Duration
	PingTimeout    time.Duration
}

// DefaultMongoDBConfig returns default configuration.
func DefaultMongoDBConfig() *MongoDBConfig {
	return &MongoDBConfig{
		URI:            "mongodb://localhost:27017",
		Database:       "wardenly",
		ConnectTimeout: 10 * time.Second,
		PingTimeout:    5 * time.Second,
	}
}

// NewMongoDB creates a new MongoDB connection.
func NewMongoDB(ctx context.Context, cfg *MongoDBConfig, logger *slog.Logger) (*MongoDB, error) {
	if cfg == nil {
		cfg = DefaultMongoDBConfig()
	}
	if logger == nil {
		logger = slog.Default()
	}

	connectCtx, cancel := context.WithTimeout(ctx, cfg.ConnectTimeout)
	defer cancel()

	clientOptions := options.Client().ApplyURI(cfg.URI)
	client, err := mongo.Connect(connectCtx, clientOptions)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to MongoDB: %w", err)
	}

	// Ping to verify connection
	pingCtx, pingCancel := context.WithTimeout(ctx, cfg.PingTimeout)
	defer pingCancel()

	if err := client.Ping(pingCtx, nil); err != nil {
		// Disconnect on ping failure
		_ = client.Disconnect(ctx)
		return nil, fmt.Errorf("failed to ping MongoDB: %w", err)
	}

	logger.Info("Connected to MongoDB", "uri", cfg.URI, "database", cfg.Database)

	return &MongoDB{
		client:   client,
		database: client.Database(cfg.Database),
		logger:   logger,
	}, nil
}

// Close disconnects from MongoDB.
func (m *MongoDB) Close(ctx context.Context) error {
	if m.client == nil {
		return nil
	}
	return m.client.Disconnect(ctx)
}

// Collection returns a collection by name.
func (m *MongoDB) Collection(name string) *mongo.Collection {
	return m.database.Collection(name)
}

// Client returns the underlying MongoDB client.
func (m *MongoDB) Client() *mongo.Client {
	return m.client
}

// Database returns the underlying MongoDB database.
func (m *MongoDB) Database() *mongo.Database {
	return m.database
}
