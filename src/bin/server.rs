//use env_logger::Env;

use cpu_bars::server::Server;

fn main() {
    //env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    //let mut server = Server::new().expect("couldn't open server");
    //server.run();
    Server::start().expect("couldn't start daemon");
}
