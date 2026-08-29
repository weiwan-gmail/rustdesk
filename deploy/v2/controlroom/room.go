package controlroom

import (
	"encoding/json"
	"net"
	"strings"
	"sync"
)

const (
	maxRooms     = 256
	maxMembers   = 32
	maxTargetLen = 256
	maxPending   = 16
	maxJSONBytes = 4096
)

// State is the JSON snapshot pushed to every member after a room change.
type State struct {
	Type         string `json:"type"`
	You          string `json:"you"`
	ControllerIP string `json:"controllerIp"`
	ViewerCount  int    `json:"viewerCount"`
	MemberCount  int    `json:"memberCount"`
	PendingIP    string `json:"pendingIp"`
	YouRequested bool   `json:"youRequested"`
	AutoApprove  bool   `json:"autoApprove"`
	GlobalAuto   bool   `json:"globalAutoApprove"`
}

// Member is one control-room participant (one browser tab).
type Member struct {
	id          int
	ip          string
	autoApprove bool
	send        func([]byte)
	room        *Room
}

func (m *Member) ID() int    { return m.id }
func (m *Member) IP() string { return m.ip }

type Room struct {
	key        string
	members    map[int]*Member
	controller *Member
	pending    []*Member
}

// Hub holds all rooms for one process.
type Hub struct {
	mu                sync.Mutex
	rooms             map[string]*Room
	memberRoom        map[int]*Room
	nextID            int
	globalAutoApprove bool
}

func NewHub(globalAutoApprove bool) *Hub {
	return &Hub{
		rooms:             make(map[string]*Room),
		memberRoom:        make(map[int]*Room),
		globalAutoApprove: globalAutoApprove,
	}
}

func (h *Hub) GlobalAutoApprove() bool { return h.globalAutoApprove }

// NormalizeTarget maps 192.168.1.50 and 192.168.1.50:21118 to the same key.
func NormalizeTarget(s string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		return ""
	}
	if len(s) > maxTargetLen {
		s = s[:maxTargetLen]
	}
	if strings.HasPrefix(s, "[") {
		host, _, err := net.SplitHostPort(s)
		if err == nil {
			if ip := net.ParseIP(strings.Trim(host, "[]")); ip != nil {
				return canonicalIP(ip)
			}
		}
	}
	if host, _, err := net.SplitHostPort(s); err == nil {
		if ip := net.ParseIP(host); ip != nil {
			return canonicalIP(ip)
		}
		return strings.ToLower(s)
	}
	if ip := net.ParseIP(s); ip != nil {
		return canonicalIP(ip)
	}
	return strings.ToLower(s)
}

func canonicalIP(ip net.IP) string {
	if v4 := ip.To4(); v4 != nil {
		return v4.String()
	}
	return ip.String()
}

