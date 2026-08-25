// rustdesk-web-direct is a self-contained web server for the RustDesk web
// client in DIRECT mode: the browser connects to a controlled RustDesk client
// by IP, with no hbbs/hbbr server needed (similar to websockify for noVNC).
//
// It serves the embedded web client and bridges /direct WebSocket connections
// to the controlled client's direct-access TCP port.
//
// Pure standard library, no third-party dependencies.
//
//	./rustdesk-web-direct --listen :8081 --open
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
)

//go:embed all:static
var staticFS embed.FS

var (
	listen     = flag.String("listen", ":8081", "address the web page is served on")
	directPort = flag.Int("direct-port", 21118, "the only allowed target port (the controlled client's direct-access port)")
	allowCIDR  = flag.String("allow-cidr", "", "comma-separated CIDRs allowed as direct targets (default: loopback/private/link-local only)")
	allowAny   = flag.Bool("allow-any", false, "disable target IP restrictions (DANGEROUS: the proxy can then reach arbitrary hosts)")
	basePath   = flag.String("base-path", "/", "URL path the client is mounted under (must match the build's BASE_HREF)")
	tlsCert    = flag.String("tls-cert", "", "TLS certificate file; plain HTTP when empty (fine for intranet/localhost)")
	tlsKey     = flag.String("tls-key", "", "TLS key file")
	open       = flag.Bool("open", false, "open the page in the system browser after start")
)

var allowedNets []*net.IPNet

func main() {
	flag.Parse()
	var err error
	allowedNets, err = buildAllowedNets(*allowCIDR, *allowAny)
	if err != nil {
		log.Fatalf("invalid --allow-cidr: %v", err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/direct", handleDirect)

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

// handleDirect bridges /direct?target=IP:PORT to the controlled client's
// direct-access TCP port.
func handleDirect(w http.ResponseWriter, r *http.Request) {
	target := r.URL.Query().Get("target")
	addr, err := validateTarget(target)
	if err != nil {
		http.Error(w, "invalid target: "+err.Error(), http.StatusForbidden)
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

// validateTarget enforces IP-literal host, the single allowed direct port,
// and the IP allowlist - this is what stops the proxy from becoming an open
// TCP relay (SSRF).
func validateTarget(target string) (string, error) {
	if target == "" {
		return "", fmt.Errorf("missing target")
	}
	host, portStr, err := net.SplitHostPort(target)
	if err != nil {
		return "", fmt.Errorf("target must be IP:port")
	}
	port, err := strconv.Atoi(portStr)
	if err != nil || port <= 0 || port > 65535 {
		return "", fmt.Errorf("invalid port")
	}
	if port != *directPort {
		return "", fmt.Errorf("only port %d is allowed", *directPort)
	}
	ip := net.ParseIP(strings.Trim(host, "[]"))
	if ip == nil {
		return "", fmt.Errorf("host must be an IP literal")
	}
	if !ipAllowed(ip) {
		return "", fmt.Errorf("target %s not allowed", ip)
	}
	return net.JoinHostPort(host, portStr), nil
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
