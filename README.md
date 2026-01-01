# ddns6 - IPv6 Prefix DynDNS Daemon

A Rust-based DynDNS daemon that solves the problem of dynamic IPv6 prefix assignments by combining them with static Interface IDs.

## The Problem

Modern Internet Service Providers often no longer provide static IPv4 addresses, instead using CGNAT or DS-Lite. However, they do provide IPv6 prefixes that are accessible from the internet. Since these prefixes can change dynamically, a DynDNS service is needed.

The challenge: Each device on your network has its own Interface ID (the suffix of the IPv6 address). Traditionally, you would need to run a separate DynDNS client on every device you want to expose to the internet.

## The Solution

ddns6 provides a centralized solution:
- Accepts standard DynDNS2 protocol requests from any client
- Extracts the dynamic IPv6 prefix from the client's request
- Combines it with pre-configured static Interface IDs for each device
- Updates Cloudflare DNS records with the complete IPv6 addresses
- Smart caching: only updates DNS when the prefix actually changes

## Features

- Standard DynDNS2 HTTP protocol (compatible with routers and existing clients)
- Cloudflare API integration
- Smart state caching (avoids unnecessary API calls)
- Graceful shutdown handling (SIGTERM, SIGINT)
- Comprehensive logging with tracing
- TOML configuration (designed for future web interface support)
- Minimal resource usage
- Production-ready error handling

## Architecture

```
Client (Router/Device) → ddns6 Daemon → Cloudflare API
   ↓
[Sends IPv6: 2001:db8:1234:5678::1]
                    ↓
         [Extracts Prefix: 2001:db8:1234:5678::]
         [Looks up Interface ID: ::1]
         [Combines: 2001:db8:1234:5678::1]
         [Updates DNS if changed]
```

## Installation

### Prerequisites

- Rust 1.70 or later
- A Cloudflare account with a domain
- IPv6 connectivity

### Building from Source

```bash
git clone <repository-url>
cd ddns6
cargo build --release
```

The binary will be available at `target/release/ddns6`.

## Configuration

1. Copy the example configuration:
```bash
cp config.example.toml config.toml
```

2. Edit `config.toml` with your settings:

```toml
[server]
bind_address = "0.0.0.0:8080"

[cloudflare]
api_token = "your-cloudflare-api-token"
zone_id = "your-zone-id"
ttl = 300

[[hosts]]
hostname = "device1.example.com"
interface_id = "::1"

[[hosts]]
hostname = "device2.example.com"
interface_id = "::2"
```

### Getting Cloudflare Credentials

1. **API Token**:
   - Go to https://dash.cloudflare.com/profile/api-tokens
   - Click "Create Token"
   - Use "Edit zone DNS" template
   - Select your zone
   - Required permissions: `Zone.DNS (Edit)`

2. **Zone ID**:
   - Go to https://dash.cloudflare.com/
   - Select your domain
   - Find Zone ID in the right sidebar under "API"

### Finding Interface IDs

On your devices, find the Interface ID (last 64 bits of the IPv6 address):

**Linux:**
```bash
ip -6 addr show scope global
# Look for addresses like 2001:db8:1234:5678::a1b2:c3d4:e5f6:7890
# Interface ID is: ::a1b2:c3d4:e5f6:7890
```

**Windows:**
```powershell
ipconfig
# Look for IPv6 addresses
```

You can specify Interface IDs in various formats:
- `::1` - Simple notation
- `1` - Short form
- `::a1b2:c3d4:e5f6:7890` - Full notation
- `a1b2:c3d4:e5f6:7890` - Without leading `::`

## Usage

### Running the Daemon

```bash
# Using default config.toml
./ddns6

# Specifying config file
./ddns6 --config /path/to/config.toml

# With debug logging
RUST_LOG=debug ./ddns6
```

### Configuring Clients

Configure your router or client to send updates to the daemon. Since the IPv6 prefix changes for ALL devices on your network simultaneously, the daemon automatically updates all configured hosts with the new prefix.

**URL Format:**
```
http://your-server-ip:8080/update?prefix=<current-ipv6>
```

**Example with curl:**
```bash
curl "http://localhost:8080/update?prefix=2001:db8:1234:5678::1"
```

The daemon will:
1. Extract the IPv6 prefix from your address
2. Combine it with each configured Interface ID
3. Update ALL hostnames in your config.toml with their new addresses