// Join adds a member. The first member in an empty room becomes controller.
func (h *Hub) Join(target, ip string, autoApprove bool, send func([]byte)) *Member {
	key := NormalizeTarget(target)
	if key == "" || send == nil {
		return nil
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	if len(h.rooms) >= maxRooms && h.rooms[key] == nil {
		return nil
	}
	r := h.rooms[key]
	if r == nil {
		r = &Room{key: key, members: make(map[int]*Member)}
		h.rooms[key] = r
	}
	if len(r.members) >= maxMembers {
		return nil
	}
	h.nextID++
	m := &Member{id: h.nextID, ip: ip, autoApprove: autoApprove, send: send, room: r}
	r.members[m.id] = m
	h.memberRoom[m.id] = r
	if r.controller == nil && len(r.members) == 1 {
		r.controller = m
	}
	h.broadcastLocked(r)
	return m
}

func (h *Hub) Leave(m *Member) {
	if m == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[m.id]
	if r == nil {
		return
	}
	h.removeLocked(r, m)
}

func (h *Hub) Request(m *Member) {
	if m == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[m.id]
	if r == nil {
		return
	}
	if r.controller == m {
		return
	}
	if r.containsPending(m) {
		h.broadcastLocked(r)
		return
	}
	if r.controller == nil {
		r.controller = m
		h.broadcastLocked(r)
		return
	}
	if h.shouldAutoApprove(r) {
		r.controller = m
		r.clearPending(m)
		h.broadcastLocked(r)
		return
	}
	if len(r.pending) >= maxPending {
		return
	}
	r.pending = append(r.pending, m)
	h.broadcastLocked(r)
}

func (h *Hub) Release(m *Member) {
	if m == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[m.id]
	if r == nil || r.controller != m {
		return
	}
	r.controller = nil
	h.grantNextIfVacantLocked(r)
	h.broadcastLocked(r)
}

func (h *Hub) Approve(controller *Member, autoApprove *bool) {
	if controller == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[controller.id]
	if r == nil || r.controller != controller {
		return
	}
	if autoApprove != nil {
		controller.autoApprove = *autoApprove
	}
	if len(r.pending) == 0 {
		h.broadcastLocked(r)
		return
	}
	next := r.pending[0]
	r.pending = r.pending[1:]
	if r.members[next.id] == nil {
		h.grantNextIfVacantLocked(r)
		h.broadcastLocked(r)
		return
	}
	r.controller = next
	h.broadcastLocked(r)
}

func (h *Hub) Deny(controller *Member) {
	if controller == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[controller.id]
	if r == nil || r.controller != controller || len(r.pending) == 0 {
		return
	}
	r.pending = r.pending[1:]
	h.broadcastLocked(r)
}

func (h *Hub) SetAutoApprove(m *Member, v bool) {
	if m == nil {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[m.id]
	if r == nil {
		return
	}
	m.autoApprove = v
	if v && r.controller == m && len(r.pending) > 0 {
		next := r.pending[0]
		r.pending = r.pending[1:]
		if r.members[next.id] != nil {
			r.controller = next
		}
	}
	h.broadcastLocked(r)
}

func (h *Hub) Snapshot(m *Member) State {
	h.mu.Lock()
	defer h.mu.Unlock()
	r := h.memberRoom[m.id]
	if r == nil {
		return State{Type: "state", You: "viewer"}
	}
	return h.stateLocked(r, m)
}

func (h *Hub) shouldAutoApprove(r *Room) bool {
	if h.globalAutoApprove {
		return true
	}
	return r.controller != nil && r.controller.autoApprove
}

func (h *Hub) grantNextIfVacantLocked(r *Room) {
	if r.controller != nil {
		return
	}
	for len(r.pending) > 0 {
		next := r.pending[0]
		r.pending = r.pending[1:]
		if r.members[next.id] != nil {
			r.controller = next
			return
		}
	}
}

func (h *Hub) removeLocked(r *Room, m *Member) {
	delete(r.members, m.id)
	delete(h.memberRoom, m.id)
	r.clearPending(m)
	if r.controller == m {
		r.controller = nil
		h.grantNextIfVacantLocked(r)
	}
	if len(r.members) == 0 {
		delete(h.rooms, r.key)
		return
	}
	h.broadcastLocked(r)
}

func (r *Room) containsPending(m *Member) bool {
	for _, p := range r.pending {
		if p == m {
			return true
		}
	}
	return false
}

func (r *Room) clearPending(m *Member) {
	out := r.pending[:0]
	for _, p := range r.pending {
		if p != m {
			out = append(out, p)
		}
	}
	r.pending = out
}

func (h *Hub) stateLocked(r *Room, m *Member) State {
	you := "viewer"
	if r.controller == m {
		you = "controller"
	}
	ctrlIP := ""
	if r.controller != nil {
		ctrlIP = r.controller.ip
	}
	pendingIP := ""
	youRequested := false
	for i, p := range r.pending {
		if p == m {
			youRequested = true
		}
		if i == 0 {
			pendingIP = p.ip
		}
	}
	viewerCount := len(r.members)
	if r.controller != nil {
		viewerCount--
	}
	if viewerCount < 0 {
		viewerCount = 0
	}
	return State{
		Type:         "state",
		You:          you,
		ControllerIP: ctrlIP,
		ViewerCount:  viewerCount,
		MemberCount:  len(r.members),
		PendingIP:    pendingIP,
		YouRequested: youRequested,
		AutoApprove:  m.autoApprove,
		GlobalAuto:   h.globalAutoApprove,
	}
}

func (h *Hub) broadcastLocked(r *Room) {
	type job struct {
		send func([]byte)
		body []byte
	}
	jobs := make([]job, 0, len(r.members))
	for _, m := range r.members {
		body, err := json.Marshal(h.stateLocked(r, m))
		if err != nil {
			continue
		}
		jobs = append(jobs, job{m.send, body})
	}
	for _, j := range jobs {
		j.send(j.body)
	}
}

type clientMsg struct {
	Type        string `json:"type"`
	AutoApprove *bool  `json:"autoApprove"`
}

func parseClientMsg(b []byte) (clientMsg, bool) {
	var m clientMsg
	if len(b) == 0 || len(b) > maxJSONBytes {
		return m, false
	}
	if err := json.Unmarshal(b, &m); err != nil {
		return m, false
	}
	return m, m.Type != ""
}
