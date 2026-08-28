package main

// Minimal RFC 6455 WebSocket server, pure standard library.
//
// Only what the RustDesk web client needs: binary frames, client frames are
// masked single-frame messages, server frames are sent unmasked and
// unfragmented. Ping is answered with pong, close is acknowledged.

import (
	"bufio"
	"crypto/sha1"
	"encoding/base64"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"sync"
)

const (
	opContinuation = 0
	opText         = 1
	opBinary       = 2
	opClose        = 8
	opPing         = 9
	opPong         = 10
)

const wsGUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

// maxFramePayload caps a single WS/RustDesk frame. Video frames (VP9) are
// typically well under 1 MiB even for keyframes; 8 MiB is generous headroom
// while blocking untrusted 64 MiB allocations.
const maxFramePayload = 8 * 1024 * 1024

// wsConn is a hijacked connection with buffered IO for frame parsing.
// wmu serializes frame writes: the bridge's two goroutines (and the pong
// reply inside readFrame) can all write concurrently, and bufio.Writer is
// not safe for that.
type wsConn struct {
	net.Conn
	r    *bufio.Reader
	w    *bufio.Writer
	wmu  sync.Mutex
	werr error
}

func wsAcceptKey(key string) string {
	h := sha1.New()
	h.Write([]byte(key))
	h.Write([]byte(wsGUID))
	return base64.StdEncoding.EncodeToString(h.Sum(nil))
}

func headerHasToken(h http.Header, name, token string) bool {
	for _, v := range h[http.CanonicalHeaderKey(name)] {
		for _, t := range splitTokens(v) {
			if len(t) == len(token) && equalFoldASCII(t, token) {
				return true
			}
		}
	}
	return false
}

func splitTokens(s string) []string {
	var out []string
	start := 0
	for i := 0; i <= len(s); i++ {
		if i == len(s) || s[i] == ',' {
			t := trimSpace(s[start:i])
			if t != "" {
				out = append(out, t)
			}
			start = i + 1
		}
	}
	return out
}

func trimSpace(s string) string {
	start := 0
	for start < len(s) && (s[start] == ' ' || s[start] == '\t') {
		start++
	}
	end := len(s)
	for end > start && (s[end-1] == ' ' || s[end-1] == '\t') {
		end--
	}
	return s[start:end]
}

// originMatchesHost reports whether the Origin header's host equals the
// request's Host header (same-origin page we served).
func originMatchesHost(origin, host string) bool {
	u, err := url.Parse(origin)
	if err != nil {
		return false
	}
	return equalFoldASCII(u.Host, host)
}

func equalFoldASCII(a, b string) bool {
	for i := 0; i < len(a); i++ {
		ca, cb := a[i], b[i]
		if 'A' <= ca && ca <= 'Z' {
			ca += 32
		}
		if 'A' <= cb && cb <= 'Z' {
			cb += 32
		}
		if ca != cb {
			return false
		}
	}
	return true
}

// acceptWS performs the server-side WebSocket handshake by hijacking the conn.
func acceptWS(w http.ResponseWriter, r *http.Request) (*wsConn, error) {
	if r.Method != http.MethodGet ||
		!headerHasToken(r.Header, "Connection", "upgrade") ||
		!headerHasToken(r.Header, "Upgrade", "websocket") {
		return nil, errors.New("not a websocket handshake")
	}
	key := r.Header.Get("Sec-WebSocket-Key")
	if key == "" {
		return nil, errors.New("missing Sec-WebSocket-Key")
	}
	// Cross-origin WebSocket requests (a malicious page driving a victim's
	// browser against a locally-running bridge) always carry an Origin header;
	// require it to be same-origin with the page we served. Non-browser clients
	// (no Origin) are unaffected.
	if origin := r.Header.Get("Origin"); origin != "" {
		if !originMatchesHost(origin, r.Host) {
			return nil, fmt.Errorf("cross-origin websocket rejected (origin %q, host %q)", origin, r.Host)
		}
	}
	hj, ok := w.(http.Hijacker)
	if !ok {
		return nil, errors.New("response writer does not support hijacking")
	}
	conn, rw, err := hj.Hijack()
	if err != nil {
		return nil, err
	}
	fmt.Fprintf(rw, "HTTP/1.1 101 Switching Protocols\r\n"+
		"Upgrade: websocket\r\n"+
		"Connection: Upgrade\r\n"+
		"Sec-WebSocket-Accept: %s\r\n\r\n", wsAcceptKey(key))
	if err := rw.Flush(); err != nil {
		conn.Close()
		return nil, err
	}
	return &wsConn{Conn: conn, r: rw.Reader, w: rw.Writer}, nil
}

