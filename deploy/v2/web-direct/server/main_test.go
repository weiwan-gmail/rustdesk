package main

import (
	"net"
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
		*defaultTarget = ""
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
	js := runtimeConfigJS(false, "vp8", "")
	if !strings.Contains(js, `videoCodec: "vp8"`) || !strings.Contains(js, "direct: true") {
		t.Fatalf("codec config: %s", js)
	}
	if strings.Contains(js, "control: true") {
		t.Fatalf("control should stay off: %s", js)
	}
	plain := runtimeConfigJS(true, "", "")
	if strings.Contains(plain, "videoCodec") {
		t.Fatalf("omitted codec should not appear: %s", plain)
	}
}

func TestRuntimeConfigDefaultTarget(t *testing.T) {
	js := runtimeConfigJS(false, "", "192.168.1.50")
	if !strings.Contains(js, `defaultTarget: "192.168.1.50"`) || !strings.Contains(js, "direct: true") {
		t.Fatalf("defaultTarget config: %s", js)
	}
	if strings.Contains(js, "control: true") || strings.Contains(js, "videoCodec") {
		t.Fatalf("unrelated flags should stay off: %s", js)
	}
	plain := runtimeConfigJS(false, "vp8", "  ")
	if strings.Contains(plain, "defaultTarget") {
		t.Fatalf("blank defaultTarget should be omitted: %s", plain)
	}
}

func TestServeRuntimeConfigDefaultTarget(t *testing.T) {
	t.Cleanup(func() {
		*control = false
		*videoCodec = ""
		*defaultTarget = ""
	})
	*defaultTarget = "10.0.0.8"
	rec := httptest.NewRecorder()
	serveRuntimeConfig(rec, httptest.NewRequest(http.MethodGet, "/config.js", nil))
	body := rec.Body.String()
	if !strings.Contains(body, `defaultTarget: "10.0.0.8"`) || !strings.Contains(body, "direct: true") {
		t.Fatalf("runtime defaultTarget: %s", body)
	}
}

func withDefaultAllowlist(t *testing.T) {
	t.Helper()
	nets, err := buildAllowedNets("", false)
	if err != nil {
		t.Fatal(err)
	}
	prevNets, prevLookup := allowedNets, lookupIP
	allowedNets = nets
	t.Cleanup(func() {
		allowedNets = prevNets
		lookupIP = prevLookup
	})
}

func TestValidateTargetIPLiteral(t *testing.T) {
	withDefaultAllowlist(t)
	got, err := validateTarget("192.168.1.50:21118")
	if err != nil {
		t.Fatalf("private IP: %v", err)
	}
	if got != "192.168.1.50:21118" {
		t.Fatalf("got %q", got)
	}
	if _, err := validateTarget("8.8.8.8:21118"); err == nil {
		t.Fatal("public IP should be rejected")
	}
	if _, err := validateTarget("192.168.1.50:22"); err == nil {
		t.Fatal("non-direct port should be rejected")
	}
	got, err = validateTarget("192.168.1.50")
	if err != nil || got != "192.168.1.50:21118" {
		t.Fatalf("bare private IP: %q %v", got, err)
	}
}

func TestValidateTargetHostnamePrivateResolve(t *testing.T) {
	withDefaultAllowlist(t)
	lookupIP = func(host string) ([]net.IP, error) {
		if host != "xxx.yy.com" {
			t.Fatalf("unexpected host %q", host)
		}
		return []net.IP{net.ParseIP("192.168.10.4")}, nil
	}
	got, err := validateTarget("xxx.yy.com:21118")
	if err != nil {
		t.Fatalf("split-horizon private: %v", err)
	}
	if got != "192.168.10.4:21118" {
		t.Fatalf("got %q, want resolved private IP", got)
	}
	got, err = validateTarget("xxx.yy.com")
	if err != nil || got != "192.168.10.4:21118" {
		t.Fatalf("hostname without port: %q %v", got, err)
	}
}

func TestValidateTargetHostnamePublicResolveDenied(t *testing.T) {
	withDefaultAllowlist(t)
	lookupIP = func(string) ([]net.IP, error) {
		return []net.IP{net.ParseIP("8.8.8.8")}, nil
	}
	if _, err := validateTarget("xxx.yy.com:21118"); err == nil {
		t.Fatal("public A record should be rejected without --allow-any")
	}
}

func TestValidateTargetHostnamePrefersPrivateIPv4(t *testing.T) {
	withDefaultAllowlist(t)
	lookupIP = func(string) ([]net.IP, error) {
		return []net.IP{
			net.ParseIP("8.8.8.8"),
			net.ParseIP("2001:db8::1"),
			net.ParseIP("10.1.2.3"),
			net.ParseIP("fd00::1"),
		}, nil
	}
	got, err := validateTarget("pc.local:21118")
	if err != nil {
		t.Fatal(err)
	}
	if got != "10.1.2.3:21118" {
		t.Fatalf("got %q, want private IPv4", got)
	}
}

func TestValidateTargetNumericIDNotHostname(t *testing.T) {
	withDefaultAllowlist(t)
	lookupIP = func(string) ([]net.IP, error) {
		t.Fatal("digit-only targets must not be resolved")
		return nil, nil
	}
	for _, id := range []string{"123456789", "123456789:21118"} {
		if _, err := validateTarget(id); err == nil {
			t.Fatalf("%q should not be a hostname", id)
		}
	}
}

func TestValidateTargetAllowAnyPublicHostname(t *testing.T) {
	prevNets, prevLookup := allowedNets, lookupIP
	allowedNets, _ = buildAllowedNets("", true)
	lookupIP = func(string) ([]net.IP, error) {
		return []net.IP{net.ParseIP("8.8.8.8")}, nil
	}
	t.Cleanup(func() {
		allowedNets = prevNets
		lookupIP = prevLookup
	})
	got, err := validateTarget("xxx.yy.com:21118")
	if err != nil || got != "8.8.8.8:21118" {
		t.Fatalf("--allow-any public: %q %v", got, err)
	}
}

func TestValidateTargetLocalhostRealDNS(t *testing.T) {
	withDefaultAllowlist(t)
	got, err := validateTarget("localhost:21118")
	if err != nil {
		t.Fatalf("localhost: %v", err)
	}
	host, port, err := net.SplitHostPort(got)
	if err != nil || port != "21118" {
		t.Fatalf("got %q %v", got, err)
	}
	ip := net.ParseIP(host)
	if ip == nil || !ipAllowed(ip) {
		t.Fatalf("localhost resolved to non-allowed %q", got)
	}
}

func TestValidateTargetPublicNameRealDNSDenied(t *testing.T) {
	withDefaultAllowlist(t)
	if _, err := validateTarget("example.com:21118"); err == nil {
		t.Fatal("example.com must be rejected without --allow-any")
	}
}
