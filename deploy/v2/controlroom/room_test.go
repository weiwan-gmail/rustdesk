package controlroom

import (
	"encoding/json"
	"sync"
	"testing"
)

type sink struct {
	mu   sync.Mutex
	last State
	n    int
}

func (s *sink) send(b []byte) {
	var st State
	if err := json.Unmarshal(b, &st); err != nil {
		return
	}
	s.mu.Lock()
	s.last = st
	s.n++
	s.mu.Unlock()
}

func (s *sink) state() State {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.last
}

func TestNormalizeTarget(t *testing.T) {
	cases := []struct {
		a, b string
		same bool
	}{
		{"192.168.1.50", "192.168.1.50:21118", true},
		{"192.168.1.50:21118", "192.168.1.50:21119", true},
		{"10.0.0.1", "10.0.0.2", false},
		{"desk-id-1", "desk-id-1", true},
		{"Desk-ID", "desk-id", true},
		{"  192.168.1.50  ", "192.168.1.50", true},
	}
	for _, c := range cases {
		got := NormalizeTarget(c.a) == NormalizeTarget(c.b)
		if got != c.same {
			t.Fatalf("NormalizeTarget(%q) vs %q same=%v want %v (%q vs %q)",
				c.a, c.b, got, c.same, NormalizeTarget(c.a), NormalizeTarget(c.b))
		}
	}
	if NormalizeTarget("") != "" {
		t.Fatal("empty target")
	}
}

func TestJoinAutoApprove(t *testing.T) {
	h := NewHub(false)
	s := &sink{}
	if h.Join("desk", "10.0.0.1", true, s.send) == nil {
		t.Fatal("join")
	}
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	if sb.state().You != "controller" {
		t.Fatalf("controller autoApprove from join: %+v", sb.state())
	}
}

func TestFirstJoinerIsController(t *testing.T) {
	h := NewHub(false)
	a, sa := join(h, "desk", "1.1.1.1")
	if sa.state().You != "controller" {
		t.Fatalf("first joiner: %+v", sa.state())
	}
	if sa.state().MemberCount != 1 || sa.state().ViewerCount != 0 {
		t.Fatalf("counts: %+v", sa.state())
	}
	b, sb := join(h, "desk", "2.2.2.2")
	if sb.state().You != "viewer" {
		t.Fatalf("second joiner: %+v", sb.state())
	}
	if sa.state().ViewerCount != 1 || sa.state().ControllerIP != "1.1.1.1" {
		t.Fatalf("controller snapshot: %+v", sa.state())
	}
	h.Leave(a)
	h.Leave(b)
}

func TestRequestNeedsApprove(t *testing.T) {
	h := NewHub(false)
	_, sa := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	if sb.state().You != "viewer" || !sb.state().YouRequested {
		t.Fatalf("still viewer pending: %+v", sb.state())
	}
	if sa.state().PendingIP != "10.0.0.2" {
		t.Fatalf("controller pending: %+v", sa.state())
	}
	h.Approve(b, nil) // not controller; ignore
	if sb.state().You != "viewer" {
		t.Fatal("non-controller approve must not transfer")
	}
}

func TestApproveTransfersControl(t *testing.T) {
	h := NewHub(false)
	a, sa := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	yes := true
	h.Approve(a, &yes)
	if sa.state().You != "viewer" {
		t.Fatalf("old controller now viewer: %+v", sa.state())
	}
	if sb.state().You != "controller" {
		t.Fatalf("requester now controller: %+v", sb.state())
	}
	if !a.autoApprove || !sa.state().AutoApprove {
		t.Fatal("approve flag should stick on the member who clicked it")
	}
}

func TestDenyDropsPending(t *testing.T) {
	h := NewHub(false)
	a, sa := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	h.Deny(a)
	if sa.state().PendingIP != "" {
		t.Fatalf("pending cleared: %+v", sa.state())
	}
	if sb.state().YouRequested || sb.state().You != "viewer" {
		t.Fatalf("denied stays viewer: %+v", sb.state())
	}
}

