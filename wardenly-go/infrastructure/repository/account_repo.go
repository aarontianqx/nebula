package repository

import (
	"context"
	"fmt"
	"log/slog"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/bson/primitive"
	"go.mongodb.org/mongo-driver/mongo"

	"wardenly-go/domain/account"
)

// accountDocument is the MongoDB document structure for accounts.
type accountDocument struct {
	ID       primitive.ObjectID `bson:"_id,omitempty"`
	RoleName string             `bson:"role_name"`
	UserName string             `bson:"user_name"`
	Password string             `bson:"password"`
	Ranking  int                `bson:"ranking"`
	ServerID int                `bson:"server_id"`
	Cookies  []cookieDocument   `bson:"cookies,omitempty"`
}

// cookieDocument is the MongoDB document structure for cookies.
type cookieDocument struct {
	Name         string `bson:"name"`
	Value        string `bson:"value"`
	Domain       string `bson:"domain"`
	Path         string `bson:"path"`
	HTTPOnly     bool   `bson:"http_only"`
	Secure       bool   `bson:"secure"`
	SourcePort   int    `bson:"source_port"`
	SourceScheme string `bson:"source_scheme,omitempty"`
	Priority     string `bson:"priority,omitempty"`
}

// MongoAccountRepository implements account.Repository using MongoDB.
type MongoAccountRepository struct {
	collection *mongo.Collection
	logger     *slog.Logger
}

// NewMongoAccountRepository creates a new MongoDB-based account repository.
func NewMongoAccountRepository(db *MongoDB, logger *slog.Logger) *MongoAccountRepository {
	if logger == nil {
		logger = slog.Default()
	}
	return &MongoAccountRepository{
		collection: db.Collection("account"),
		logger:     logger,
	}
}

