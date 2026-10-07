package microsoftgraph

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"testing"
)

type graphTransport func(*http.Request) (*http.Response, error)

func (transport graphTransport) RoundTrip(request *http.Request) (*http.Response, error) {
	return transport(request)
}

// Runs against the complete generated Graph SDK without credentials or network.
func TestGraphSDKTypedUser(t *testing.T) {
	called := false
	client, err := NewClient(ClientConfig{
		BaseURL: "https://graph.example.test/v1.0", APIKey: "test-token", APIKeyPrefix: "Bearer",
		HTTPClient: &http.Client{Transport: graphTransport(func(request *http.Request) (*http.Response, error) {
			called = true
			if request.Method != "GET" || request.URL.EscapedPath() != "/v1.0/users/user%2F123" {
				t.Fatalf("unexpected request: %s %s", request.Method, request.URL)
			}
			if request.Header.Get("Authorization") != "Bearer test-token" {
				t.Fatal("missing bearer auth")
			}
			if request.URL.Query().Get("$select") != "displayName" {
				t.Fatal("missing OData query")
			}
			return &http.Response{StatusCode: 200, Header: http.Header{"Content-Type": {"application/json"}}, Body: io.NopCloser(strings.NewReader(`{"id":"user/123","displayName":"Ada"}`))}, nil
		})},
	})
	if err != nil {
		t.Fatal(err)
	}
	selectFields := []string{"displayName"}
	user, err := client.UsersUser.GetUser(context.Background(), &UsersUserGetUserRequest{UserID: "user/123", Select: &selectFields})
	if err != nil {
		t.Fatal(err)
	}
	if !called || user.ID == nil || *user.ID != "user/123" || user.DisplayName == nil || *user.DisplayName != "Ada" {
		t.Fatalf("typed inherited fields were lost: %+v", user)
	}
}

// Graph exposes a declared additionalProperties field as well as extension data.
func TestGraphOptionalClaimExtensionRoundTrip(t *testing.T) {
	var claim MicrosoftGraphOptionalClaim
	if err := json.Unmarshal([]byte(`{"@odata.type":"#microsoft.graph.optionalClaim","additionalProperties":["emit_as_roles"],"future":{"enabled":true}}`), &claim); err != nil {
		t.Fatal(err)
	}
	if claim.AdditionalProperties == nil || len(*claim.AdditionalProperties) != 1 || (*claim.AdditionalProperties)[0] == nil || *(*claim.AdditionalProperties)[0] != "emit_as_roles" {
		t.Fatalf("declared field lost: %+v", claim)
	}
	raw, err := json.Marshal(claim)
	if err != nil {
		t.Fatal(err)
	}
	var wire map[string]json.RawMessage
	if err = json.Unmarshal(raw, &wire); err != nil {
		t.Fatal(err)
	}
	if string(wire["additionalProperties"]) != `["emit_as_roles"]` || string(wire["future"]) != `{"enabled":true}` {
		t.Fatal(string(raw))
	}
}
