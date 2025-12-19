//! UPnP NAT Traversal for P2P networking
//!
//! Provides:
//! - UPnP port mapping for incoming connections
//! - External IP address discovery
//! - Automatic port mapping renewal

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::sleep;

// =============================================================================
// Constants
// =============================================================================

/// Default port mapping lease duration (1 hour)
const PORT_MAPPING_LEASE: u32 = 3600;

/// How often to renew port mapping (50 minutes)
const RENEWAL_INTERVAL: Duration = Duration::from_secs(50 * 60);

/// Description for port mapping
const MAPPING_DESCRIPTION: &str = "mini-blockchain";

// =============================================================================
// Errors
// =============================================================================

#[derive(Error, Debug)]
pub enum UpnpError {
    #[error("UPnP gateway not found")]
    GatewayNotFound,
    #[error("Failed to get external IP: {0}")]
    ExternalIpError(String),
    #[error("Failed to add port mapping: {0}")]
    PortMappingError(String),
    #[error("Failed to remove port mapping: {0}")]
    RemoveMappingError(String),
    #[error("UPnP not supported")]
    NotSupported,
    #[error("Invalid local address")]
    InvalidLocalAddress,
}

// =============================================================================
// UPnP Manager
// =============================================================================

/// UPnP NAT traversal manager
///
/// Uses igd-next crate for UPnP operations, running them in blocking tasks
/// to avoid async compatibility issues.
pub struct UpnpManager {
    /// Our local IP address
    local_ip: Option<Ipv4Addr>,
    /// Our external IP address
    external_ip: Arc<RwLock<Option<Ipv4Addr>>>,
    /// Port we've mapped
    mapped_port: Arc<RwLock<Option<u16>>>,
    /// Whether UPnP is enabled
    enabled: bool,
}

impl UpnpManager {
    /// Create a new UPnP manager
    pub fn new() -> Self {
        Self {
            local_ip: None,
            external_ip: Arc::new(RwLock::new(None)),
            mapped_port: Arc::new(RwLock::new(None)),
            enabled: true,
        }
    }

    /// Create a disabled UPnP manager
    pub fn disabled() -> Self {
        Self {
            local_ip: None,
            external_ip: Arc::new(RwLock::new(None)),
            mapped_port: Arc::new(RwLock::new(None)),
            enabled: false,
        }
    }

    /// Check if UPnP is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the external IP address
    pub async fn external_ip(&self) -> Option<Ipv4Addr> {
        *self.external_ip.read().await
    }

    /// Get the mapped port
    pub async fn mapped_port(&self) -> Option<u16> {
        *self.mapped_port.read().await
    }

    /// Discover gateway and setup port mapping
    pub async fn setup(&mut self, local_port: u16) -> Result<(), UpnpError> {
        if !self.enabled {
            return Err(UpnpError::NotSupported);
        }

        log::info!("Searching for UPnP gateway...");

        // Get local IP first
        let local_ip = get_local_ip().ok_or(UpnpError::InvalidLocalAddress)?;
        self.local_ip = Some(local_ip);

        let local_addr = SocketAddr::V4(SocketAddrV4::new(local_ip, local_port));
        let external_ip = self.external_ip.clone();
        let mapped_port = self.mapped_port.clone();

        // Run UPnP operations in a blocking task
        let result = tokio::task::spawn_blocking(move || {
            use igd_next::{search_gateway, PortMappingProtocol};

            // Search for gateway
            let gateway =
                search_gateway(Default::default()).map_err(|e| UpnpError::GatewayNotFound)?;

            log::info!("Found UPnP gateway: {}", gateway.addr);

            // Get external IP
            let ext_ip = gateway
                .get_external_ip()
                .map_err(|e| UpnpError::ExternalIpError(e.to_string()))?;

            // Extract IPv4
            let ext_ip_v4 = match ext_ip {
                std::net::IpAddr::V4(ip) => ip,
                std::net::IpAddr::V6(_) => {
                    return Err(UpnpError::ExternalIpError("IPv6 not supported".to_string()))
                }
            };

            log::info!("External IP: {}", ext_ip_v4);

            // Add port mapping
            gateway
                .add_port(
                    PortMappingProtocol::TCP,
                    local_port,
                    local_addr,
                    PORT_MAPPING_LEASE,
                    MAPPING_DESCRIPTION,
                )
                .map_err(|e| UpnpError::PortMappingError(e.to_string()))?;

            log::info!(
                "Added UPnP port mapping: {}:{} -> {}",
                ext_ip_v4,
                local_port,
                local_addr
            );

            Ok::<Ipv4Addr, UpnpError>(ext_ip_v4)
        })
        .await
        .map_err(|_| UpnpError::GatewayNotFound)??;

        *external_ip.write().await = Some(result);
        *mapped_port.write().await = Some(local_port);

        Ok(())
    }

