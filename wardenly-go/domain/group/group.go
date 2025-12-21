// Package group defines the Group entity and related types.
package group

import "errors"

// Common errors for group operations.
var (
	ErrGroupNotFound = errors.New("group not found")
	ErrEmptyGroup    = errors.New("group has no valid accounts")
)

// Group represents a collection of accounts for batch operations.
type Group struct {
	// ID is the unique identifier (MongoDB ObjectID)
	ID string

	// Name is the display name of the group
	Name string

	// Description is an optional description
	Description string

	// AccountIDs is the list of account IDs in this group
	AccountIDs []string

	// Ranking is for sorting groups in UI (lower = higher priority)
	Ranking int
}

// IsEmpty returns true if the group has no accounts.
func (g *Group) IsEmpty() bool {
	return len(g.AccountIDs) == 0
}

// AccountCount returns the number of accounts in the group.
func (g *Group) AccountCount() int {
	return len(g.AccountIDs)
}

// ContainsAccount checks if the group contains a specific account.
func (g *Group) ContainsAccount(accountID string) bool {
	for _, id := range g.AccountIDs {
		if id == accountID {
			return true
		}
	}
	return false
}

// AddAccount adds an account to the group if not already present.
// Returns true if the account was added.
func (g *Group) AddAccount(accountID string) bool {
	if g.ContainsAccount(accountID) {
		return false
	}
	g.AccountIDs = append(g.AccountIDs, accountID)
	return true
}

// RemoveAccount removes an account from the group.
// Returns true if the account was removed.
func (g *Group) RemoveAccount(accountID string) bool {
	for i, id := range g.AccountIDs {
		if id == accountID {
			g.AccountIDs = append(g.AccountIDs[:i], g.AccountIDs[i+1:]...)
			return true
		}
	}
	return false
}

// Clone creates a deep copy of the group.
func (g *Group) Clone() *Group {
	clone := &Group{
		ID:          g.ID,
		Name:        g.Name,
		Description: g.Description,
		Ranking:     g.Ranking,
	}

	if len(g.AccountIDs) > 0 {
		clone.AccountIDs = make([]string, len(g.AccountIDs))
		copy(clone.AccountIDs, g.AccountIDs)
	}

	return clone
}
