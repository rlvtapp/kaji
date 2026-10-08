package provider

import (
	"context"
	"errors"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	rschema "github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/hashicorp/terraform-plugin-go/tftypes"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"
)

const pollingReady = `{"id":"child/id","organizationId":"a/b","name":"planned","status":"ready"}`
const pollingPending = `{"id":"child/id","organizationId":"a/b","name":"planned","status":"pending"}`

type pollingDriver struct {
	t                        *testing.T
	mode                     string
	action                   string
	mutations, reads, closed int
	cancel                   context.CancelFunc
}
type pollingBody struct {
	io.Reader
	driver *pollingDriver
}

func (b *pollingBody) Close() error { b.driver.closed++; return nil }
func (d *pollingDriver) Do(req *http.Request) (*http.Response, error) {
	if req.Header.Get("Authorization") != "Bearer private-token" {
		d.t.Fatal("auth lost during polling")
	}
	expected := "/organizations/a%2Fb/things"
	if req.Method != "POST" {
		expected += "/child%2Fid"
	}
	if req.URL.EscapedPath() != expected {
		d.t.Fatalf("identity escaping lost: %s", req.URL.EscapedPath())
	}
	status := 202
	body := pollingPending
	if req.Method != "GET" {
		d.mutations++
		if d.mutations > 1 {
			d.t.Fatal("polling replayed mutation")
		}
		if req.Method != "DELETE" {
			payload, _ := io.ReadAll(req.Body)
			if strings.Contains(string(payload), "organizationId") {
				d.t.Fatal("parent path field leaked into body")
			}
		}
	} else {
		d.reads++
		status = 200
		body = pollingReady
		switch d.mode {
		case "pending":
			body = pollingPending
		case "and":
			if d.reads == 1 {
				status = 202
			} else if d.reads == 2 {
				body = pollingPending
			}
		case "failure":
			body = `{"id":"child/id","organizationId":"a/b","name":"planned","status":"ready","failed":true,"secret":"private-response"}`
		case "cancel":
			d.cancel()
			return nil, req.Context().Err()
		case "timeout":
			<-req.Context().Done()
			return nil, req.Context().Err()
		case "malformed":
			body = `{"status": "private-response"`
		case "trailing":
			body = pollingReady + ` {"secret":"private-response"}`
		case "duplicate":
			body = `{"status":"ready","status":"pending","secret":"private-response"}`
		case "oversize":
			body = strings.Repeat("private-response", 800000)
		case "denied":
			status = 403
			body = `{"secret":"private-response"}`
		case "missing":
			if d.reads == 1 {
				status = 404
				body = ""
			}
		default:
			if d.reads == 1 {
				body = pollingPending
			}
		}
		if d.action == "delete" && d.mode == "ready" && d.reads > 1 {
			status = 404
			body = ""
		}
	}
	return &http.Response{StatusCode: status, Header: http.Header{"Content-Type": []string{"application/json"}}, Body: &pollingBody{Reader: strings.NewReader(body), driver: d}}, nil
}
func pollingSchema(ctx context.Context, r *ThingResource) rschema.Schema {
	var response resource.SchemaResponse
	r.Schema(ctx, resource.SchemaRequest{}, &response)
	return response.Schema
}
func pollingPlan(t *testing.T, ctx context.Context, s rschema.Schema, created bool) tfsdk.Plan {
	model := ThingResourceModel{ID: types.StringUnknown(), OrganizationId: types.StringValue("a/b"), Name: types.StringValue("planned"), Status: types.StringUnknown()}
	if created {
		model.ID = types.StringValue(`{"org":"a/b","thingId":"child/id"}`)
	}
	plan := tfsdk.Plan{Schema: s, Raw: tftypes.NewValue(s.Type().TerraformType(ctx), nil)}
	if diagnostics := plan.Set(ctx, &model); diagnostics.HasError() {
		t.Fatal(diagnostics)
	}
	return plan
}
func pollingState(t *testing.T, ctx context.Context, s rschema.Schema) tfsdk.State {
	model := ThingResourceModel{ID: types.StringValue(`{"org":"a/b","thingId":"child/id"}`), OrganizationId: types.StringValue("a/b"), Name: types.StringValue("planned"), Status: types.StringValue("ready")}
	state := tfsdk.State{Schema: s, Raw: tftypes.NewValue(s.Type().TerraformType(ctx), nil)}
	if diagnostics := state.Set(ctx, &model); diagnostics.HasError() {
		t.Fatal(diagnostics)
	}
	return state
}
func TestPollingFrameworkLifecycle(t *testing.T) {
	for _, action := range []string{"create", "update", "delete"} {
		t.Run(action, func(t *testing.T) {
			ctx := context.Background()
			driver := &pollingDriver{t: t, mode: "ready", action: action}
			r := &ThingResource{client: &apiClient{baseURL: "https://example.test", token: "private-token", httpClient: driver}}
			schema := pollingSchema(ctx, r)
			state := pollingState(t, ctx, schema)
			switch action {
			case "create":
				response := resource.CreateResponse{State: tfsdk.State{Schema: schema, Raw: tftypes.NewValue(schema.Type().TerraformType(ctx), nil)}}
				r.Create(ctx, resource.CreateRequest{Plan: pollingPlan(t, ctx, schema, false)}, &response)
				if response.Diagnostics.HasError() {
					t.Fatal(response.Diagnostics)
				}
				state = response.State
			case "update":
				response := resource.UpdateResponse{State: state}
				r.Update(ctx, resource.UpdateRequest{Plan: pollingPlan(t, ctx, schema, true), State: state}, &response)
				if response.Diagnostics.HasError() {
					t.Fatal(response.Diagnostics)
				}
				state = response.State
			case "delete":
				response := resource.DeleteResponse{State: state}
				r.Delete(ctx, resource.DeleteRequest{State: state}, &response)
				if response.Diagnostics.HasError() || !response.State.Raw.IsNull() {
					t.Fatal("delete polling did not remove state", response.Diagnostics)
				}
			}
			if driver.mutations != 1 || driver.reads != 2 {
				t.Fatalf("wrong attempt counts: mutation=%d read=%d", driver.mutations, driver.reads)
			}
			if driver.closed != driver.reads+driver.mutations {
				t.Fatal("response body ownership leaked")
			}
			if action != "delete" {
				var model ThingResourceModel
				if diagnostics := state.Get(ctx, &model); diagnostics.HasError() {
					t.Fatal(diagnostics)
				}
				identity, err := ThingResourceParseIdentity(model.ID.ValueString())
				if err != nil || identity["org"] != "a/b" || identity["thingId"] != "child/id" || model.Status.ValueString() != "ready" {
					t.Fatal("poll completed without hydrated identity/state")
				}
			}
		})
	}
}
func TestPollingAllSuccessConditionsMustMatch(t *testing.T) {
	ctx := context.Background()
	driver := &pollingDriver{t: t, mode: "and"}
	r := &ThingResource{client: &apiClient{baseURL: "https://example.test", token: "private-token", httpClient: driver}}
	schema := pollingSchema(ctx, r)
	response := resource.CreateResponse{State: tfsdk.State{Schema: schema, Raw: tftypes.NewValue(schema.Type().TerraformType(ctx), nil)}}
	r.Create(ctx, resource.CreateRequest{Plan: pollingPlan(t, ctx, schema, false)}, &response)
	if response.Diagnostics.HasError() || driver.reads != 3 {
		t.Fatal("success criteria were not AND", response.Diagnostics, driver.reads)
	}
}

