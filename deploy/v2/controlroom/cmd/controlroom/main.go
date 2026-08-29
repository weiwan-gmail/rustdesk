package main

import (
	"flag"
	"fmt"
	"log"
	"net/http"
	"strings"

	"rustdesk-web-controlroom"
)

var demoHTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>RustDesk control room demo</title>
<style>
  body { font-family: sans-serif; background: #111; color: #eee; margin: 16px; }
  h1 { font-size: 18px; }
  .row { display: flex; gap: 12px; flex-wrap: wrap; }
  .col { flex: 1; min-width: 240px; background: #1c1c1c; padding: 12px; border-radius: 8px; }
  button, input { margin: 4px 4px 4px 0; }
  pre { font-size: 12px; white-space: pre-wrap; }
  .bar {
    pointer-events: none; display: inline-flex; align-items: center; gap: 8px;
    background: rgba(0,0,0,0.45); color: #fff; border-radius: 16px; padding: 6px 12px;
    font-size: 13px;
  }
  .bar button, .bar label { pointer-events: auto; }
</style>
</head>
<body>
<h1>Control room (target shared)</h1>
<p>First connect becomes controller. Others are viewers until the controller approves.</p>
<div class="row" id="cols"></div>
<script>
const target = "demo-desk";
const names = ["A", "B", "C"];
function col(name) {
  const d = document.createElement("div");
  d.className = "col";
  d.innerHTML = "<h2>Client " + name + "</h2>" +
    "<button data-act=connect>Connect</button>" +
    "<button data-act=request>Request</button>" +
    "<button data-act=approve>Approve</button>" +
    "<button data-act=deny>Deny</button>" +
    "<button data-act=release>Release</button>" +
    "<label><input type=checkbox data-act=auto> Auto-approve next</label>" +
    "<div class=bar id=bar-" + name + "></div>" +
    "<pre id=st-" + name + "></pre>";
  let ws;
  const st = d.querySelector("pre");
  const bar = d.querySelector(".bar");
  const auto = d.querySelector("[data-act=auto]");
  function render(s) {
    st.textContent = JSON.stringify(s, null, 2);
    let html = s.you === "controller" ? "Controlling" : "Viewing";
    html += " · " + (s.viewerCount || 0) + " watching";
    if (s.controllerIp) html += " · ctrl " + s.controllerIp;
    if (s.you === "viewer") {
      html += s.youRequested
        ? " · waiting"
        : " <button data-bar=request>Request control</button>";
    } else {
      html += " <button data-bar=release>Release</button>";
      if (s.pendingIp) {
        html += " · " + s.pendingIp +
          " <button data-bar=approve>Approve</button>" +
          " <button data-bar=deny>Deny</button>";
      }
    }
    bar.innerHTML = html;
  }
  function send(obj) { if (ws && ws.readyState === 1) ws.send(JSON.stringify(obj)); }
  d.addEventListener("click", (e) => {
    const act = e.target.getAttribute("data-act") || e.target.getAttribute("data-bar");
    if (act === "connect") {
      if (ws) ws.close();
      const u = (location.protocol === "https:" ? "wss:" : "ws:") + "//" + location.host +
        "/control?target=" + encodeURIComponent(target) + (auto.checked ? "&autoApprove=1" : "");
      ws = new WebSocket(u);
      ws.onmessage = (ev) => { try { render(JSON.parse(ev.data)); } catch (e) {} };
      ws.onclose = () => { st.textContent = "disconnected"; };
    }
    if (act === "request" || act === "approve" || act === "deny" || act === "release") {
      const body = { type: act };
      if (act === "approve") body.autoApprove = auto.checked;
      send(body);
    }
  });
  auto.addEventListener("change", () => send({ type: "setAutoApprove", autoApprove: auto.checked }));
  document.getElementById("cols").appendChild(d);
}
names.forEach(col);
</script>
</body>
</html>
`

func main() {
	listen := flag.String("listen", ":8099", "listen address")
	auto := flag.Bool("auto-approve", false, "approve every control request immediately")
	demo := flag.Bool("demo", false, "serve a two/three-client demo page at /")
	flag.Parse()

	h := controlroom.NewHub(*auto)
	mux := http.NewServeMux()
	mux.Handle("/control", h)
	mux.Handle("/control/", h)
	if *demo {
		mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			fmt.Fprint(w, demoHTML)
		})
	}
	log.Printf("control room on %s (auto-approve=%v demo=%v)", *listen, *auto, *demo)
	if strings.HasPrefix(*listen, ":") {
		log.Printf("  ws://127.0.0.1%s/control?target=<id>", *listen)
	}
	log.Fatal(http.ListenAndServe(*listen, mux))
}
