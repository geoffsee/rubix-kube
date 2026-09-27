// Parser fixtures for Semgrep, not compiled workspace source.
fn tls(client: Client) {
    // ruleid: rubix-invalid-tls-certificates
    client.danger_accept_invalid_certs(true);
    // ok: rubix-invalid-tls-certificates
    client.danger_accept_invalid_certs(false);
    // ruleid: rubix-invalid-tls-hostnames
    client.danger_accept_invalid_hostnames(true);
    // ok: rubix-invalid-tls-hostnames
    client.danger_accept_invalid_hostnames(false);
    // ruleid: rubix-native-tls-verification
    client.set_verify(openssl::ssl::SslVerifyMode::NONE);
    // ok: rubix-native-tls-verification
    client.set_verify(openssl::ssl::SslVerifyMode::PEER);
    // ruleid: rubix-native-tls-verification
    client.ssl_verify_peer(false);
    // ruleid: rubix-native-tls-verification
    client.ssl_verify_host(false);
    // ok: rubix-native-tls-verification
    client.ssl_verify_peer(true);
    // ok: rubix-native-tls-verification
    client.ssl_verify_host(true);
}

fn hashes(data: &[u8]) {
    // ruleid: rubix-weak-hash
    let _ = md5::compute(data);
    // ruleid: rubix-weak-hash
    let _ = md5::Md5::new();
    // ruleid: rubix-weak-hash
    let _ = sha1::Sha1::new();
    // ruleid: rubix-weak-hash
    let _ = sha1::Sha1::digest(data);
    // ok: rubix-weak-hash
    let _ = sha2::Sha256::digest(data);
}

fn commands(input: &str) {
    // ruleid: rubix-formatted-shell-command
    std::process::Command::new("sh").arg("-c").arg(format!("echo {}", input));
    // ruleid: rubix-formatted-shell-command
    std::process::Command::new("/bin/bash").arg("-c").arg(format!("echo {}", input));
    // ok: rubix-formatted-shell-command
    std::process::Command::new("echo").arg(input);
    // ok: rubix-formatted-shell-command
    std::process::Command::new("sh").arg("-c").arg("echo fixed");
}
