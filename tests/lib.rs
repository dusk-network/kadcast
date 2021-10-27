// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

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
