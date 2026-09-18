// rustdesk-web-v2-direct is a self-contained web server for the RustDesk v2
// web client in DIRECT mode: the browser connects to a controlled RustDesk
// client by IP, with no hbbs/hbbr server needed (similar to websockify for
// noVNC).
//
// It serves the embedded v2 web client and bridges /direct WebSocket
// connections to the controlled client's direct-access TCP port.
//
// This server is version-agnostic delivery infrastructure. The retired v1
// counterpart is archived at deploy/v1_backup/web-direct/server; only the
// embedded static client differs (here: the current flutter/ tree build).
//
// Pure standard library, no third-party dependencies.
//
//	./rustdesk-web-v2-direct --listen :8081 --open
package main

import (
	"embed"
	"flag"
	"fmt"
	"io/fs"
	"log"
	"net"
	"net/http"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"time"

	"rustdesk-web-controlroom"
)

//go:embed all:static
var staticFS embed.FS

var (
	listen             = flag.String("listen", ":8081", "address the web page is served on")
	directPort         = flag.Int("direct-port", 21118, "the only allowed target port (the controlled client's direct-access port)")
	allowCIDR          = flag.String("allow-cidr", "", "comma-separated CIDRs allowed as direct targets (default: loopback/private/link-local only)")
	allowAny           = flag.Bool("allow-any", false, "disable target IP restrictions (DANGEROUS: the proxy can then reach arbitrary hosts)")
	basePath           = flag.String("base-path", "/", "URL path the client is mounted under (must match the build's BASE_HREF)")
	tlsCert            = flag.String("tls-cert", "", "TLS certificate file; plain HTTP when empty (fine for intranet/localhost)")
	tlsKey             = flag.String("tls-key", "", "TLS key file")
	open               = flag.Bool("open", false, "open the page in the system browser after start")
	control            = flag.Bool("control", false, "enable exclusive control room at /control (off by default)")
	controlAutoApprove = flag.Bool("control-auto-approve", false, "approve every control request immediately (implies --control)")
	videoCodec         = flag.String("video-codec", "", "web decode preference: auto, vp8, or vp9 (ogv.js). Empty keeps the page default (auto).")
	defaultTarget      = flag.String("default-target", "", "optional first-visit Remote ID prefill (config.js defaultTarget); overrides the page hostname when set")
)

var allowedNets []*net.IPNet

// lookupIP is net.LookupIP in production; tests replace it to simulate
// split-horizon / internal DNS without touching the system resolver.
var lookupIP = net.LookupIP

// bridgeSem bounds concurrent /direct bridges so unauthenticated sessions
// can't exhaust memory/goroutines.
var bridgeSem = make(chan struct{}, 32)

