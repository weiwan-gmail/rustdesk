// rustdesk-web-v2 is a self-contained web server for the RustDesk v2 web
// client: it serves the embedded Flutter web build (current flutter/ tree)
// and transparently proxies the WebSocket endpoints (/ws/id, /ws/relay) to a
// RustDesk server (hbbs/hbbr), similar to how novnc_proxy fronts a VNC server.
//
// This server is version-agnostic delivery infrastructure, shared with the v1
// server-mode localserver (deploy/v1/web/localserver); only the embedded
// static client differs.
//
// Pure standard library, no third-party dependencies.
//
//	./rustdesk-web-v2 --server 192.168.1.10 --listen :8080 --open
package main

import (
	"embed"
	"flag"
	"fmt"
	"io/fs"
	"log"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"time"
)

//go:embed all:static
var staticFS embed.FS

const (
	defaultRendezvousPort = 21116
	wsIDPath              = "/ws/id"
	wsRelayPath           = "/ws/relay"
)

var (
	listen   = flag.String("listen", ":8080", "address the web page is served on")
	server   = flag.String("server", "localhost", "RustDesk server host[:port] (default port 21116); WS ports are derived as port+2 / port+3")
	wsID     = flag.String("ws-id", "", "explicit upstream for "+wsIDPath+" (e.g. http://host:21118), overrides --server derivation")
	wsRelay  = flag.String("ws-relay", "", "explicit upstream for "+wsRelayPath+" (e.g. http://host:21119), overrides --server derivation")
	basePath = flag.String("base-path", "/", "URL path the client is mounted under (must match the build's BASE_HREF)")
	tlsCert  = flag.String("tls-cert", "", "TLS certificate file; plain HTTP when empty (fine for intranet/localhost)")
	tlsKey   = flag.String("tls-key", "", "TLS key file")
	open     = flag.Bool("open", false, "open the page in the system browser after start")
)

func main() {
	flag.Parse()

	idUpstream := *wsID
	if idUpstream == "" {
		idUpstream = wsUpstream(*server, false)
	}
	relayUpstream := *wsRelay
	if relayUpstream == "" {
		relayUpstream = wsUpstream(*server, true)
	}

	mux := http.NewServeMux()
	mux.Handle(wsIDPath, wsProxy(idUpstream))
	mux.Handle(wsIDPath+"/", wsProxy(idUpstream))
	mux.Handle(wsRelayPath, wsProxy(relayUpstream))
	mux.Handle(wsRelayPath+"/", wsProxy(relayUpstream))
	mux.HandleFunc("/config.js", serveConfig)

	static, err := fs.Sub(staticFS, "static")
	if err != nil {
		log.Fatal(err)
	}
	mux.Handle("/", mount(static))

	page := fmt.Sprintf("http://localhost%s%s", displayPort(*listen), *basePath)
	log.Printf("RustDesk web client: %s", page)
	log.Printf("upstream: %s -> %s , %s -> %s", wsIDPath, idUpstream, wsRelayPath, relayUpstream)
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

// wsUpstream mirrors the address rules of the web client's getrUriFromRs():
//
//	"" / "host" (domain) -> http://host           (path /ws/id kept by proxy)
//	"domain:21116"       -> http://domain:21118
//	"1.2.3.4"            -> http://1.2.3.4:21118
//	"1.2.3.4:21116"      -> http://1.2.3.4:21118
//
// relay uses port+3 from the rendezvous port.
func wsUpstream(server string, relay bool) string {
	if server == "" {
		server = "localhost"
	}
	host, portStr, err := net.SplitHostPort(server)
	if err != nil {
		host, portStr = server, ""
	}
	port, _ := strconv.Atoi(portStr)
	// "localhost" resolves to a loopback IP; treat it as IP-like so the WS
	// ports are derived (21118/21119) instead of assuming a reverse proxy.
	isIP := net.ParseIP(strings.Trim(host, "[]")) != nil || host == "localhost"
	if !isIP && port == 0 {
		// domain without port: reverse proxy on 80/443 with /ws/* paths;
		// the incoming path is preserved by wsProxy.
		return "http://" + host
	}
	base := port
	if base == 0 {
		base = defaultRendezvousPort
	}
	offset := 2
	if relay {
		offset = 3
	}
	return "http://" + net.JoinHostPort(host, strconv.Itoa(base+offset))
}

// wsProxy transparently forwards WebSocket (and plain HTTP) requests to the
// upstream. httputil.ReverseProxy handles the Upgrade handshake natively.
func wsProxy(upstream string) http.Handler {
	target, err := url.Parse(upstream)
	if err != nil {
		log.Fatalf("invalid upstream %q: %v", upstream, err)
	}
	proxy := &httputil.ReverseProxy{
		Director: func(req *http.Request) {
			req.URL.Scheme = target.Scheme
			req.URL.Host = target.Host
			if host, _, err := net.SplitHostPort(req.RemoteAddr); err == nil {
				req.Header.Set("X-Real-IP", host)
			}
		},
		ErrorHandler: func(w http.ResponseWriter, _ *http.Request, err error) {
			log.Printf("proxy %s: %v", target, err)
			http.Error(w, "Bad Gateway", http.StatusBadGateway)
		},
	}
	return proxy
}

func serveConfig(w http.ResponseWriter, _ *http.Request) {
	w.Header().Set("Content-Type", "application/javascript")
	// The client talks to the same origin; this binary forwards /ws/* to the
	// configured upstream, so no server address is baked into the page.
	fmt.Fprintf(w, "window.RUSTDESK_CONFIG = {server: \"\", wsIdPath: %q, wsRelayPath: %q};\n",
		wsIDPath, wsRelayPath)
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
