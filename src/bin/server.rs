use env_logger::Env;
use std::thread;

use cpu_bars::server::Server;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let producer = thread::spawn(move || {
        let mut server = Server::new().expect("couldn't open server");
        server.run();
    });

    producer.join().expect("error returned by server closing");
    log::info!("server joined");
}
