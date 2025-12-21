package account

import "testing"

func TestAccount_Identity(t *testing.T) {
	tests := []struct {
		name     string
		account  *Account
		expected string
	}{
		{
			name:     "with server and role",
			account:  &Account{ServerID: 123, RoleName: "TestRole"},
			expected: "123 - TestRole",
		},
		{
			name:     "server zero with role",
			account:  &Account{ServerID: 0, RoleName: "TestRole"},
			expected: "0 - TestRole",
		},
		{
			name:     "server only",
			account:  &Account{ServerID: 456},
			expected: "456 - ",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.account.Identity(); got != tt.expected {
				t.Errorf("Identity() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestAccount_HasCookies(t *testing.T) {
	tests := []struct {
		name     string
		account  *Account
		expected bool
	}{
		{
			name:     "with cookies",
			account:  &Account{Cookies: []Cookie{{Name: "session"}}},
			expected: true,
		},
		{
			name:     "without cookies",
			account:  &Account{},
			expected: false,
		},
		{
			name:     "empty cookies slice",
			account:  &Account{Cookies: []Cookie{}},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.account.HasCookies(); got != tt.expected {
				t.Errorf("HasCookies() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestAccount_Clone(t *testing.T) {
	original := &Account{
		ID:       "123",
		RoleName: "TestRole",
		UserName: "testuser",
		Password: "password",
		Ranking:  1,
		ServerID: 100,
		Cookies:  []Cookie{{Name: "session", Value: "abc123"}},
	}

	clone := original.Clone()

	// Verify values are copied
	if clone.ID != original.ID {
		t.Errorf("ID not copied: got %v, want %v", clone.ID, original.ID)
	}
	if clone.RoleName != original.RoleName {
		t.Errorf("RoleName not copied")
	}
	if clone.UserName != original.UserName {
		t.Errorf("UserName not copied")
	}
	if clone.Password != original.Password {
		t.Errorf("Password not copied")
	}
	if clone.Ranking != original.Ranking {
		t.Errorf("Ranking not copied")
	}
	if clone.ServerID != original.ServerID {
		t.Errorf("ServerID not copied")
	}

	// Verify slices are deep copied
	if len(clone.Cookies) != len(original.Cookies) {
		t.Errorf("Cookies length mismatch")
	}

	// Modify clone and verify original is unchanged
	clone.Cookies[0].Name = "modified"
	if original.Cookies[0].Name == "modified" {
		t.Error("Cookies slice was not deep copied")
	}
}

func TestAccount_Clone_EmptySlices(t *testing.T) {
	original := &Account{
		ID:       "123",
		RoleName: "TestRole",
	}

	clone := original.Clone()

	if clone.Cookies != nil {
		t.Error("Expected nil Cookies for empty original")
	}
}
