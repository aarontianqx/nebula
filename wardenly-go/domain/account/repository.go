package account

import "context"

// Repository defines the interface for account persistence operations.
// This interface follows the Repository pattern to abstract data access.
type Repository interface {
	// FindByID retrieves an account by its unique identifier.
	// Returns nil if not found.
	FindByID(ctx context.Context, id string) (*Account, error)

	// FindAll retrieves all accounts.
	FindAll(ctx context.Context) ([]*Account, error)

	// Insert creates a new account.
	Insert(ctx context.Context, account *Account) error

	// Update updates an existing account.
	Update(ctx context.Context, account *Account) error

	// UpdateCookies updates only the cookies for an account.
	// This is a specialized method for frequent cookie updates.
	UpdateCookies(ctx context.Context, id string, cookies []Cookie) error

	// Delete removes an account by its identifier.
	Delete(ctx context.Context, id string) error
}
