// Synthetic fixed-path executables for disposable chroot tests, not package/service oracles.
use std::{fs, io::Write, path::Path, time::Duration};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = Path::new(&args[0]).file_name().unwrap().to_str().unwrap();
    let mut log = fs::OpenOptions::new().create(true).append(true).open("/actions").unwrap();
    writeln!(log, "{}", args.join(" ")).unwrap();
    let mode = fs::read_to_string("/mode").unwrap_or_default();
    if mode == "hold" {
        fs::write("/active-pid.pending", std::process::id().to_string()).unwrap();
        fs::rename("/active-pid.pending", "/active-pid").unwrap();
        std::thread::sleep(Duration::from_secs(90));
    }
    if mode == "fail-update" && name == "rc-update" { std::process::exit(7); }
    if mode == "noop" { return; }
    if name == "apk" {
        for package in &args[3..] {
            let tool = match package.as_str() { "nftables" => "nft", "iptables" => "iptables", _ => panic!("unexpected package") };
            fs::write(format!("/sbin/{tool}"), "fixture").unwrap();
        }
    } else if name == "rc-service" {
        fs::write("/sys/fs/cgroup/cgroup.controllers", "cpuset cpu io memory pids\n").unwrap();
    }
}
