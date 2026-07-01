use env_logger::Env;

use cpu_bars::client::Client;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let client = Client::connect().expect("couldn't start and connect");
    client.read();
    println!("'{}'", client.read());
}