func main() {
	flag.Parse()
	codec, err := normalizeVideoCodec(*videoCodec)
	if err != nil {
		log.Fatalf("%v", err)
	}
	*videoCodec = codec
	allowedNets, err = buildAllowedNets(*allowCIDR, *allowAny)
	if err != nil {
		log.Fatalf("invalid --allow-cidr: %v", err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/direct", handleDirect)
	if controlOn() || videoCodecSet() || defaultTargetSet() {
		if controlOn() {
			attachControlRoom(mux, *controlAutoApprove)
			log.Printf("control room: /control (auto-approve=%v)", *controlAutoApprove)
		}
		mux.HandleFunc("/config.js", serveRuntimeConfig)
		if videoCodecSet() {
			log.Printf("video codec preference: %s", *videoCodec)
		}
		if defaultTargetSet() {
			log.Printf("default target: %s", strings.TrimSpace(*defaultTarget))
		}
	}

	static, err := fs.Sub(staticFS, "static")
	if err != nil {
		log.Fatal(err)
	}
	mux.Handle("/", mount(static))

	page := fmt.Sprintf("http://localhost%s%s", displayPort(*listen), *basePath)
	log.Printf("RustDesk web client (direct mode): %s", page)
	log.Printf("direct targets: port %d, %s", *directPort, allowedDesc())
	if *open {
		go openBrowser(page)
	}

	var serveErr error
	if *tlsCert != "" && *tlsKey != "" {
		serveErr = http.ListenAndServeTLS(*listen, *tlsCert, *tlsKey, mux)
	} else {
		serveErr = http.ListenAndServe(*listen, mux)
	}
	log.Fatal(serveErr)
}

func displayPort(listen string) string {
	if strings.HasPrefix(listen, ":") {
		return listen
	}
	_, port, err := net.SplitHostPort(listen)
	if err != nil {
		return listen
	}
	return ":" + port
}

func controlOn() bool {
	return *control || *controlAutoApprove
}

func videoCodecSet() bool {
	return *videoCodec != ""
}

func defaultTargetSet() bool {
	return strings.TrimSpace(*defaultTarget) != ""
}

func normalizeVideoCodec(s string) (string, error) {
	v := strings.ToLower(strings.TrimSpace(s))
	switch v {
	case "", "auto", "vp8", "vp9":
		return v, nil
	default:
		return "", fmt.Errorf("invalid --video-codec %q (want auto, vp8, or vp9; web-direct paints VP8/VP9 only)", s)
	}
}

func runtimeConfigJS(control bool, videoCodec, defaultTarget string) string {
	var b strings.Builder
	b.WriteString(`window.RUSTDESK_CONFIG = {server: "", wsIdPath: "/ws/id", wsRelayPath: "/ws/relay", direct: true`)
	if control {
		b.WriteString(`, control: true, controlPath: "/control", controlBar: true`)
	}
	if videoCodec != "" {
		fmt.Fprintf(&b, `, videoCodec: %q`, videoCodec)
	}
	if t := strings.TrimSpace(defaultTarget); t != "" {
		fmt.Fprintf(&b, `, defaultTarget: %q`, t)
	}
	b.WriteString("};\n")
	return b.String()
}

func attachControlRoom(mux *http.ServeMux, autoApprove bool) {
	h := controlroom.NewHub(autoApprove)
	mux.Handle("/control", h)
}

func serveRuntimeConfig(w http.ResponseWriter, _ *http.Request) {
	w.Header().Set("Content-Type", "application/javascript")
	fmt.Fprint(w, runtimeConfigJS(controlOn(), *videoCodec, *defaultTarget))
}

// handleDirect bridges /direct?target=HOST:PORT to the controlled client's
// direct-access TCP port. HOST may be an IP literal or a hostname that
// resolves to an allowlisted address.
func handleDirect(w http.ResponseWriter, r *http.Request) {
	target := r.URL.Query().Get("target")
	addr, err := validateTarget(target)
	if err != nil {
		http.Error(w, "invalid target: "+err.Error(), http.StatusForbidden)
		return
	}
	select {
	case bridgeSem <- struct{}{}:
		defer func() { <-bridgeSem }()
	default:
		http.Error(w, "too many connections", http.StatusServiceUnavailable)
		return
	}
	ws, err := acceptWS(w, r)
	if err != nil {
		// not a websocket request; acceptWS already failed, nothing written yet
		http.Error(w, "websocket required", http.StatusBadRequest)
		return
	}
	tcp, err := net.DialTimeout("tcp", addr, 10*time.Second)
	if err != nil {
		log.Printf("direct dial %s: %v", addr, err)
		ws.Close()
		return
	}
	log.Printf("direct bridge %s <-> %s", r.RemoteAddr, addr)
	bridge(ws, tcp)
}

// validateTarget enforces the single allowed direct port and the IP
// allowlist — this is what stops the proxy from becoming an open TCP relay
// (SSRF). Host may be an IP literal or a hostname (LAN name, .local, or a
// public-looking FQDN). Hostnames are always DNS-resolved; the connection
// is dialed to a chosen allowlisted address (IPv4 preferred). A name that
// resolves only to public IPs is rejected unless --allow-any.
func validateTarget(target string) (string, error) {
	if target == "" {
		return "", fmt.Errorf("missing target")
	}
	host, portStr, err := splitTargetHostPort(target)
	if err != nil {
		return "", err
	}
	port, err := strconv.Atoi(portStr)
	if err != nil || port <= 0 || port > 65535 {
		return "", fmt.Errorf("invalid port")
	}
	if port != *directPort {
		return "", fmt.Errorf("only port %d is allowed", *directPort)
	}
	ip := net.ParseIP(strings.Trim(host, "[]"))
	if ip != nil {
		if !ipAllowed(ip) {
			return "", fmt.Errorf("target %s not allowed", ip)
		}
		return net.JoinHostPort(ip.String(), portStr), nil
	}
	if !validHostname(host) {
		return "", fmt.Errorf("host must be an IP literal or hostname")
	}
	ips, err := lookupIP(host)
	if err != nil {
		return "", fmt.Errorf("resolve %s: %w", host, err)
	}
	chosen := pickAllowedIP(ips)
	if chosen == nil {
		return "", fmt.Errorf("target %s not allowed", host)
	}
	return net.JoinHostPort(chosen.String(), portStr), nil
}

func splitTargetHostPort(target string) (string, string, error) {
	host, portStr, err := net.SplitHostPort(target)
	if err == nil {
		return host, portStr, nil
	}
	if net.ParseIP(strings.Trim(target, "[]")) != nil || validHostname(target) {
		return strings.Trim(target, "[]"), strconv.Itoa(*directPort), nil
	}
	return "", "", fmt.Errorf("target must be host:port")
}

func validHostname(host string) bool {
	if host == "" || len(host) > 253 {
		return false
	}
	if isDigitOnly(host) {
		return false
	}
	host = strings.TrimSuffix(host, ".")
	if host == "" {
		return false
	}
	labels := strings.Split(host, ".")
	lastHasLetter := false
	for _, label := range labels {
		if !validHostnameLabel(label) {
			return false
		}
		lastHasLetter = hasASCIILetter(label)
	}
	return lastHasLetter
}

func validHostnameLabel(label string) bool {
	n := len(label)
	if n == 0 || n > 63 {
		return false
	}
	if label[0] == '-' || label[n-1] == '-' {
		return false
	}
	for i := 0; i < n; i++ {
		c := label[i]
		if c >= 'A' && c <= 'Z' {
			continue
		}
		if c >= 'a' && c <= 'z' {
			continue
		}
		if c >= '0' && c <= '9' {
			continue
		}
		if c == '-' {
			continue
		}
		return false
	}
	return true
}

func isDigitOnly(s string) bool {
	if s == "" {
		return false
	}
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return false
		}
	}
	return true
}

