# pistonproxy

**pistonproxy** is a reverse proxy for Minecraft servers.  
Its main purpose is to handle the `server_addr` from the Minecraft handshake packet and proxy connections to different backend servers based on this address—similar to how virtual hosts work for web servers.

## Features

- **Virtual Host Routing:** Proxies incoming Minecraft connections to different backend servers depending on the hostname (`server_addr`) used by the client.
- **Port Scanner Protection:** Can silently close connections without replying unless the correct hostname is used, helping to protect backend servers from port scanners and unwanted connections.

## Use Cases

- Host multiple Minecraft servers behind a single IP and port, routing players based on the address they connect to.
- Add an extra layer of security by hiding backend servers from unauthorized or automated scans.

## Getting Started

1. **Clone the repository:**
   ```sh
   git clone https://github.com/martin640/pistonproxy
   ```

2. **Build the project:**
   ```sh
   cargo build --release
   ```

3. **Create config.yaml**

   ```yaml
   settings:
     cache_size: 2048
     handshake_timeout: 8 # how long in milliseconds to wait for handshake from client
     client_buffer_size: 4096 # size of buffer for incoming data from client
     client_packets_limit: 3 # maximum number of packets to accept from client before handshake is complete
     backend_buffer_size: 4096 # size of buffer for incoming data from backend
     ratelimit_window: 1000
     ratelimit: 4 # maximum number of connections from a single IP in given ratelimit_window
     concurrent_limit: 5 # maximum number of concurrent active connections from a single IP
     clients_limit: 100 # maximum number of total active connections
     listen: 25565 # port to listen on
     log: DEBUG
     log_inspect_buffer_limit: 1024

   endpoints:
     - hostname: server1.example.com
       origin: 192.168.0.100:25565 # address of backend server
       motd: Epic server 1
     - hostname: server2.example.com
       motd: Epic server 2
       message: Sorry this server is not available # when user tries to join, disconnect them with this message

   blocklist: # list of IP addresses to block - socket will be closed immediately
     - 192.168.1.1
   ```

4. **Run the proxy:**
   ```sh
   cargo run --release
   ```

## License

MIT License. See [LICENSE](LICENSE.txt) for details.

