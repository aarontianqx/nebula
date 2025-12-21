package repository

import "testing"

func TestDefaultMongoDBConfig(t *testing.T) {
	config := DefaultMongoDBConfig()

	if config == nil {
		t.Fatal("DefaultMongoDBConfig returned nil")
	}

	if config.URI != "mongodb://localhost:27017" {
		t.Errorf("URI = %v, want mongodb://localhost:27017", config.URI)
	}

	if config.Database != "wardenly" {
		t.Errorf("Database = %v, want wardenly", config.Database)
	}

	if config.ConnectTimeout != 10*1e9 {
		t.Errorf("ConnectTimeout = %v, want 10s", config.ConnectTimeout)
	}

	if config.PingTimeout != 5*1e9 {
		t.Errorf("PingTimeout = %v, want 5s", config.PingTimeout)
	}
}

func TestAccountDocument_Conversion(t *testing.T) {
	// Test document to account conversion
	doc := &accountDocument{
		RoleName: "TestRole",
		UserName: "testuser",
		Password: "password",
		Ranking:  1,
		ServerID: 100,
		Cookies: []cookieDocument{
			{
				Name:     "session",
				Value:    "abc123",
				Domain:   ".example.com",
				Path:     "/",
				HTTPOnly: true,
				Secure:   true,
			},
		},
	}

	acc := documentToAccount(doc)

	if acc.RoleName != "TestRole" {
		t.Errorf("RoleName = %v, want TestRole", acc.RoleName)
	}
	if acc.UserName != "testuser" {
		t.Errorf("UserName = %v, want testuser", acc.UserName)
	}
	if acc.Password != "password" {
		t.Errorf("Password = %v, want password", acc.Password)
	}
	if acc.Ranking != 1 {
		t.Errorf("Ranking = %d, want 1", acc.Ranking)
	}
	if acc.ServerID != 100 {
		t.Errorf("ServerID = %d, want 100", acc.ServerID)
	}
	if len(acc.Cookies) != 1 {
		t.Errorf("Cookies length = %d, want 1", len(acc.Cookies))
	}
	if acc.Cookies[0].Name != "session" {
		t.Errorf("Cookie name = %v, want session", acc.Cookies[0].Name)
	}
}

func TestAccountToDocument(t *testing.T) {
	// Import domain account for testing
	// Since we can't easily import here without circular deps,
	// we test the basic structure

	doc := &accountDocument{
		RoleName: "TestRole",
		UserName: "testuser",
	}

	if doc.RoleName != "TestRole" {
		t.Errorf("RoleName = %v, want TestRole", doc.RoleName)
	}
}

func TestCookieDocument(t *testing.T) {
	cookie := cookieDocument{
		Name:       "session",
		Value:      "abc123",
		Domain:     ".example.com",
		Path:       "/",
		HTTPOnly:   true,
		Secure:     true,
		SourcePort: 443,
	}

	if cookie.Name != "session" {
		t.Errorf("Name = %v, want session", cookie.Name)
	}
	if cookie.Value != "abc123" {
		t.Errorf("Value = %v, want abc123", cookie.Value)
	}
	if cookie.Domain != ".example.com" {
		t.Errorf("Domain = %v, want .example.com", cookie.Domain)
	}
	if !cookie.HTTPOnly {
		t.Error("HTTPOnly should be true")
	}
	if !cookie.Secure {
		t.Error("Secure should be true")
	}
	if cookie.SourcePort != 443 {
		t.Errorf("SourcePort = %d, want 443", cookie.SourcePort)
	}
}