func TestGlobalAutoApprove(t *testing.T) {
	h := NewHub(true)
	_, sa := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	if sb.state().You != "controller" {
		t.Fatalf("global auto-approve: %+v", sb.state())
	}
	if sa.state().You != "viewer" {
		t.Fatalf("previous controller: %+v", sa.state())
	}
	if !sa.state().GlobalAuto {
		t.Fatal("global flag in snapshot")
	}
}

func TestCheckboxThenNextAutoPasses(t *testing.T) {
	h := NewHub(false)
	a, sa := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	yes := true
	h.Approve(a, &yes) // a -> autoApprove, b becomes controller
	if sb.state().You != "controller" || sa.state().You != "viewer" {
		t.Fatalf("after first approve a=%+v b=%+v", sa.state(), sb.state())
	}
	// Restore a as controller (b releases), a still has autoApprove.
	h.Release(b)
	if sa.state().You != "controller" {
		// vacant grants pending; a is not pending. So room is vacant.
		if sa.state().You != "viewer" || sb.state().You != "viewer" {
			t.Fatalf("after release: a=%+v b=%+v", sa.state(), sb.state())
		}
		h.Request(a)
	}
	if sa.state().You != "controller" {
		t.Fatalf("a should be controller: %+v", sa.state())
	}
	c, sc := join(h, "desk", "10.0.0.3")
	h.Request(c)
	if sc.state().You != "controller" {
		t.Fatalf("a auto-approve should pass next request: a=%+v c=%+v", sa.state(), sc.state())
	}
}

func TestVacantRequestGranted(t *testing.T) {
	h := NewHub(false)
	a, _ := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Release(a)
	h.Request(b)
	if sb.state().You != "controller" {
		t.Fatalf("vacant request: %+v", sb.state())
	}
}

func TestControllerLeaveGrantsPending(t *testing.T) {
	h := NewHub(false)
	a, _ := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	h.Leave(a)
	if sb.state().You != "controller" {
		t.Fatalf("pending granted on vacant: %+v", sb.state())
	}
}

func TestSetAutoApproveFlushesPending(t *testing.T) {
	h := NewHub(false)
	a, _ := join(h, "desk", "10.0.0.1")
	b, sb := join(h, "desk", "10.0.0.2")
	h.Request(b)
	h.SetAutoApprove(a, true)
	if sb.state().You != "controller" {
		t.Fatalf("checkbox while pending: %+v", sb.state())
	}
}

func TestSameRoomForIPAndPort(t *testing.T) {
	h := NewHub(false)
	_, sa := join(h, "192.168.1.50", "1.1.1.1")
	_, sb := join(h, "192.168.1.50:21118", "2.2.2.2")
	if sa.state().MemberCount != 2 || sb.state().MemberCount != 2 {
		t.Fatalf("same room: a=%+v b=%+v", sa.state(), sb.state())
	}
}

func TestParseClientMsg(t *testing.T) {
	_, ok := parseClientMsg([]byte(`{"type":"request"}`))
	if !ok {
		t.Fatal("request")
	}
	m, ok := parseClientMsg([]byte(`{"type":"approve","autoApprove":true}`))
	if !ok || m.AutoApprove == nil || !*m.AutoApprove {
		t.Fatalf("approve: %+v ok=%v", m, ok)
	}
	if _, ok := parseClientMsg(nil); ok {
		t.Fatal("empty")
	}
	big := make([]byte, maxJSONBytes+1)
	if _, ok := parseClientMsg(big); ok {
		t.Fatal("too large")
	}
}

func join(h *Hub, target, ip string) (*Member, *sink) {
	s := &sink{}
	m := h.Join(target, ip, false, s.send)
	if m == nil {
		panic("join failed")
	}
	return m, s
}
