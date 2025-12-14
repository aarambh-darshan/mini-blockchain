//! TCP server and connection handling
//!
//! Accepts incoming peer connections and manages the network server.

use crate::network::message::{Handshake, Message, MAGIC};
use crate::network::peer::{PeerError, PeerHandle, PeerManager};
use bytes::{Buf, BufMut, BytesMut};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Encoder, Framed};

/// Message codec for length-prefixed framing
pub struct MessageCodec;

impl Encoder<Message> for MessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let data = item
            .to_bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        // Magic (4) + Length (4) + Data
        dst.reserve(8 + data.len());
        dst.put_slice(&MAGIC);
        dst.put_u32(data.len() as u32);
        dst.put_slice(&data);

        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Need at least header
        if src.len() < 8 {
            return Ok(None);
        }

        // Check magic
        if src[..4] != MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid magic bytes",
            ));
        }

        // Get length
        let len = u32::from_be_bytes([src[4], src[5], src[6], src[7]]) as usize;

        // Check if we have full message
        if src.len() < 8 + len {
            return Ok(None);
        }

        // Skip header
        src.advance(8);

        // Extract message data
        let data = src.split_to(len);

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
    let framed = Framed::new(stream, MessageCodec);
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
        let mut codec = MessageCodec;
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
}
