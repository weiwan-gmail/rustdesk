package controlroom

import (
	"bufio"
	"crypto/rand"
	"encoding/base64"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func TestHTTPMissingTarget(t *testing.T) {
	h := NewHub(false)
	ts := httptest.NewServer(h)
	defer ts.Close()
	res, err := http.Get(ts.URL + "/control")
	if err != nil {
		t.Fatal(err)
	}
	res.Body.Close()
	if res.StatusCode != http.StatusBadRequest {
		t.Fatalf("status %d", res.StatusCode)
	}
}

func TestHTTPNotFound(t *testing.T) {
	h := NewHub(false)
	ts := httptest.NewServer(h)
	defer ts.Close()
	res, err := http.Get(ts.URL + "/other")
	if err != nil {
		t.Fatal(err)
	}
	res.Body.Close()
	if res.StatusCode != http.StatusNotFound {
		t.Fatalf("status %d", res.StatusCode)
	}
}

func TestHTTPCrossOriginRejected(t *testing.T) {
	h := NewHub(false)
	ts := httptest.NewServer(h)
	defer ts.Close()
	req, _ := http.NewRequest(http.MethodGet, ts.URL+"/control?target=desk", nil)
	req.Header.Set("Origin", "http://evil.example")
	req.Header.Set("Connection", "Upgrade")
	req.Header.Set("Upgrade", "websocket")
	req.Header.Set("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
	req.Header.Set("Sec-WebSocket-Version", "13")
	res, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	res.Body.Close()
	if res.StatusCode == http.StatusSwitchingProtocols {
		t.Fatal("evil origin accepted")
	}
}

func TestTwoClientApproveOverWS(t *testing.T) {
	h := NewHub(false)
	ts := httptest.NewServer(h)
	defer ts.Close()

	a := dialControl(t, ts, "desk", "10.0.0.1", false)
	defer a.Close()
	stA := a.mustState(t)
	if stA.You != "controller" {
		t.Fatalf("A: %+v", stA)
	}

	b := dialControl(t, ts, "desk", "10.0.0.2", false)
	defer b.Close()
	stB := b.mustState(t)
	if stB.You != "viewer" {
		t.Fatalf("B: %+v", stB)
	}

	a.mustState(t) // member-count update
	b.sendJSON(t, `{"type":"request"}`)
	stA = waitYou(t, a, "controller")
	if stA.PendingIP != "10.0.0.2" {
		// drain until pending shows
		deadline := time.Now().Add(2 * time.Second)
		for time.Now().Before(deadline) && stA.PendingIP != "10.0.0.2" {
			stA = a.mustState(t)
		}
	}
	if stA.PendingIP != "10.0.0.2" {
		t.Fatalf("pending on A: %+v", stA)
	}
	b.drainUntil(t, func(s State) bool { return s.YouRequested })

	a.sendJSON(t, `{"type":"approve","autoApprove":true}`)
	stB = waitYou(t, b, "controller")
	stA = waitYou(t, a, "viewer")
	if stB.You != "controller" || stA.You != "viewer" {
		t.Fatalf("after approve A=%+v B=%+v", stA, stB)
	}

	c := dialControl(t, ts, "desk", "10.0.0.3", false)
	defer c.Close()
	c.mustState(t)
	c.sendJSON(t, `{"type":"request"}`)
	// A is viewer; B is controller without autoApprove. C should stay pending.
	stC := c.drainUntil(t, func(s State) bool { return s.YouRequested || s.You == "controller" })
	if stC.You == "controller" {
		t.Fatal("C must not steal control without B's approval")
	}
}

func TestGlobalAutoApproveOverWS(t *testing.T) {
	h := NewHub(true)
	ts := httptest.NewServer(h)
	defer ts.Close()
	a := dialControl(t, ts, "desk", "1.1.1.1", false)
	defer a.Close()
	a.mustState(t)
	b := dialControl(t, ts, "desk", "2.2.2.2", false)
	defer b.Close()
	b.mustState(t)
	b.sendJSON(t, `{"type":"request"}`)
	if waitYou(t, b, "controller").You != "controller" {
		t.Fatal("global auto")
	}
}

func TestVacantRequestOverWS(t *testing.T) {
	h := NewHub(false)
	ts := httptest.NewServer(h)
	defer ts.Close()
	a := dialControl(t, ts, "desk", "1.1.1.1", false)
	defer a.Close()
	a.mustState(t)
	b := dialControl(t, ts, "desk", "2.2.2.2", false)
	defer b.Close()
	b.mustState(t)
	a.sendJSON(t, `{"type":"release"}`)
	waitYou(t, a, "viewer")
	b.sendJSON(t, `{"type":"request"}`)
	if waitYou(t, b, "controller").You != "controller" {
		t.Fatal("vacant grant")
	}
}

func TestClientIP(t *testing.T) {
	r, _ := http.NewRequest(http.MethodGet, "http://x/control", nil)
	r.RemoteAddr = "9.9.9.9:1234"
	if ClientIP(r) != "9.9.9.9" {
		t.Fatal(ClientIP(r))
	}
	r.Header.Set("X-Real-IP", "8.8.8.8")
	if ClientIP(r) != "8.8.8.8" {
		t.Fatal(ClientIP(r))
	}
}

type wsClient struct {
	net.Conn
	r *bufio.Reader
}

func dialControl(t *testing.T, ts *httptest.Server, target, ip string, auto bool) *wsClient {
	t.Helper()
	u := strings.TrimPrefix(ts.URL, "http://")
	conn, err := net.Dial("tcp", u)
	if err != nil {
		t.Fatal(err)
	}
	key := make([]byte, 16)
	if _, err := rand.Read(key); err != nil {
		t.Fatal(err)
	}
	q := "/control?target=" + target
	if auto {
		q += "&autoApprove=1"
	}
	fmt.Fprintf(conn, "GET %s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\nX-Real-IP: %s\r\n\r\n",
		q, u, base64.StdEncoding.EncodeToString(key), ip)
	c := &wsClient{Conn: conn, r: bufio.NewReader(conn)}
	line, err := c.r.ReadString('\n')
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(line, "101") {
		t.Fatalf("handshake: %s", line)
	}
	for {
		l, err := c.r.ReadString('\n')
		if err != nil {
			t.Fatal(err)
		}
		if l == "\r\n" {
			break
		}
	}
	return c
}

func (c *wsClient) sendJSON(t *testing.T, s string) {
	t.Helper()
	payload := []byte(s)
	mask := []byte{1, 2, 3, 4}
	header := []byte{0x81, byte(0x80 | len(payload))}
	masked := make([]byte, len(payload))
	for i := range payload {
		masked[i] = payload[i] ^ mask[i%4]
	}
	if _, err := c.Write(header); err != nil {
		t.Fatal(err)
	}
	if _, err := c.Write(mask); err != nil {
		t.Fatal(err)
	}
	if _, err := c.Write(masked); err != nil {
		t.Fatal(err)
	}
}

func (c *wsClient) readState(t *testing.T) (State, error) {
	t.Helper()
	_ = c.SetReadDeadline(time.Now().Add(2 * time.Second))
	for {
		var hdr [2]byte
		if _, err := io.ReadFull(c.r, hdr[:]); err != nil {
			return State{}, err
		}
		opcode := hdr[0] & 0x0f
		length := uint64(hdr[1] & 0x7f)
		if length == 126 {
			var b [2]byte
			if _, err := io.ReadFull(c.r, b[:]); err != nil {
				return State{}, err
			}
			length = uint64(binary.BigEndian.Uint16(b[:]))
		}
		payload := make([]byte, length)
		if _, err := io.ReadFull(c.r, payload); err != nil {
			return State{}, err
		}
		switch opcode {
		case opPing:
			continue
		case opPong, opClose:
			continue
		case opText, opBinary:
			var st State
			if err := json.Unmarshal(payload, &st); err != nil {
				return State{}, err
			}
			return st, nil
		}
	}
}

func (c *wsClient) mustState(t *testing.T) State {
	t.Helper()
	st, err := c.readState(t)
	if err != nil {
		t.Fatal(err)
	}
	return st
}

func (c *wsClient) drainUntil(t *testing.T, ok func(State) bool) State {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	var last State
	for time.Now().Before(deadline) {
		st, err := c.readState(t)
		if err != nil {
			t.Fatalf("read: %v last=%+v", err, last)
		}
		last = st
		if ok(st) {
			return st
		}
	}
	t.Fatalf("timeout last=%+v", last)
	return last
}

func waitYou(t *testing.T, c *wsClient, you string) State {
	t.Helper()
	return c.drainUntil(t, func(s State) bool { return s.You == you })
}
