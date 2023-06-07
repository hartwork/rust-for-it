// This file is part of the rust-for-it project.
//
// Copyright (c) 2023 Sebastian Pipping <sebastian@pipping.org>
// SPDX-License-Identifier: MIT

use std::io;
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::result::Result;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub type TimeoutSeconds = u64;

fn resolve_address(host_and_port: &str, timeout: Duration) -> Result<SocketAddr, std::io::Error> {
    let timer = Instant::now();
    loop {
        let address_result = host_and_port.to_socket_addrs();
        match address_result {
            Ok(_) => {
                let address = address_result.unwrap().next().unwrap();
                return Ok(address);
            }
            Err(_) => {
                if timer.elapsed() >= timeout {
                    return Err(address_result.err().unwrap());
                }
            }
        }
        sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
#[test]
fn test_resolve_address_for_valid() {
    use std::net::{IpAddr, Ipv4Addr};
    let expected_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 631);
    assert_eq!(
        resolve_address("127.0.0.1:631", Duration::from_secs(1)).unwrap(),
        expected_address
    );
}

#[cfg(test)]
#[test]
fn test_resolve_address_for_invalid() {
    assert!(resolve_address("not valid syntax", Duration::from_secs(1)).is_err());
}

fn wait_for_tcp_socket(host_and_port: &str, timeout: Duration) -> Result<(), std::io::Error> {
    let timer = Instant::now();
    let address = resolve_address(host_and_port, timeout)?;
    loop {
        let timeout_left = timeout.saturating_sub(timer.elapsed());
        if timeout_left.is_zero() {
            let error = io::Error::new(io::ErrorKind::TimedOut, "Time is up");
            return Err(error);
        }

        // NOTE: This distinction is mainly for Windows where
        //       TcpStream::connect_timeout([..], Duration::MAX)
        //       seems to never return even when the target is available.
        //       Seems like a bug.
        let connect_res = if timeout == Duration::MAX {
            TcpStream::connect(&address)
        } else {
            TcpStream::connect_timeout(&address, timeout_left)
        };

        match connect_res {
            Ok(connection) => {
                let _ = connection.shutdown(Shutdown::Both);
                return Ok(());
            }
            Err(error) => {
                if timer.elapsed() >= timeout {
                    return Err(error);
                }
            }
        }
        sleep(Duration::from_millis(500));
    }
}

pub fn wait_for_service(
    host_and_port: &str,
    timeout_seconds: TimeoutSeconds,
    verbose: bool,
) -> Result<(), std::io::Error> {
    let timer = Instant::now();
    let forever = timeout_seconds == 0;

    if verbose {
        if forever {
            println!("[*] Waiting for {host_and_port} without a timeout...");
        } else {
            println!("[*] Waiting {timeout_seconds} seconds for {host_and_port}...");
        }
    }

    let timeout = if timeout_seconds == 0 {
        Duration::MAX
    } else {
        Duration::from_secs(timeout_seconds)
    };

    let connect_result = wait_for_tcp_socket(host_and_port, timeout);

    if verbose {
        match connect_result {
            Ok(_) => {
                let duration = timer.elapsed().as_secs();
                println!("[+] {host_and_port} is available after {duration} seconds.");
            }
            Err(ref error) => {
                println!(
                    "[-] {host_and_port} timed out after waiting for {timeout_seconds} seconds ({error})."
                );
            }
        }
    }

    connect_result
}