func TestCreatePollingToleratesEventualRead404(t *testing.T) {
	ctx := context.Background()
	driver := &pollingDriver{t: t, mode: "missing"}
	r := &ThingResource{client: &apiClient{baseURL: "https://example.test", token: "private-token", httpClient: driver}}
	schema := pollingSchema(ctx, r)
	response := resource.CreateResponse{State: tfsdk.State{Schema: schema, Raw: tftypes.NewValue(schema.Type().TerraformType(ctx), nil)}}
	r.Create(ctx, resource.CreateRequest{Plan: pollingPlan(t, ctx, schema, false)}, &response)
	if response.Diagnostics.HasError() || driver.reads != 2 || driver.mutations != 1 {
		t.Fatal("create did not tolerate eventual-consistency 404", response.Diagnostics)
	}
}
func TestPollingFailuresRetainManagedIdentityAndState(t *testing.T) {
	for _, action := range []string{"create", "update", "delete"} {
		for _, mode := range []string{"pending", "failure", "cancel", "timeout", "malformed", "trailing", "duplicate", "oversize", "denied"} {
			t.Run(action+"/"+mode, func(t *testing.T) {
				ctx, cancel := context.WithCancel(context.Background())
				defer cancel()
				driver := &pollingDriver{t: t, mode: mode, action: action, cancel: cancel}
				r := &ThingResource{client: &apiClient{baseURL: "https://example.test", token: "private-token", httpClient: driver}}
				schema := pollingSchema(ctx, r)
				state := pollingState(t, ctx, schema)
				started := time.Now()
				failed := false
				details := ""
				switch action {
				case "create":
					response := resource.CreateResponse{State: tfsdk.State{Schema: schema, Raw: tftypes.NewValue(schema.Type().TerraformType(ctx), nil)}}
					r.Create(ctx, resource.CreateRequest{Plan: pollingPlan(t, ctx, schema, false)}, &response)
					state = response.State
					failed = response.Diagnostics.HasError()
					for _, d := range response.Diagnostics {
						details += d.Detail()
					}
				case "update":
					response := resource.UpdateResponse{State: state}
					r.Update(ctx, resource.UpdateRequest{Plan: pollingPlan(t, ctx, schema, true), State: state}, &response)
					state = response.State
					failed = response.Diagnostics.HasError()
					for _, d := range response.Diagnostics {
						details += d.Detail()
					}
				case "delete":
					response := resource.DeleteResponse{State: state}
					r.Delete(ctx, resource.DeleteRequest{State: state}, &response)
					state = response.State
					failed = response.Diagnostics.HasError()
					for _, d := range response.Diagnostics {
						details += d.Detail()
					}
				}
				if !failed {
					t.Fatal("poll failure reported success")
				}
				if strings.Contains(details, "private-response") || strings.Contains(details, "private-token") {
					t.Fatal("diagnostics disclosed credential/response")
				}
				if driver.mutations != 1 || driver.reads > 3 {
					t.Fatal("unbounded waiter or replayed mutation")
				}
				if mode == "failure" && driver.reads != 1 {
					t.Fatal("failure criteria did not beat matching success")
				}
				if mode == "pending" && driver.reads != 3 {
					t.Fatal("max attempts ignored")
				}
				if time.Since(started) > time.Second {
					t.Fatal("timeout/cancellation not bounded")
				}
				if state.Raw.IsNull() || !state.Raw.IsFullyKnown() {
					t.Fatal("failed operation lost recoverable known state")
				}
				var model ThingResourceModel
				if diagnostics := state.Get(context.Background(), &model); diagnostics.HasError() {
					t.Fatal(diagnostics)
				}
				identity, err := ThingResourceParseIdentity(model.ID.ValueString())
				if err != nil || identity["thingId"] != "child/id" || identity["org"] != "a/b" {
					t.Fatal("accepted create identity not saved")
				}
				if action != "create" && model.Status.ValueString() != "ready" {
					t.Fatal("failed update/delete mutated prior state")
				}
				if mode == "cancel" && !errors.Is(ctx.Err(), context.Canceled) {
					t.Fatal("cancellation cause lost")
				}
			})
		}
	}
}