// readFrame reads one data frame. Control frames (ping/pong/close) are handled
// internally; the returned payload belongs to a binary/text data frame.
// Returns io.EOF (or the underlying error) when the connection is closed.
func (c *wsConn) readFrame() ([]byte, error) {
	for {
		fin, opcode, payload, err := c.readRawFrame()
		if err != nil {
			return nil, err
		}
		switch opcode {
		case opPing:
			c.writeFrame(opPong, payload)
			continue
		case opPong:
			continue
		case opClose:
			c.writeFrame(opClose, nil)
			return nil, io.EOF
		case opContinuation:
			// The web client never fragments; treat as data to be safe.
			fallthrough
		case opBinary, opText:
			_ = fin
			return payload, nil
		default:
			return nil, fmt.Errorf("unsupported opcode %d", opcode)
		}
	}
}

func (c *wsConn) readRawFrame() (fin bool, opcode byte, payload []byte, err error) {
	var hdr [2]byte
	if _, err = io.ReadFull(c.r, hdr[:]); err != nil {
		return
	}
	fin = hdr[0]&0x80 != 0
	opcode = hdr[0] & 0x0f
	masked := hdr[1]&0x80 != 0
	length := uint64(hdr[1] & 0x7f)
	if length == 126 {
		var b [2]byte
		if _, err = io.ReadFull(c.r, b[:]); err != nil {
			return
		}
		length = uint64(binary.BigEndian.Uint16(b[:]))
	} else if length == 127 {
		var b [8]byte
		if _, err = io.ReadFull(c.r, b[:]); err != nil {
			return
		}
		length = binary.BigEndian.Uint64(b[:])
	}
	if length > maxFramePayload {
		err = errors.New("frame too large")
		return
	}
	var maskKey [4]byte
	if masked {
		if _, err = io.ReadFull(c.r, maskKey[:]); err != nil {
			return
		}
	}
	payload = make([]byte, length)
	if _, err = io.ReadFull(c.r, payload); err != nil {
		return
	}
	if masked {
		for i := range payload {
			payload[i] ^= maskKey[i%4]
		}
	}
	return
}

// writeFrame sends a single unmasked, unfragmented frame (server→client).
// Safe for concurrent goroutines; the first write error is sticky.
func (c *wsConn) writeFrame(opcode byte, payload []byte) error {
	c.wmu.Lock()
	defer c.wmu.Unlock()
	if c.werr != nil {
		return c.werr
	}
	if err := c.writeFrameLocked(opcode, payload); err != nil {
		c.werr = err
		return err
	}
	return nil
}

func (c *wsConn) writeFrameLocked(opcode byte, payload []byte) error {
	header := []byte{0x80 | opcode}
	n := len(payload)
	switch {
	case n <= 125:
		header = append(header, byte(n))
	case n <= 0xFFFF:
		header = append(header, 126, byte(n>>8), byte(n))
	default:
		header = append(header, 127)
		var b [8]byte
		binary.BigEndian.PutUint64(b[:], uint64(n))
		header = append(header, b[:]...)
	}
	if _, err := c.w.Write(header); err != nil {
		return err
	}
	if _, err := c.w.Write(payload); err != nil {
		return err
	}
	return c.w.Flush()
}
