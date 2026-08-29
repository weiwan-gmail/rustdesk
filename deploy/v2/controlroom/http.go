package controlroom

import (
	"context"
	"net"
	"net/http"
	"strings"
	"time"
)

const pingInterval = 20 * time.Second

// ClientIP prefers X-Real-IP (Caddy / localserver set this) then RemoteAddr.
func ClientIP(r *http.Request) string {
	if x := strings.TrimSpace(r.Header.Get("X-Real-IP")); x != "" {
		return x
	}
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}

// ServeHTTP upgrades GET /control?target= to a control-room WebSocket.
func (h *Hub) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	switch r.URL.Path {
	case "/control", "/control/":
	default:
		http.NotFound(w, r)
		return
	}
	target := r.URL.Query().Get("target")
	if NormalizeTarget(target) == "" {
		http.Error(w, "missing target", http.StatusBadRequest)
		return
	}
	auto := r.URL.Query().Get("autoApprove") == "1"
	ws, err := acceptWS(w, r)
	if err != nil {
		http.Error(w, "websocket required", http.StatusBadRequest)
		return
	}
	defer ws.Close()

	sendCh := make(chan []byte, 32)
	send := func(b []byte) {
		cp := append([]byte(nil), b...)
		select {
		case sendCh <- cp:
		default:
		}
	}
	m := h.Join(target, ClientIP(r), auto, send)
	if m == nil {
		_ = ws.writeFrame(opClose, nil)
		return
	}
	defer h.Leave(m)

	ctx, cancel := context.WithCancel(r.Context())
	defer cancel()

	go func() {
		ticker := time.NewTicker(pingInterval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				if err := ws.writeFrame(opPing, nil); err != nil {
					cancel()
					return
				}
			case b, ok := <-sendCh:
				if !ok {
					return
				}
				if err := ws.writeFrame(opText, b); err != nil {
					cancel()
					return
				}
			}
		}
	}()

	for {
		payload, err := ws.readFrame()
		if err != nil {
			return
		}
		msg, ok := parseClientMsg(payload)
		if !ok {
			continue
		}
		switch msg.Type {
		case "request":
			h.Request(m)
		case "release":
			h.Release(m)
		case "approve":
			h.Approve(m, msg.AutoApprove)
		case "deny":
			h.Deny(m)
		case "setAutoApprove":
			if msg.AutoApprove != nil {
				h.SetAutoApprove(m, *msg.AutoApprove)
			}
		}
	}
}
