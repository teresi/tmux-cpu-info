use std::thread;
use std::time::Duration;

use env_logger::Env;

use cpu_bars::client::Client;
use cpu_bars::server::Server;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let producer = thread::spawn(move || {
        let mut server = Server::new().expect("couldn't open server");
        server.run();
    });

    let consumer = thread::spawn(move || {
        let client = Client::start_and_connect().expect("couldn't start and connect");
        loop {
            client.read();
            thread::sleep(Duration::from_secs(1));
        }
    });

    consumer.join().unwrap();
    log::info!("client joined");
    //producer.join();
    //log::info!("server joined");
}
