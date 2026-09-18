use base::direct_access::DirectAccessCli;
use hbb_common::log;

pub use base::direct_access::take_direct_access_args;

pub fn apply_cli_direct_access(cli: DirectAccessCli) {
    if let Some(port) = cli.port {
        log::info!("cli --direct-access-port {port}");
    }
    if let Some(enabled) = cli.enabled {
        log::info!("cli --direct-server {}", if enabled { "Y" } else { "N" });
    }
    base::direct_access::set_cli_direct_access(cli);
}

pub fn print_direct_access_help() {
    println!("  --direct-access-port <n>");
    println!("      Controlled-side direct IP listen port for this process (not persisted).");
    println!("      Implies enable. Not web-direct --direct-port, not --rendezvous-server.");
    println!("  --direct-server [Y|N]");
    println!("      Enable or disable that listen for this process. Bare flag enables.");
    println!();
    println!("Examples:");
    println!("  rustdesk --server --direct-access-port 21119");
    println!("  rustdesk --server --direct-server");
}
