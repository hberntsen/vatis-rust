#[macro_use] extern crate log;
extern crate tokio;
extern crate paho_mqtt as mqtt;
extern crate futures;

use std::{env, process};
use std::string::String;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;
use env_logger::Env;
use linux_stats;
use tokio::time::{self, Duration};
use tokio::signal::unix::{signal, SignalKind};
use mqtt::Client;
use futures::executor::block_on;


#[cfg(target_os="linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Initialize the logger from the environment
    env_logger::from_env(Env::default().default_filter_or("warn")).init();

    // Create a client & define connect options
    let host = env::args().nth(1).unwrap_or_else( ||
        "tcp://localhost:1883".to_string()
    );

    let cli = connect_mqtt(host);

    // Get interval
    let interval = env::args().nth(2).unwrap_or_else( ||
        "10".to_string()
    ).parse::<u64>().unwrap_or_else( |_|
        10
    );

    let mut timer = time::interval(Duration::from_secs(interval));
    info!("timer with interval {}s started", interval);

    let mac_address = get_mac();

    // Create streams for SIGINT, SIGTERM signals.
    let mut sigint_stream = signal(SignalKind::interrupt())?;
    let mut sigterm_stream = signal(SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = timer.tick() => {
                send_stats(&cli, &mac_address);
            },
            _ = sigint_stream.recv() => {
                debug!("SIGINT received");
                break;
            },
            _ = sigterm_stream.recv() => {
                debug!("SIGTERM received");
                break;
            },
        };
    };

    // Disconnect from the broker
    info!("disconnecting...");
    cli.disconnect(None).unwrap();

    info!("exited");
    Ok(())
}


// Alternative main fn for none-linux
#[cfg(not(target_os="linux"))]
fn main() {
    panic!("unfortunately vatis only works on linux");
}


// Send the statistics to the appropriate MQTT topic
fn send_stats(cli: &mqtt::Client, mac: &String) {

    // Asynchronously send memory statistics
    let mem_future = send_mem_stats(cli, mac);
    let tcp_future = send_tcp_stats(cli, mac);


    // Wait until everything is sent..
    block_on(mem_future);
    block_on(tcp_future);
    debug!("stats published");
}


// Returns a new connected MQTT client or exits when it fails
fn connect_mqtt(host: String) -> Client {

    info!("Creating MQTT connection to {}", host);

    let mut cli = mqtt::Client::new(host).unwrap_or_else( |e| {
        error!("error creating the client: {:?}", e);
        process::exit(1);
    });

    // Use 5sec timeouts for sync calls.
    cli.set_timeout(Duration::from_secs(3));

    info!("MQTT client created");

    // Connect and wait for it to complete or fail
    if let Err(e) = cli.connect(None) {
        error!("error connecting to MQTT server: {:?}", e);
        process::exit(1);
    }

    info!("MQTT connection established");

    cli
}


// Get the MAC address of the first interface that is not lo
fn get_mac() -> String {

    let net = Path::new("/sys/class/net");
    let entry = fs::read_dir(net).expect("error reading /sys/class/net");

    let mut ifaces = entry.filter_map(|p| p.ok()).map( |p| {
        p.path()
        .file_name()
        .expect("error getting interface name")
        .to_os_string()
    }).filter_map( |s| {
        s.into_string().ok()
    }).collect::<Vec<String>>();

    debug!("available interfaces: {:?}", ifaces);

    // filter lo from list
    ifaces.retain(|x| x != "lo");

    // read /sys/class/net/<iface>/address
    let iface = net.join(ifaces[0].as_str()).join("address");
    let mut f = fs::File::open(iface).expect("error reading address");
    let mut macaddr = String::new();
    f.read_to_string(&mut macaddr).expect("error reading address");
    info!("MAC address: {}", macaddr);

    macaddr
}


// Sends a metric value to mqtt
fn send(cli: &mqtt::Client, mac: &String, metric: String, ts: u128, mvalue: String) {
    let msg = mqtt::MessageBuilder::new()
        .topic(format!("metrics/{}/{}", mac, metric))
        .payload(format!("{};{}", ts, mvalue))
        .qos(0)
        .finalize();

    if let Err(e) = cli.publish(msg) {
        warn!("error sending message: {:?}", e);
    }
}