    /// Remove the port mapping
    pub async fn cleanup(&mut self) -> Result<(), UpnpError> {
        let port = *self.mapped_port.read().await;

        if let Some(port) = port {
            let result = tokio::task::spawn_blocking(move || {
                use igd_next::{search_gateway, PortMappingProtocol};

                if let Ok(gateway) = search_gateway(Default::default()) {
                    let _ = gateway.remove_port(PortMappingProtocol::TCP, port);
                    log::info!("Removed UPnP port mapping for port {}", port);
                }
                Ok::<(), UpnpError>(())
            })
            .await;

            *self.mapped_port.write().await = None;
        }

        Ok(())
    }

    /// Start a background task that renews the port mapping periodically
    pub fn start_renewal_task(&self) -> tokio::task::JoinHandle<()> {
        let mapped_port = self.mapped_port.clone();
        let local_ip = self.local_ip;

        tokio::spawn(async move {
            loop {
                sleep(RENEWAL_INTERVAL).await;

                let port = *mapped_port.read().await;

                if let (Some(port), Some(ip)) = (port, local_ip) {
                    let local_addr = SocketAddr::V4(SocketAddrV4::new(ip, port));

                    let _ = tokio::task::spawn_blocking(move || {
                        use igd_next::{search_gateway, PortMappingProtocol};

                        if let Ok(gw) = search_gateway(Default::default()) {
                            match gw.add_port(
                                PortMappingProtocol::TCP,
                                port,
                                local_addr,
                                PORT_MAPPING_LEASE,
                                MAPPING_DESCRIPTION,
                            ) {
                                Ok(()) => log::debug!("Renewed UPnP port mapping"),
                                Err(e) => log::warn!("Failed to renew UPnP port mapping: {}", e),
                            }
                        }
                    })
                    .await;
                } else {
                    break;
                }
            }
        })
    }

    /// Get UPnP status
    pub async fn status(&self) -> UpnpStatus {
        UpnpStatus {
            enabled: self.enabled,
            gateway_found: self.mapped_port.read().await.is_some(),
            external_ip: *self.external_ip.read().await,
            mapped_port: *self.mapped_port.read().await,
        }
    }
}

impl Default for UpnpManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Helper Types & Functions
// =============================================================================

/// UPnP status information
#[derive(Debug, Clone)]
pub struct UpnpStatus {
    pub enabled: bool,
    pub gateway_found: bool,
    pub external_ip: Option<Ipv4Addr>,
    pub mapped_port: Option<u16>,
}

/// Get local IP address by connecting to a remote address
pub fn get_local_ip() -> Option<Ipv4Addr> {
    // Connect to a public DNS server to determine our local IP
    // This doesn't actually send any data, just determines the route
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:53").ok()?;

    let addr = socket.local_addr().ok()?;
    match addr.ip() {
        IpAddr::V4(ip) => Some(ip),
        _ => None,
    }
}

/// Detect external IP using public APIs (fallback when UPnP fails)
pub async fn detect_external_ip() -> Option<Ipv4Addr> {
    // Try multiple services for reliability
    let services = ["http://ifconfig.me/ip", "http://icanhazip.com"];

    for service in services {
        log::debug!("Trying to get external IP from {}", service);

        match reqwest_lite(service).await {
            Ok(ip_str) => {
                if let Ok(ip) = ip_str.trim().parse() {
                    log::info!("External IP detected via API: {}", ip);
                    return Some(ip);
                }
            }
            Err(e) => {
                log::debug!("Failed to get IP from {}: {}", service, e);
            }
        }
    }

    None
}

/// Simple HTTP GET request (minimal implementation to avoid reqwest dependency)
async fn reqwest_lite(url: &str) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    // Parse URL (very basic)
    let url = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let (host, path) = url.split_once('/').unwrap_or((url, ""));
    let path = format!("/{}", path);

    // Connect
    let addr = format!("{}:80", host);
    let mut stream = TcpStream::connect(&addr).await.map_err(|e| e.to_string())?;

    // Send request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    // Read response
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .map_err(|e| e.to_string())?;

    // Extract body (after headers)
    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos + 4..].to_string())
    } else {
        Err("Invalid HTTP response".to_string())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upnp_manager_creation() {
        let mgr = UpnpManager::new();
        assert!(mgr.is_enabled());
    }

    #[test]
    fn test_upnp_disabled() {
        let mgr = UpnpManager::disabled();
        assert!(!mgr.is_enabled());
    }

    #[test]
    fn test_get_local_ip() {
        // This might fail in CI environments without network access
        let ip = get_local_ip();
        // Just check it doesn't panic
        if let Some(ip) = ip {
            assert!(!ip.is_loopback());
        }
    }

    #[tokio::test]
    async fn test_upnp_status() {
        let mgr = UpnpManager::new();
        let status = mgr.status().await;

        assert!(status.enabled);
        assert!(!status.gateway_found);
        assert!(status.external_ip.is_none());
        assert!(status.mapped_port.is_none());
    }
}
