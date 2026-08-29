package controlroom

// Minimal RFC 6455 WebSocket server, standard library only.
// Text and binary frames, unfragmented. Ping is answered with pong.

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
	"strings"
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

// Control-room frames are small JSON; keep this well under the video-bridge cap.
const maxFramePayload = 16 * 1024

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
			if strings.EqualFold(t, token) {
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
			t := strings.TrimSpace(s[start:i])
			if t != "" {
				out = append(out, t)
			}
			start = i + 1
		}
	}
	return out
}

func originMatchesHost(origin, host string) bool {
	u, err := url.Parse(origin)
	if err != nil || u.Host == "" {
		return false
	}
	return strings.EqualFold(u.Host, host)
}

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

func (c *wsConn) readFrame() ([]byte, error) {
	for {
		_, opcode, payload, err := c.readRawFrame()
		if err != nil {
			return nil, err
		}
		switch opcode {
		case opPing:
			_ = c.writeFrame(opPong, payload)
			continue
		case opPong:
			continue
		case opClose:
			_ = c.writeFrame(opClose, nil)
			return nil, io.EOF
		case opContinuation, opBinary, opText:
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
		length = uint64(binary.BigEndian.Uint64(b[:]))
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
	_ = fin
	return
}

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