func TestPollingCancellationInterruptsDelays(t *testing.T) {
	for _, initial := range []bool{true, false} {
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
		driver := &pollingDriver{t: t, mode: "pending"}
		client := &apiClient{baseURL: "https://example.test", token: "private-token", httpClient: driver}
		config := pollingConfig{intervalMS: 1000, maxAttempts: 3, success: []pollingCriterion{{status: 200}, {pointer: "/status", expected: `"ready"`, body: true}}}
		if initial {
			config.delayMS = 1000
		}
		started := time.Now()
		_, err := client.waitPolling(ctx, "/organizations/a%2Fb/things/child%2Fid", true, config, true, false)
		cancel()
		if !errors.Is(err, context.DeadlineExceeded) || time.Since(started) > time.Second {
			t.Fatal("delay did not honor cancellation", err)
		}
		expected := 1
		if initial {
			expected = 0
		}
		if driver.reads != expected {
			t.Fatal("extra request while cancelled", driver.reads)
		}
	}
}

func TestPollingCriteriaPreservePrecisionPresenceAndPointerEscapes(t *testing.T) {
	value, err := pollingJSON([]byte(`{"a/b":{"~key":[9007199254740993]},"enabled":false,"count":0,"note":null}`))
	if err != nil {
		t.Fatal(err)
	}
	criteria := []pollingCriterion{
		{status: 200},
		{pointer: "/a~1b/~0key/0", expected: "9007199254740993", body: true},
		{pointer: "/enabled", expected: "false", body: true},
		{pointer: "/count", expected: "0", body: true},
		{pointer: "/note", expected: "null", body: true},
	}
	if !pollingMatch(criteria, 200, value) {
		t.Fatal("valid scalar criteria lost precision or presence")
	}
	for _, criterion := range []pollingCriterion{
		{pointer: "/a~1b/~0key/0", expected: "9007199254740992", body: true},
		{pointer: "/a~1b/~0key/00", expected: "9007199254740993", body: true},
		{pointer: "/missing", expected: "null", body: true},
		{pointer: "/count", expected: `"0"`, body: true},
	} {
		if pollingMatch([]pollingCriterion{criterion}, 200, value) {
			t.Fatal("invalid/missing/type-coerced criterion matched")
		}
	}
	if pollingMatch(criteria, 202, value) {
		t.Fatal("HTTP status criterion ignored")
	}
}

func TestPollingJSONRejectsAmbiguityAndUnboundedNumbers(t *testing.T) {
	for _, body := range [][]byte{
		[]byte(`{"nested":{"private-secret":0,"private-secret":1}}`),
		[]byte(`{"secret":0} {"secret":1}`),
		[]byte(`{"secret":1e999999999}`),
		[]byte{'"', 0xff, '"'},
	} {
		_, err := pollingJSON(body)
		if err == nil || strings.Contains(err.Error(), "secret") {
			t.Fatal("invalid JSON accepted or leaked", err)
		}
	}
	left, err := pollingJSON([]byte("1"))
	if err != nil {
		t.Fatal(err)
	}
	right, err := pollingJSON([]byte("1.0"))
	if err != nil {
		t.Fatal(err)
	}
	if !pollingEqual(left, right) {
		t.Fatal("equivalent JSON numbers differ")
	}
}