// FindByID retrieves an account by its unique identifier.
func (r *MongoAccountRepository) FindByID(ctx context.Context, id string) (*account.Account, error) {
	objectID, err := primitive.ObjectIDFromHex(id)
	if err != nil {
		return nil, fmt.Errorf("invalid ID format: %w", err)
	}

	filter := bson.M{"_id": objectID}
	var doc accountDocument
	if err := r.collection.FindOne(ctx, filter).Decode(&doc); err != nil {
		if err == mongo.ErrNoDocuments {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to find account: %w", err)
	}

	return documentToAccount(&doc), nil
}

// FindAll retrieves all accounts.
func (r *MongoAccountRepository) FindAll(ctx context.Context) ([]*account.Account, error) {
	cursor, err := r.collection.Find(ctx, bson.D{})
	if err != nil {
		return nil, fmt.Errorf("failed to find accounts: %w", err)
	}
	defer cursor.Close(ctx)

	var docs []accountDocument
	if err := cursor.All(ctx, &docs); err != nil {
		return nil, fmt.Errorf("failed to decode accounts: %w", err)
	}

	accounts := make([]*account.Account, len(docs))
	for i, doc := range docs {
		accounts[i] = documentToAccount(&doc)
	}

	return accounts, nil
}

// Insert creates a new account.
func (r *MongoAccountRepository) Insert(ctx context.Context, acc *account.Account) error {
	doc := accountToDocument(acc)
	result, err := r.collection.InsertOne(ctx, doc)
	if err != nil {
		return fmt.Errorf("failed to insert account: %w", err)
	}

	// Update the account ID with the generated ObjectID
	if oid, ok := result.InsertedID.(primitive.ObjectID); ok {
		acc.ID = oid.Hex()
	}

	r.logger.Info("Account inserted", "id", acc.ID, "role_name", acc.RoleName)
	return nil
}

// Update updates an existing account.
func (r *MongoAccountRepository) Update(ctx context.Context, acc *account.Account) error {
	objectID, err := primitive.ObjectIDFromHex(acc.ID)
	if err != nil {
		return fmt.Errorf("invalid ID format: %w", err)
	}

	doc := accountToDocument(acc)
	doc.ID = objectID

	filter := bson.M{"_id": objectID}
	update := bson.M{"$set": doc}

	result, err := r.collection.UpdateOne(ctx, filter, update)
	if err != nil {
		return fmt.Errorf("failed to update account: %w", err)
	}

	if result.MatchedCount == 0 {
		return account.ErrAccountNotFound
	}

	return nil
}

// UpdateCookies updates only the cookies for an account.
func (r *MongoAccountRepository) UpdateCookies(ctx context.Context, id string, cookies []account.Cookie) error {
	objectID, err := primitive.ObjectIDFromHex(id)
	if err != nil {
		return fmt.Errorf("invalid ID format: %w", err)
	}

	cookieDocs := make([]cookieDocument, len(cookies))
	for i, c := range cookies {
		cookieDocs[i] = cookieDocument{
			Name:         c.Name,
			Value:        c.Value,
			Domain:       c.Domain,
			Path:         c.Path,
			HTTPOnly:     c.HTTPOnly,
			Secure:       c.Secure,
			SourcePort:   c.SourcePort,
			SourceScheme: c.SourceScheme,
			Priority:     c.Priority,
		}
	}

	filter := bson.M{"_id": objectID}
	update := bson.M{"$set": bson.M{"cookies": cookieDocs}}

	result, err := r.collection.UpdateOne(ctx, filter, update)
	if err != nil {
		return fmt.Errorf("failed to update cookies: %w", err)
	}

	if result.MatchedCount == 0 {
		return account.ErrAccountNotFound
	}

	r.logger.Info("Cookies updated", "id", id, "count", len(cookies))
	return nil
}

// Delete removes an account by its identifier.
func (r *MongoAccountRepository) Delete(ctx context.Context, id string) error {
	objectID, err := primitive.ObjectIDFromHex(id)
	if err != nil {
		return fmt.Errorf("invalid ID format: %w", err)
	}

	filter := bson.M{"_id": objectID}
	result, err := r.collection.DeleteOne(ctx, filter)
	if err != nil {
		return fmt.Errorf("failed to delete account: %w", err)
	}

	if result.DeletedCount == 0 {
		return account.ErrAccountNotFound
	}

	r.logger.Info("Account deleted", "id", id)
	return nil
}

// documentToAccount converts a MongoDB document to a domain Account.
func documentToAccount(doc *accountDocument) *account.Account {
	acc := &account.Account{
		ID:       doc.ID.Hex(),
		RoleName: doc.RoleName,
		UserName: doc.UserName,
		Password: doc.Password,
		Ranking:  doc.Ranking,
		ServerID: doc.ServerID,
	}

	if len(doc.Cookies) > 0 {
		acc.Cookies = make([]account.Cookie, len(doc.Cookies))
		for i, c := range doc.Cookies {
			acc.Cookies[i] = account.Cookie{
				Name:         c.Name,
				Value:        c.Value,
				Domain:       c.Domain,
				Path:         c.Path,
				HTTPOnly:     c.HTTPOnly,
				Secure:       c.Secure,
				SourcePort:   c.SourcePort,
				SourceScheme: c.SourceScheme,
				Priority:     c.Priority,
			}
		}
	}

	return acc
}

// accountToDocument converts a domain Account to a MongoDB document.
func accountToDocument(acc *account.Account) *accountDocument {
	doc := &accountDocument{
		RoleName: acc.RoleName,
		UserName: acc.UserName,
		Password: acc.Password,
		Ranking:  acc.Ranking,
		ServerID: acc.ServerID,
	}

	if acc.ID != "" {
		if oid, err := primitive.ObjectIDFromHex(acc.ID); err == nil {
			doc.ID = oid
		}
	}

	if len(acc.Cookies) > 0 {
		doc.Cookies = make([]cookieDocument, len(acc.Cookies))
		for i, c := range acc.Cookies {
			doc.Cookies[i] = cookieDocument{
				Name:         c.Name,
				Value:        c.Value,
				Domain:       c.Domain,
				Path:         c.Path,
				HTTPOnly:     c.HTTPOnly,
				Secure:       c.Secure,
				SourcePort:   c.SourcePort,
				SourceScheme: c.SourceScheme,
				Priority:     c.Priority,
			}
		}
	}

	return doc
}

// Ensure MongoAccountRepository implements account.Repository
var _ account.Repository = (*MongoAccountRepository)(nil)
