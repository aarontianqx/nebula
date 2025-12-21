package account

import (
	"context"
	"errors"
	"sort"
)

// Common errors for account operations.
var (
	ErrAccountNotFound = errors.New("account not found")
	ErrDuplicateID     = errors.New("account with this ID already exists")
)

// Service provides business logic for account management.
type Service struct {
	repo Repository
}

// NewService creates a new account service.
func NewService(repo Repository) *Service {
	return &Service{repo: repo}
}

// GetAccount retrieves an account by ID.
func (s *Service) GetAccount(ctx context.Context, id string) (*Account, error) {
	account, err := s.repo.FindByID(ctx, id)
	if err != nil {
		return nil, err
	}
	if account == nil {
		return nil, ErrAccountNotFound
	}
	return account, nil
}

// ListAccounts retrieves all accounts, sorted by ranking then ID.
func (s *Service) ListAccounts(ctx context.Context) ([]*Account, error) {
	accounts, err := s.repo.FindAll(ctx)
	if err != nil {
		return nil, err
	}

	// Sort by ranking first, then by ID for stable ordering
	sort.Slice(accounts, func(i, j int) bool {
		if accounts[i].Ranking != accounts[j].Ranking {
			return accounts[i].Ranking < accounts[j].Ranking
		}
		return accounts[i].ID < accounts[j].ID
	})

	return accounts, nil
}

// SaveCookies updates the cookies for an account.
func (s *Service) SaveCookies(ctx context.Context, id string, cookies []Cookie) error {
	return s.repo.UpdateCookies(ctx, id, cookies)
}

// CreateAccount creates a new account.
func (s *Service) CreateAccount(ctx context.Context, account *Account) error {
	return s.repo.Insert(ctx, account)
}

// UpdateAccount updates an existing account.
func (s *Service) UpdateAccount(ctx context.Context, account *Account) error {
	return s.repo.Update(ctx, account)
}

// DeleteAccount removes an account.
func (s *Service) DeleteAccount(ctx context.Context, id string) error {
	return s.repo.Delete(ctx, id)
}
