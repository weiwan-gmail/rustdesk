/// Fold one-shot `--rendezvous-server` / `--key` command line flags into the peer id
/// as `<id>@<host>?key=<key>`, the shape the client parses into `other_server`.
/// Session-scoped: nothing is written to the config, so the saved
/// `custom-rendezvous-server` / `key` stay untouched.
String composeOneShotServerId(String id, String? server, String? key) {
  var composed = id;
  if (server != null && server.isNotEmpty && !composed.contains('@')) {
    composed = '$composed@$server';
  }
  if (key != null &&
      key.isNotEmpty &&
      composed.contains('@') &&
      !composed.contains('key=')) {
    composed = '$composed?key=$key';
  }
  return composed;
}