func hasASCIILetter(s string) bool {
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'A' && c <= 'Z' || c >= 'a' && c <= 'z' {
			return true
		}
	}
	return false
}

// pickAllowedIP chooses a resolved address that passes ipAllowed.
// Prefer an allowed IPv4 (typically RFC1918 / loopback / link-local).
func pickAllowedIP(ips []net.IP) net.IP {
	var first, firstV4 net.IP
	for _, ip := range ips {
		if ip == nil || !ipAllowed(ip) {
			continue
		}
		if first == nil {
			first = ip
		}
		if ip.To4() != nil && firstV4 == nil {
			firstV4 = ip
		}
	}
	if firstV4 != nil {
		return firstV4
	}
	return first
}

func buildAllowedNets(cidrs string, any bool) ([]*net.IPNet, error) {
	if any {
		return nil, nil // nil = allow all
	}
	if cidrs != "" {
		var nets []*net.IPNet
		for _, c := range strings.Split(cidrs, ",") {
			_, n, err := net.ParseCIDR(strings.TrimSpace(c))
			if err != nil {
				return nil, err
			}
			nets = append(nets, n)
		}
		return nets, nil
	}
	// default: loopback, RFC1918 private, link-local, IPv6 ULA/link-local
	var defaults []*net.IPNet
	for _, c := range []string{
		"127.0.0.0/8", "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16",
		"169.254.0.0/16", "::1/128", "fc00::/7", "fe80::/10",
	} {
		_, n, _ := net.ParseCIDR(c)
		defaults = append(defaults, n)
	}
	return defaults, nil
}

func ipAllowed(ip net.IP) bool {
	if allowedNets == nil { // --allow-any
		return true
	}
	for _, n := range allowedNets {
		if n.Contains(ip) {
			return true
		}
	}
	return false
}

func allowedDesc() string {
	if allowedNets == nil {
		return "ALL targets (--allow-any, dangerous)"
	}
	var s []string
	for _, n := range allowedNets {
		s = append(s, n.String())
	}
	return "allowed CIDRs: " + strings.Join(s, ", ")
}

// mount serves the embedded build under --base-path with index.html fallback.
func mount(static fs.FS) http.Handler {
	fileServer := http.FileServer(http.FS(static))
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		p := strings.TrimPrefix(r.URL.Path, "/")
		if p == "" {
			p = "index.html"
		}
		if f, err := static.Open(p); err == nil {
			f.Close()
			fileServer.ServeHTTP(w, r)
			return
		}
		r.URL.Path = "/"
		fileServer.ServeHTTP(w, r)
	})
	base := strings.TrimSuffix(*basePath, "/")
	if base == "" || base == "/" {
		return handler
	}
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" {
			http.Redirect(w, r, base+"/", http.StatusFound)
			return
		}
		http.StripPrefix(base, handler).ServeHTTP(w, r)
	})
}

func openBrowser(page string) {
	time.Sleep(300 * time.Millisecond)
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command("rundll32", "url.dll,FileProtocolHandler", page)
	case "darwin":
		cmd = exec.Command("open", page)
	default:
		cmd = exec.Command("xdg-open", page)
	}
	if err := cmd.Start(); err != nil {
		log.Printf("cannot open browser: %v", err)
	}
}
