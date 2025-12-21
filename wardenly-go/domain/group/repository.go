package group

import "context"

// Repository defines the interface for group persistence operations.
// This interface follows the Repository pattern to abstract data access.
type Repository interface {
	// FindByID retrieves a group by its unique identifier.
	// Returns nil if not found.
	FindByID(ctx context.Context, id string) (*Group, error)

	// FindByName retrieves a group by its name.
	// Returns nil if not found.
	FindByName(ctx context.Context, name string) (*Group, error)

	// FindAll retrieves all groups.
	FindAll(ctx context.Context) ([]*Group, error)

	// FindByAccountID retrieves all groups containing a specific account.
	FindByAccountID(ctx context.Context, accountID string) ([]*Group, error)

	// Insert creates a new group.
	Insert(ctx context.Context, group *Group) error

	// Update updates an existing group.
	Update(ctx context.Context, group *Group) error

	// Delete removes a group by its identifier.
	Delete(ctx context.Context, id string) error
}
