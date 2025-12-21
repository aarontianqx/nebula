// Package account defines the Account entity and related types.
package account

import "fmt"

// Account represents a game account with authentication credentials and metadata.
type Account struct {
	// ID is the unique identifier (MongoDB ObjectID)
	ID string

	// RoleName is the in-game character name
	RoleName string

	// UserName is the login username
	UserName string

	// Password is the login password
	Password string

	// Ranking is the account's ranking/priority for sorting
	Ranking int

	// ServerID is the game server identifier
	ServerID int

	// Cookies stores browser cookies for session restoration
	Cookies []Cookie
}

// Cookie represents a browser cookie for session persistence.
type Cookie struct {
	Name         string
	Value        string
	Domain       string
	Path         string
	HTTPOnly     bool
	Secure       bool
	SourcePort   int
	SourceScheme string
	Priority     string
}

// Identity returns a human-readable identifier for the account.
// Format: "ServerID - RoleName" to distinguish accounts across different servers.
func (a *Account) Identity() string {
	return fmt.Sprintf("%d - %s", a.ServerID, a.RoleName)
}

// HasCookies returns true if the account has stored cookies.
func (a *Account) HasCookies() bool {
	return len(a.Cookies) > 0
}

// Clone creates a deep copy of the account.
func (a *Account) Clone() *Account {
	clone := &Account{
		ID:       a.ID,
		RoleName: a.RoleName,
		UserName: a.UserName,
		Password: a.Password,
		Ranking:  a.Ranking,
		ServerID: a.ServerID,
	}

	if len(a.Cookies) > 0 {
		clone.Cookies = make([]Cookie, len(a.Cookies))
		copy(clone.Cookies, a.Cookies)
	}

	return clone
}
