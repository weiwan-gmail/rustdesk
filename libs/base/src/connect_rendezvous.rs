/// Fold one-shot `--rendezvous-server` / `--key` command line flags into the
/// peer id as `<id>@<host>?key=<key>`, the shape the client parses into
/// `LoginConfigHandler::other_server`. Returns the id plus the `key` uni link query
/// parameter the flutter side folds back onto the id. Nothing is persisted, so the
/// saved `custom-rendezvous-server` / `key` stay untouched.
pub fn compose_other_server(
    id: String,
    other_server: Option<String>,
    other_key: Option<String>,
) -> (String, Option<String>) {
    let mut id = id;
    if let Some(server) = other_server.filter(|server| !server.is_empty()) {
        if !id.contains('@') {
            id = format!("{id}@{server}");
        }
    }
    let other_key = other_key.filter(|key| !key.is_empty());
    match other_key {
        // Without a host in the id there is no `other_server` to carry the key, and
        // the key of the configured server can not be overridden here.
        Some(key) if id.contains('@') && !id.contains("key=") => {
            let key_param = format!("key={}", encode_query_value(&key));
            (id, Some(key_param))
        }
        _ => (id, None),
    }
}

/// Percent-encode an uni link query value. Keys are base64 and `+` (as well as `&`,
/// `=`, `%`) would otherwise be mangled: the flutter side reads the value back with
/// `Uri.queryParameters`.
fn encode_query_value(value: &str) -> String {
    const UNRESERVED: &[u8] = b"-._~";
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || UNRESERVED.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_other_server_builds_the_id_shape_the_client_parses() {
        let (id, key) = compose_other_server(
            "1234".to_owned(),
            Some("hbbs.example.com".to_owned()),
            Some("5Qbws+de3/unUcJBtr=x9Zkv&UmwFNoExHzpryHuPUdqlWM=".to_owned()),
        );
        assert_eq!(id, "1234@hbbs.example.com");
        assert_eq!(
            key,
            Some("key=5Qbws%2Bde3%2FunUcJBtr%3Dx9Zkv%26UmwFNoExHzpryHuPUdqlWM%3D".to_owned())
        );

        // an id that already carries a host keeps it, the key is still added
        let (id, key) = compose_other_server(
            "1234@other:21116".to_owned(),
            Some("ignored.example.com".to_owned()),
            Some("abc".to_owned()),
        );
        assert_eq!(id, "1234@other:21116");
        assert_eq!(key, Some("key=abc".to_owned()));

        // without a host there is nothing to carry the key
        for server in [None, Some("".to_owned())] {
            let (id, key) = compose_other_server("1234".to_owned(), server, Some("abc".to_owned()));
            assert_eq!(id, "1234");
            assert_eq!(key, None);
        }

        let (id, key) = compose_other_server("1234".to_owned(), None, None);
        assert_eq!(id, "1234");
        assert_eq!(key, None);

        // an id that already carries key= keeps it
        let (id, key) = compose_other_server(
            "1234@hbbs.example.com?key=keep".to_owned(),
            Some("ignored.example.com".to_owned()),
            Some("new".to_owned()),
        );
        assert_eq!(id, "1234@hbbs.example.com?key=keep");
        assert_eq!(key, None);
    }

    #[test]
    fn compose_other_server_uni_link_matches_connect_usage() {
        let (id, key) = compose_other_server(
            "123456789".to_owned(),
            Some("hbbs.example.com".to_owned()),
            Some("5Qbws+de3/unUcJBtr=x9Zkv&UmwFNoExHzpryHuPUdqlWM=".to_owned()),
        );
        let uni = format!("rustdesk://connect/{id}?{}", key.expect("key query"));
        assert_eq!(
            uni,
            "rustdesk://connect/123456789@hbbs.example.com?key=5Qbws%2Bde3%2FunUcJBtr%3Dx9Zkv%26UmwFNoExHzpryHuPUdqlWM%3D"
        );
    }
}
