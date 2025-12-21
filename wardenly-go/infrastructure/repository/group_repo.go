package repository

import (
	"context"
	"fmt"
	"log/slog"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/bson/primitive"
	"go.mongodb.org/mongo-driver/mongo"

	"wardenly-go/domain/group"
)

// groupDocument is the MongoDB document structure for groups.
type groupDocument struct {
	ID          primitive.ObjectID `bson:"_id,omitempty"`
	Name        string             `bson:"name"`
	Description string             `bson:"description,omitempty"`
	AccountIDs  []string           `bson:"account_ids"`
	Ranking     int                `bson:"ranking"`
}

// MongoGroupRepository implements group.Repository using MongoDB.
type MongoGroupRepository struct {
	collection *mongo.Collection
	logger     *slog.Logger
}

// NewMongoGroupRepository creates a new MongoDB-based group repository.
func NewMongoGroupRepository(db *MongoDB, logger *slog.Logger) *MongoGroupRepository {
	if logger == nil {
		logger = slog.Default()
	}
	return &MongoGroupRepository{
		collection: db.Collection("group"),
		logger:     logger,
	}
}

// FindByID retrieves a group by its unique identifier.
func (r *MongoGroupRepository) FindByID(ctx context.Context, id string) (*group.Group, error) {
	objectID, err := primitive.ObjectIDFromHex(id)
	if err != nil {
		return nil, fmt.Errorf("invalid ID format: %w", err)
	}

	filter := bson.M{"_id": objectID}
	var doc groupDocument
	if err := r.collection.FindOne(ctx, filter).Decode(&doc); err != nil {
		if err == mongo.ErrNoDocuments {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to find group: %w", err)
	}

	return documentToGroup(&doc), nil
}

// FindByName retrieves a group by its name.
func (r *MongoGroupRepository) FindByName(ctx context.Context, name string) (*group.Group, error) {
	filter := bson.M{"name": name}
	var doc groupDocument
	if err := r.collection.FindOne(ctx, filter).Decode(&doc); err != nil {
		if err == mongo.ErrNoDocuments {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to find group by name: %w", err)
	}

	return documentToGroup(&doc), nil
}

// FindAll retrieves all groups.
func (r *MongoGroupRepository) FindAll(ctx context.Context) ([]*group.Group, error) {
	cursor, err := r.collection.Find(ctx, bson.D{})
	if err != nil {
		return nil, fmt.Errorf("failed to find groups: %w", err)
	}
	defer cursor.Close(ctx)

	var docs []groupDocument
	if err := cursor.All(ctx, &docs); err != nil {
		return nil, fmt.Errorf("failed to decode groups: %w", err)
	}

	groups := make([]*group.Group, len(docs))
	for i, doc := range docs {
		groups[i] = documentToGroup(&doc)
	}

	return groups, nil
}

// FindByAccountID retrieves all groups containing a specific account.
func (r *MongoGroupRepository) FindByAccountID(ctx context.Context, accountID string) ([]*group.Group, error) {
	filter := bson.M{"account_ids": accountID}
	cursor, err := r.collection.Find(ctx, filter)
	if err != nil {
		return nil, fmt.Errorf("failed to find groups by account: %w", err)
	}
	defer cursor.Close(ctx)

	var docs []groupDocument
	if err := cursor.All(ctx, &docs); err != nil {
		return nil, fmt.Errorf("failed to decode groups: %w", err)
	}

	groups := make([]*group.Group, len(docs))
	for i, doc := range docs {
		groups[i] = documentToGroup(&doc)
	}

	return groups, nil
}

// Insert creates a new group.
func (r *MongoGroupRepository) Insert(ctx context.Context, grp *group.Group) error {
	doc := groupToDocument(grp)
	result, err := r.collection.InsertOne(ctx, doc)
	if err != nil {
		return fmt.Errorf("failed to insert group: %w", err)
	}

	// Update the group ID with the generated ObjectID
	if oid, ok := result.InsertedID.(primitive.ObjectID); ok {
		grp.ID = oid.Hex()
	}

	r.logger.Info("Group inserted", "id", grp.ID, "name", grp.Name)
	return nil
}

// Update updates an existing group.
func (r *MongoGroupRepository) Update(ctx context.Context, grp *group.Group) error {
	objectID, err := primitive.ObjectIDFromHex(grp.ID)
	if err != nil {
		return fmt.Errorf("invalid ID format: %w", err)
	}

	doc := groupToDocument(grp)
	doc.ID = objectID

	filter := bson.M{"_id": objectID}
	update := bson.M{"$set": doc}

	result, err := r.collection.UpdateOne(ctx, filter, update)
	if err != nil {
		return fmt.Errorf("failed to update group: %w", err)
	}

	if result.MatchedCount == 0 {
		return group.ErrGroupNotFound
	}

	r.logger.Info("Group updated", "id", grp.ID, "name", grp.Name)
	return nil
}

// Delete removes a group by its identifier.
func (r *MongoGroupRepository) Delete(ctx context.Context, id string) error {
	objectID, err := primitive.ObjectIDFromHex(id)
	if err != nil {
		return fmt.Errorf("invalid ID format: %w", err)
	}

	filter := bson.M{"_id": objectID}
	result, err := r.collection.DeleteOne(ctx, filter)
	if err != nil {
		return fmt.Errorf("failed to delete group: %w", err)
	}

	if result.DeletedCount == 0 {
		return group.ErrGroupNotFound
	}

	r.logger.Info("Group deleted", "id", id)
	return nil
}

// documentToGroup converts a MongoDB document to a domain Group.
func documentToGroup(doc *groupDocument) *group.Group {
	accountIDs := doc.AccountIDs
	if accountIDs == nil {
		accountIDs = []string{}
	}
	return &group.Group{
		ID:          doc.ID.Hex(),
		Name:        doc.Name,
		Description: doc.Description,
		AccountIDs:  accountIDs,
		Ranking:     doc.Ranking,
	}
}

// groupToDocument converts a domain Group to a MongoDB document.
func groupToDocument(grp *group.Group) *groupDocument {
	accountIDs := grp.AccountIDs
	if accountIDs == nil {
		accountIDs = []string{}
	}

	doc := &groupDocument{
		Name:        grp.Name,
		Description: grp.Description,
		AccountIDs:  accountIDs,
		Ranking:     grp.Ranking,
	}

	if grp.ID != "" {
		if oid, err := primitive.ObjectIDFromHex(grp.ID); err == nil {
			doc.ID = oid
		}
	}

	return doc
}

// Ensure MongoGroupRepository implements group.Repository
var _ group.Repository = (*MongoGroupRepository)(nil)