// Takes all memory statistics, and sends them to mqtt
async fn send_mem_stats(cli: &mqtt::Client, mac: &String) {
    // Get system time in Unix Nano
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();

    // let stat = linux_stats::stat().unwrap();
    let mem_info = linux_stats::meminfo().unwrap();
    // let tcp_stat = linux_stats::tcp().unwrap();

    send(cli, mac, String::from("memory/total"), now, format!("{}", mem_info.mem_total));
    send(cli, mac, String::from("memory/free"), now, format!("{}", mem_info.mem_free));
    send(cli, mac, String::from("memory/available"), now, format!("{}", mem_info.mem_available));
    send(cli, mac, String::from("memory/buffers"), now, format!("{}", mem_info.bufers));
    send(cli, mac, String::from("memory/cached"), now, format!("{}", mem_info.cached));
    send(cli, mac, String::from("memory/swap/cached"), now, format!("{}", mem_info.swap_cached));
    send(cli, mac, String::from("memory/active"), now, format!("{}", mem_info.active));
    send(cli, mac, String::from("memory/active/anon"), now, format!("{}", mem_info.active_anon));
    send(cli, mac, String::from("memory/active/file"), now, format!("{}", mem_info.active_file));
    send(cli, mac, String::from("memory/inactive"), now, format!("{}", mem_info.inactive));
    send(cli, mac, String::from("memory/inactive/anon"), now, format!("{}", mem_info.inactive_anon));
    send(cli, mac, String::from("memory/inactive/file"), now, format!("{}", mem_info.inactive_file));
    send(cli, mac, String::from("memory/mlocked"), now, format!("{}", mem_info.mlocked));
    send(cli, mac, String::from("memory/unevictable"), now, format!("{}", mem_info.unevictable));
    send(cli, mac, String::from("memory/swap/total"), now, format!("{}", mem_info.swap_total));
    send(cli, mac, String::from("memory/swap/free"), now, format!("{}", mem_info.swap_free));
    send(cli, mac, String::from("memory/dirty"), now, format!("{}", mem_info.dirty));
    send(cli, mac, String::from("memory/writeback"), now, format!("{}", mem_info.writeback));
    send(cli, mac, String::from("memory/anon-pages"), now, format!("{}", mem_info.anon_pages));
    send(cli, mac, String::from("memory/mapped"), now, format!("{}", mem_info.mapped));
    send(cli, mac, String::from("memory/shmem"), now, format!("{}", mem_info.shmem));
    send(cli, mac, String::from("memory/sreclaimable"), now, format!("{}", mem_info.s_reclaimable));
    send(cli, mac, String::from("memory/sunreclaim"), now, format!("{}", mem_info.s_unreclaim));
    send(cli, mac, String::from("memory/slab"), now, format!("{}", mem_info.slab));
    send(cli, mac, String::from("memory/kernelstack"), now, format!("{}", mem_info.kernel_stack));
    send(cli, mac, String::from("memory/pagetables"), now, format!("{}", mem_info.page_tables));
    send(cli, mac, String::from("memory/nfs-unstable"), now, format!("{}", mem_info.nfs_unstable));
    send(cli, mac, String::from("memory/bounce"), now, format!("{}", mem_info.bounce));
    send(cli, mac, String::from("memory/writebacktmp"), now, format!("{}", mem_info.writeback_tmp));
    send(cli, mac, String::from("memory/commitlimit"), now, format!("{}", mem_info.commit_limit));
    send(cli, mac, String::from("memory/committed-as"), now, format!("{}", mem_info.committed_as));
    send(cli, mac, String::from("memory/vmalloc/total"), now, format!("{}", mem_info.vmalloc_total));
    send(cli, mac, String::from("memory/vmalloc/used"), now, format!("{}", mem_info.vmalloc_used));
    send(cli, mac, String::from("memory/vmalloc/chunk"), now, format!("{}", mem_info.vmalloc_chunk));
    send(cli, mac, String::from("memory/hardware-corrupted"), now, format!("{}", mem_info.hardware_corrupted));
    send(cli, mac, String::from("memory/hugepages/anon"), now, format!("{}", mem_info.anon_huge_pages));
    send(cli, mac, String::from("memory/hugepages/total"), now, format!("{}", mem_info.huge_pages_total));
    send(cli, mac, String::from("memory/hugepages/free"), now, format!("{}", mem_info.huge_pages_free));
    send(cli, mac, String::from("memory/hugepages/surp"), now, format!("{}", mem_info.huge_pages_surp));
    send(cli, mac, String::from("memory/hugepages/rsvd"), now, format!("{}", mem_info.huge_pages_rsvd));
    send(cli, mac, String::from("memory/hugepagesize"), now, format!("{}", mem_info.hugepagesize));
    send(cli, mac, String::from("memory/cma/total"), now, format!("{}", mem_info.cma_total));
    send(cli, mac, String::from("memory/cma/free"), now, format!("{}", mem_info.cma_free));
}

// Takes all memory statistics, and sends them to mqtt
async fn send_tcp_stats(cli: &mqtt::Client, mac: &String) {
    // Get system time in Unix Nano
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();

    let tcp_stat = linux_stats::tcp().unwrap();

    for s in &tcp_stat {
        send(cli, mac, format!("tcp/{}/ipv4address", s.sl), now, s.local_address.to_string());

    };
}
