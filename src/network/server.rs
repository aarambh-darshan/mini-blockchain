//! TCP server and connection handling
//!
//! Accepts incoming peer connections and manages the network server.
//! Production-grade features:
//! - SHA-256 message checksums for integrity
//! - Length-prefixed framing with magic bytes

use crate::network::message::{Handshake, Message, MAGIC, HEADER_SIZE, MAX_MESSAGE_SIZE};
use crate::network::peer::{PeerError, PeerHandle, PeerManager};
use bytes::{Buf, BufMut, BytesMut};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Encoder, Framed};

/// Message codec for length-prefixed framing with checksums
/// 
/// Header format (24 bytes):
/// - Magic (4 bytes): Network identification
/// - Command (12 bytes): Message type name (null-padded)
/// - Length (4 bytes): Payload length (big-endian)
/// - Checksum (4 bytes): First 4 bytes of double SHA-256 of payload
pub struct MessageCodec {
    /// Whether to verify checksums (can be disabled for testing)
    pub verify_checksum: bool,
}

impl MessageCodec {
    pub fn new() -> Self {
        Self {
            verify_checksum: true,
        }
    }
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let data = item
            .to_bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        // Check message size limit
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes", data.len()),
            ));
        }

        // Compute checksum (first 4 bytes of double SHA-256)
        let checksum = Message::compute_checksum(&data);

        // Get command name
        let command = item.command();

        // Header: Magic (4) + Command (12) + Length (4) + Checksum (4) = 24 bytes
        dst.reserve(HEADER_SIZE + data.len());
        dst.put_slice(&MAGIC);           // 4 bytes
        dst.put_slice(&command);         // 12 bytes
        dst.put_u32(data.len() as u32);  // 4 bytes
        dst.put_slice(&checksum);        // 4 bytes
        dst.put_slice(&data);            // payload

        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Need at least header (24 bytes)
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }

        // Check magic
        if src[..4] != MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid magic bytes",
            ));
        }

        // Extract command (bytes 4-16) - for logging
        let _command = &src[4..16];

        // Get length (bytes 16-20)
        let len = u32::from_be_bytes([src[16], src[17], src[18], src[19]]) as usize;

        // Check message size limit
        if len > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes", len),
            ));
        }

        // Get expected checksum (bytes 20-24)
        let expected_checksum = [src[20], src[21], src[22], src[23]];

        // Check if we have full message
        if src.len() < HEADER_SIZE + len {
            return Ok(None);
        }

        // Skip header
        src.advance(HEADER_SIZE);

        // Extract message data
        let data = src.split_to(len);

        // Verify checksum
        if self.verify_checksum {
            let actual_checksum = Message::compute_checksum(&data);
            if actual_checksum != expected_checksum {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Checksum mismatch: expected {:02x}{:02x}{:02x}{:02x}, got {:02x}{:02x}{:02x}{:02x}",
                        expected_checksum[0], expected_checksum[1], expected_checksum[2], expected_checksum[3],
                        actual_checksum[0], actual_checksum[1], actual_checksum[2], actual_checksum[3]
                    ),
                ));
            }
        }

        // Deserialize
        let msg = Message::from_bytes(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        Ok(Some(msg))
    }
}

/// TCP server for accepting peer connections
pub struct Server {
    listener: TcpListener,
    port: u16,
}

impl Server {
    /// Bind to a port and create the server
    pub async fn bind(port: u16) -> Result<Self, std::io::Error> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await?;
        log::info!("Server listening on {}", addr);

        Ok(Self { listener, port })
    }

    /// Get the listening port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Accept incoming connections
    pub async fn accept(&self) -> Result<(TcpStream, SocketAddr), std::io::Error> {
        self.listener.accept().await
    }
}

/// Connect to a peer
pub async fn connect_to_peer(addr: &str) -> Result<(TcpStream, SocketAddr), PeerError> {
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|e| PeerError::ConnectionFailed(e.to_string()))?;

    let peer_addr = stream
        .peer_addr()
        .map_err(|e| PeerError::ConnectionFailed(e.to_string()))?;

    Ok((stream, peer_addr))
}

/// Handle a peer connection (both inbound and outbound)
pub async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    peer_manager: Arc<PeerManager>,
    our_handshake: Handshake,
    message_tx: mpsc::Sender<(SocketAddr, Message)>,
    outbound: bool,
) -> Result<(), PeerError> {
    let framed = Framed::new(stream, MessageCodec::new());
    let (mut writer, mut reader) = framed.split();

    // Create channel for sending to this peer
    let (tx, mut rx) = mpsc::channel::<Message>(100);
    let handle = PeerHandle { addr, tx };

    // Add peer to manager
    peer_manager.add_peer(addr, handle, outbound).await?;

    // Send our handshake
    writer
        .send(Message::Handshake(our_handshake))
        .await
        .map_err(PeerError::IoError)?;

    log::debug!("Sent handshake to {}", addr);

    // Spawn writer task
    let write_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if writer.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read messages
    loop {
        match reader.next().await {
            Some(Ok(msg)) => {
                // Forward message to node
                if message_tx.send((addr, msg)).await.is_err() {
                    break;
                }
            }
            Some(Err(e)) => {
                log::warn!("Error reading from {}: {}", addr, e);
                break;
            }
            None => {
                log::info!("Peer {} disconnected", addr);
                break;
            }
        }
    }

    // Cleanup
    write_handle.abort();
    peer_manager.remove_peer(&addr).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_codec() {
        let mut codec = MessageCodec::new();
        let msg = Message::Ping(12345);

        let mut buf = BytesMut::new();
        codec.encode(msg, &mut buf).unwrap();

        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        if let Message::Ping(nonce) = decoded {
            assert_eq!(nonce, 12345);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_checksum_verification() {
        let mut codec = MessageCodec::new();
        let msg = Message::Ping(12345);

        let mut buf = BytesMut::new();
        codec.encode(msg, &mut buf).unwrap();

        // Corrupt the checksum (bytes 20-23)
        buf[20] ^= 0xFF;

        // Should fail to decode
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_checksum_disabled() {
        let mut codec = MessageCodec { verify_checksum: false };
        let msg = Message::Ping(12345);

        let mut buf = BytesMut::new();
        codec.encode(msg, &mut buf).unwrap();

        // Corrupt the checksum
        buf[20] ^= 0xFF;

        // Should still decode with verification disabled
        let result = codec.decode(&mut buf);
        assert!(result.is_ok());
    }
}
