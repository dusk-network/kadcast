#[cfg(test)]
mod tests {

    use std::net::ToSocketAddrs;

    #[test]
    fn resolvetest() {
        let server_details = "192.168.1.5:80";
        let server: Vec<_> = server_details
            .to_socket_addrs()
            .expect("Unable to resolve domain")
            .collect();
        println!("{:?}", server);
    }
}