**Response Codes:**
- `good device1.example.com=2001:db8::1, device2.example.com=2001:db8::2` - All hosts updated successfully
- `nochg device1.example.com=2001:db8::1, device2.example.com=2001:db8::2` - Prefix hasn't changed, no updates needed
- `partial success: device1.example.com=2001:db8::1 | failed: device2.example.com` - Some hosts updated, some failed
- `911 <error>` - Server error

### Systemd Service

Create `/etc/systemd/system/ddns6.service`:

```ini
[Unit]
Description=ddns6 IPv6 DynDNS Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=ddns6
Group=ddns6
ExecStart=/usr/local/bin/ddns6 --config /etc/ddns6/config.toml
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable ddns6
sudo systemctl start ddns6
sudo systemctl status ddns6
```

## How It Works

1. **Client Update Request**: Your router/device sends its current IPv6 address to the daemon
   ```
   GET /update?prefix=2001:db8:1234:5678::1
   ```

2. **Prefix Extraction**: ddns6 extracts the /64 prefix from the address
   ```
   2001:db8:1234:5678::
   ```

3. **Batch Update**: For EACH configured host, the daemon:
   - Looks up the Interface ID from config
     ```
     device1.example.com → ::1
     device2.example.com → ::2
     nas.example.com → ::100
     ```

   - Combines prefix with each Interface ID
     ```
     2001:db8:1234:5678:: + ::1 = 2001:db8:1234:5678::1
     2001:db8:1234:5678:: + ::2 = 2001:db8:1234:5678::2
     2001:db8:1234:5678:: + ::100 = 2001:db8:1234:5678::100
     ```

   - Checks if the address changed (smart caching)
   - Updates Cloudflare if changed, skips if unchanged

4. **Response**: Returns summary of all updates
   - `good` - Lists all successfully updated hosts
   - `nochg` - Lists all unchanged hosts
   - `partial success` - Lists successes and failures separately

## Logging

Control log level with the `RUST_LOG` environment variable:

```bash
# Info level (default)
RUST_LOG=info ./ddns6

# Debug level (detailed information)
RUST_LOG=debug ./ddns6

# Trace level (very verbose)
RUST_LOG=trace ./ddns6

# Module-specific logging
RUST_LOG=ddns6::cloudflare=debug,info ./ddns6
```

## Security Considerations

- **Network-level Authentication**: This daemon trusts all incoming requests. Deploy behind a firewall or reverse proxy with authentication.
- **HTTPS**: Consider using a reverse proxy (nginx, Caddy) for TLS encryption in production.
- **API Token Security**: Keep your `config.toml` secure. Never commit it to version control.
- **Firewall**: Restrict access to the daemon's port to trusted networks only.

## Troubleshooting

### Daemon won't start
```bash
# Check configuration syntax
./ddns6 --config config.toml

# Enable debug logging
RUST_LOG=debug ./ddns6
```

### DNS not updating
1. Verify Cloudflare credentials
2. Check that hostname exists in Cloudflare (or daemon will create it)
3. Ensure Interface ID format is correct
4. Check logs: `RUST_LOG=debug ./ddns6`

### Client not connecting
1. Check firewall rules
2. Verify bind_address in config
3. Test with curl: `curl http://localhost:8080/`

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test ipv6

# Run with logging
RUST_LOG=debug cargo test
```

### Project Structure

```
ddns6/
├── src/
│   ├── main.rs          # Entry point and daemon setup
│   ├── config.rs        # Configuration management
│   ├── error.rs         # Error types
│   ├── http.rs          # HTTP server setup
│   ├── dyndns2.rs       # DynDNS2 protocol handler
│   ├── ipv6.rs          # IPv6 prefix/address handling
│   ├── state.rs         # State cache
│   └── cloudflare.rs    # Cloudflare API client
├── Cargo.toml
├── config.example.toml
└── README.md
```

## Future Enhancements

- [ ] Web-based configuration interface
- [ ] Support for additional DNS providers (via plugin system)
- [ ] Prometheus metrics endpoint
- [ ] Rate limiting per hostname
- [ ] Optional HTTP Basic Auth
- [ ] Docker image
- [ ] Support for /48 and /56 prefixes
- [ ] IPv4 passthrough support

## License

MIT OR Apache-2.0

## Contributing

Contributions welcome! Please open an issue or submit a pull request.

## Acknowledgments

Built with:
- [axum](https://github.com/tokio-rs/axum) - Web framework
- [tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [tracing](https://github.com/tokio-rs/tracing) - Logging
