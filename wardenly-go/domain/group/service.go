package group

import (
	"context"
	"sort"

	"wardenly-go/domain/account"
)

// Service provides business logic for group management.
type Service struct {
	groupRepo   Repository
	accountRepo account.Repository
}

// NewService creates a new group service.
func NewService(groupRepo Repository, accountRepo account.Repository) *Service {
	return &Service{
		groupRepo:   groupRepo,
		accountRepo: accountRepo,
	}
}

// ResolvedGroup contains a group with its resolved accounts.
type ResolvedGroup struct {
	Group    *Group
	Accounts []*account.Account
}

// GetGroup retrieves a group by ID.
func (s *Service) GetGroup(ctx context.Context, id string) (*Group, error) {
	grp, err := s.groupRepo.FindByID(ctx, id)
	if err != nil {
		return nil, err
	}
	if grp == nil {
		return nil, ErrGroupNotFound
	}
	return grp, nil
}

// GetGroupByName retrieves a group by name.
func (s *Service) GetGroupByName(ctx context.Context, name string) (*Group, error) {
	grp, err := s.groupRepo.FindByName(ctx, name)
	if err != nil {
		return nil, err
	}
	if grp == nil {
		return nil, ErrGroupNotFound
	}
	return grp, nil
}

// GetGroupWithAccounts loads a group and resolves its account IDs to actual accounts.
// Invalid account IDs are silently ignored.
func (s *Service) GetGroupWithAccounts(ctx context.Context, groupID string) (*ResolvedGroup, error) {
	grp, err := s.groupRepo.FindByID(ctx, groupID)
	if err != nil {
		return nil, err
	}
	if grp == nil {
		return nil, ErrGroupNotFound
	}

	accounts := make([]*account.Account, 0, len(grp.AccountIDs))
	for _, accID := range grp.AccountIDs {
		acc, err := s.accountRepo.FindByID(ctx, accID)
		if err != nil {
			continue // Skip on error
		}
		if acc != nil {
			accounts = append(accounts, acc)
		}
		// Silently skip invalid/missing accounts
	}

	// Sort by ranking (lower ranking = higher priority)
	sort.Slice(accounts, func(i, j int) bool {
		return accounts[i].Ranking < accounts[j].Ranking
	})

	return &ResolvedGroup{
		Group:    grp,
		Accounts: accounts,
	}, nil
}

// GetGroupWithAccountsByName loads a group by name and resolves its accounts.
func (s *Service) GetGroupWithAccountsByName(ctx context.Context, name string) (*ResolvedGroup, error) {
	grp, err := s.groupRepo.FindByName(ctx, name)
	if err != nil {
		return nil, err
	}
	if grp == nil {
		return nil, ErrGroupNotFound
	}

	accounts := make([]*account.Account, 0, len(grp.AccountIDs))
	for _, accID := range grp.AccountIDs {
		acc, err := s.accountRepo.FindByID(ctx, accID)
		if err != nil {
			continue
		}
		if acc != nil {
			accounts = append(accounts, acc)
		}
	}

	// Sort by ranking first, then by ID for stable ordering
	sort.Slice(accounts, func(i, j int) bool {
		if accounts[i].Ranking != accounts[j].Ranking {
			return accounts[i].Ranking < accounts[j].Ranking
		}
		return accounts[i].ID < accounts[j].ID
	})

	return &ResolvedGroup{
		Group:    grp,
		Accounts: accounts,
	}, nil
}

// ListAllGroups retrieves all groups sorted by ranking then ID.
func (s *Service) ListAllGroups(ctx context.Context) ([]*Group, error) {
	groups, err := s.groupRepo.FindAll(ctx)
	if err != nil {
		return nil, err
	}

	// Sort by ranking first, then by ID for stable ordering
	sort.Slice(groups, func(i, j int) bool {
		if groups[i].Ranking != groups[j].Ranking {
			return groups[i].Ranking < groups[j].Ranking
		}
		return groups[i].ID < groups[j].ID
	})

	return groups, nil
}

// CreateGroup creates a new group.
func (s *Service) CreateGroup(ctx context.Context, grp *Group) error {
	return s.groupRepo.Insert(ctx, grp)
}

// UpdateGroup updates an existing group.
func (s *Service) UpdateGroup(ctx context.Context, grp *Group) error {
	return s.groupRepo.Update(ctx, grp)
}

// DeleteGroup removes a group.
func (s *Service) DeleteGroup(ctx context.Context, id string) error {
	return s.groupRepo.Delete(ctx, id)
}

// CleanupAccountFromGroups removes an account ID from all groups.
// Called when an account is deleted.
func (s *Service) CleanupAccountFromGroups(ctx context.Context, accountID string) error {
	groups, err := s.groupRepo.FindByAccountID(ctx, accountID)
	if err != nil {
		return err
	}

	for _, grp := range groups {
		grp.RemoveAccount(accountID)
		if err := s.groupRepo.Update(ctx, grp); err != nil {
			return err
		}
	}

	return nil
}

// GetGroupsForAccount returns all groups that contain the specified account.
func (s *Service) GetGroupsForAccount(ctx context.Context, accountID string) ([]*Group, error) {
	return s.groupRepo.FindByAccountID(ctx, accountID)
}
