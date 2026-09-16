package main

// Bridge between the browser WebSocket and the controlled peer's direct-access
// TCP port. The RustDesk TCP protocol frames each protobuf message with a
// variable-length little-endian header (libs/hbb_common/src/bytes_codec.rs):
//
//	header = (payload_len << 2) | (header_len - 1)   (header_len: 1..4 bytes)
//
// while the WebSocket side carries one protobuf message per binary frame. So
// the bridge adds/parses that framing; it never touches the protobuf payload.

import (
	"errors"
	"io"
	"net"
	"sync"
)

// Cap one RustDesk frame; see maxFramePayload in websocket.go.
const maxRDFrame = maxFramePayload

// writeRDFrame writes one payload with the RustDesk length header.
func writeRDFrame(w io.Writer, payload []byte) error {
	n := len(payload)
	switch {
	case n <= 0x3F:
		if _, err := w.Write([]byte{byte(n << 2)}); err != nil {
			return err
		}
	case n <= 0x3FFF:
		h := uint16(n<<2) | 0x1
		if _, err := w.Write([]byte{byte(h), byte(h >> 8)}); err != nil {
			return err
		}
	case n <= 0x3FFFFF:
		h := uint32(n<<2) | 0x2
		if _, err := w.Write([]byte{byte(h), byte(h >> 8), byte(h >> 16)}); err != nil {
			return err
		}
	case n <= 0x3FFFFFFF:
		h := uint32(n<<2) | 0x3
		if _, err := w.Write([]byte{byte(h), byte(h >> 8), byte(h >> 16), byte(h >> 24)}); err != nil {
			return err
		}
	default:
		return errors.New("frame too large")
	}
	_, err := w.Write(payload)
	return err
}

// readRDFrame reads one RustDesk-framed payload from the TCP stream.
func readRDFrame(r io.Reader) ([]byte, error) {
	var b0 [1]byte
	if _, err := io.ReadFull(r, b0[:]); err != nil {
		return nil, err
	}
	headLen := int(b0[0]&0x3) + 1
	head := []byte{b0[0]}
	if headLen > 1 {
		rest := make([]byte, headLen-1)
		if _, err := io.ReadFull(r, rest); err != nil {
			return nil, err
		}
		head = append(head, rest...)
	}
	var v uint32
	for i := 0; i < headLen; i++ {
		v |= uint32(head[i]) << (8 * i)
	}
	n := int(v >> 2)
	if n < 0 || n > maxRDFrame {
		return nil, errors.New("invalid frame length")
	}
	payload := make([]byte, n)
	if _, err := io.ReadFull(r, payload); err != nil {
		return nil, err
	}
	return payload, nil
}

// bridge pumps bytes both ways until either side closes.
func bridge(ws *wsConn, tcp net.Conn) {
	var wg sync.WaitGroup
	wg.Add(2)

	// browser -> controlled: WS binary frame -> add RustDesk header -> TCP
	go func() {
		defer wg.Done()
		for {
			payload, err := ws.readFrame()
			if err != nil {
				break
			}
			if err := writeRDFrame(tcp, payload); err != nil {
				break
			}
		}
		tcp.Close()
	}()

	// controlled -> browser: parse RustDesk frame -> one WS binary frame
	go func() {
		defer wg.Done()
		for {
			payload, err := readRDFrame(tcp)
			if err != nil {
				break
			}
			if err := ws.writeFrame(opBinary, payload); err != nil {
				break
			}
		}
		ws.Close()
	}()

	wg.Wait()
}
