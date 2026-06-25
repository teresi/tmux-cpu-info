use env_logger::Env;
use std::thread;
use std::time::Duration;

use cpu_bars::client::Client;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let consumer = thread::spawn(move || {
        let client = Client::connect().expect("couldn't start and connect");
        loop {
            client.read();
            thread::sleep(Duration::from_millis(500));
        }
    });

    consumer.join().expect("error returned by client closing");
    log::info!("client joined");
}
