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
	t.Cleanup(func() {
		*control = false
		*videoCodec = ""
	})
	*control = true
	*videoCodec = ""
	rec := httptest.NewRecorder()
	serveRuntimeConfig(rec, httptest.NewRequest(http.MethodGet, "/config.js", nil))
	body := rec.Body.String()
	if !strings.Contains(body, "control: true") || !strings.Contains(body, "direct: true") {
		t.Fatalf("runtime config: %s", body)
	}
}

func TestNormalizeVideoCodec(t *testing.T) {
	ok, err := normalizeVideoCodec("VP8")
	if err != nil || ok != "vp8" {
		t.Fatalf("vp8: %q %v", ok, err)
	}
	if _, err := normalizeVideoCodec("h264"); err == nil {
		t.Fatal("h264 should be rejected on web-direct")
	}
	empty, err := normalizeVideoCodec("")
	if err != nil || empty != "" {
		t.Fatalf("empty: %q %v", empty, err)
	}
}

func TestRuntimeConfigVideoCodec(t *testing.T) {
	js := runtimeConfigJS(false, "vp8")
	if !strings.Contains(js, `videoCodec: "vp8"`) || !strings.Contains(js, "direct: true") {
		t.Fatalf("codec config: %s", js)
	}
	if strings.Contains(js, "control: true") {
		t.Fatalf("control should stay off: %s", js)
	}
	plain := runtimeConfigJS(true, "")
	if strings.Contains(plain, "videoCodec") {
		t.Fatalf("omitted codec should not appear: %s", plain)
	}
}
