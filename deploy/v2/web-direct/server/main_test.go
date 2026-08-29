package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestControlOffHasNoRoute(t *testing.T) {
	mux := http.NewServeMux()
	mux.HandleFunc("/direct", func(http.ResponseWriter, *http.Request) {})
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/control?target=desk", nil))
	if rec.Code != http.StatusNotFound {
		t.Fatalf("disabled /control status %d", rec.Code)
	}
}

func TestControlOnRejectsNonWS(t *testing.T) {
	mux := http.NewServeMux()
	attachControlRoom(mux, false)
	rec := httptest.NewRecorder()
	mux.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/control?target=desk", nil))
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("enabled /control without WS: %d %s", rec.Code, rec.Body.String())
	}
}

func TestRuntimeConfigEnablesControl(t *testing.T) {
	rec := httptest.NewRecorder()
	serveRuntimeConfig(rec, httptest.NewRequest(http.MethodGet, "/config.js", nil))
	body := rec.Body.String()
	if !strings.Contains(body, "control: true") || !strings.Contains(body, "direct: true") {
		t.Fatalf("runtime config: %s", body)
	}
}
