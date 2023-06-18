use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

fn main() {
    let address_result = "google.com:80".to_socket_addrs();
    let address = address_result.unwrap().next().unwrap();
    if let Err(error) = TcpStream::connect_timeout(&address, Duration::MAX) {
        println!("{error}");
        std::process::exit(1);
    } else {
        println!("all good");
    }
}
