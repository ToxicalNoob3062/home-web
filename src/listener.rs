use super::cache::Tracker;
use super::responder::Responder;
use super::types::ChannelMessage;
use simple_dns::{CLASS, Packet, PacketFlag, Question};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::Arc,
};
use tokio::{net::UdpSocket, sync::OnceCell};

#[derive(Debug)]
pub struct Listener {
    v4_socket: Arc<Option<UdpSocket>>,
    v6_socket: Arc<Option<UdpSocket>>,
    tracker: Tracker,
    responder: Responder,
    poison: OnceCell<Arc<Listener>>,
}

// here we will write socket helper functions
impl Listener {
    fn set_common_options(msock: &Socket) -> Result<(), std::io::Error> {
        msock.set_ttl(255)?;
        msock.set_reuse_address(true)?;
        msock.set_nonblocking(true)?;
        Ok(())
    }

    fn set_v4_multicast_options(msock: &Socket) -> Result<(), std::io::Error> {
        // Disable multicast loopback during production
        msock.set_multicast_loop_v4(false)?;

        let bind_addr: SocketAddrV4 = "0.0.0.0:5353".parse().unwrap();
        msock.bind(&SockAddr::from(bind_addr))?;

        let multicast_addr_v4: Ipv4Addr = "224.0.0.251".parse().unwrap();
        msock.join_multicast_v4(&multicast_addr_v4, &Ipv4Addr::UNSPECIFIED)?;
        Ok(())
    }

    fn set_v6_multicast_options(msock: &Socket) -> Result<(), std::io::Error> {
        // Disable multicast loopback during production
        msock.set_multicast_loop_v6(false)?;

        let bind_addr: SocketAddrV6 = "[::]:5353".parse().unwrap();
        msock.bind(&SockAddr::from(bind_addr))?;

        let multicast_addr_v6: Ipv6Addr = "ff02::fb".parse().unwrap();
        msock.join_multicast_v6(&multicast_addr_v6, 0)?;
        Ok(())
    }

    fn get_v4_msocket() -> Result<UdpSocket, std::io::Error> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        Self::set_common_options(&socket)?;
        Self::set_v4_multicast_options(&socket)?;
        UdpSocket::from_std(socket.into())
            .map_err(|e| std::io::Error::new(e.kind(), format!("Tokio conversion failed: {}", e)))
    }

    fn get_v6_msocket() -> Result<UdpSocket, std::io::Error> {
        let socket = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
        Self::set_common_options(&socket)?;
        Self::set_v6_multicast_options(&socket)?;
        UdpSocket::from_std(socket.into())
            .map_err(|e| std::io::Error::new(e.kind(), format!("Tokio conversion failed: {}", e)))
    }
}

impl Listener {
    // Constructor for Listener
    pub fn new(tracker: Tracker, responder: Responder) -> Result<Arc<Self>, String> {
        let v4_socket = Self::get_v4_msocket().ok();
        let v6_socket = Self::get_v6_msocket().ok();
        // if v4 and v6 both fail, return an error
        if v4_socket.is_none() && v6_socket.is_none() {
            return Err("Failed to create both IPv4 and IPv6 sockets".to_string());
        }
        let listener = Arc::new(Listener {
            v4_socket: Arc::new(v4_socket),
            v6_socket: Arc::new(v6_socket),
            tracker,
            responder,
            poison: OnceCell::new(),
        });
        // set the poison to the listener clone
        if listener.poison.set(listener.clone()).is_err() {
            return Err("Failed to set poison cell".to_string());
        }
        let listener_clone = Arc::clone(&listener);
        tokio::spawn(async move {
            let _ = listener_clone.listen().await;
        });
        Ok(listener)
    }

    // Method to start listening for service discovery messages
    pub async fn listen(&self) -> Result<(), String> {
        let (work_giver, work_taker) = async_channel::bounded::<ChannelMessage>(50);

        // Spawn a task to handle incoming messages
        self.handle_message(work_taker);

        let mut v4_buf = [0u8; 1472];
        let mut v6_buf = [0u8; 1472];
        let mut v4_broken = false;
        let mut v6_broken = false;
        loop {
            if v4_broken && v6_broken {
                return Err("Both IPv4 and IPv6 sockets are broken".to_string());
            }

            tokio::select! {
                result = async {
                    match &*self.v4_socket {
                        Some(socket) => socket.recv_from(&mut v4_buf).await,
                        None => Err(std::io::Error::new(std::io::ErrorKind::Other, "IPv4 socket not initialized")),
                    }
                }, if !v4_broken => {
                    match result {
                        Ok((len, addr)) => {
                            let _ = work_giver.send(ChannelMessage {
                                ip: addr,
                                bytes: v4_buf[..len].to_vec(),
                            }).await;
                        }
                        Err(_) => {
                            v4_broken = true;
                        }
                    }
                }

                result = async {
                    match &*self.v6_socket {
                        Some(socket) => socket.recv_from(&mut v6_buf).await,
                        None => Err(std::io::Error::new(std::io::ErrorKind::Other, "IPv6 socket not initialized")),
                    }
                }, if !v6_broken => {
                    match result {
                        Ok((len, addr)) => {
                            let _ = work_giver.send(ChannelMessage {
                                ip: addr,
                                bytes: v6_buf[..len].to_vec(),
                            }).await;
                        }
                        Err(_) => {
                            v6_broken = true;
                        }
                    }
                }
            }
        }
    }

    async fn handle_response<'a>(packet: Packet<'a>, tracker: Tracker) {
        // Handle the response from the cache or the network
        let responses = [packet.answers, packet.additional_records].concat();
        for response in responses {
            if matches!(response.class, CLASS::IN) {
                if let Some((query, response, ttl)) = super::prepare_triplet_from_record(&response)
                {
                    if let Some(sender) = tracker.get(&query) {
                        // Send the response back to the querier
                        if sender.send(Some((response, ttl))).await.is_err() {
                            println!("Failed to send response for query: {:?}", query);
                        }
                    }
                }
            }
        }
    }

    async fn handle_equery<'a>(
        ip: SocketAddr,
        packet: Packet<'a>,
        listener: Arc<Listener>,
    ) -> Result<(), String> {
        println!("Received query from: {} for {:?}", ip, packet.questions);
        // Separate unicast and multicast questions
        let mut unicast_questions: Vec<Question<'a>> = vec![];
        let mut multicast_questions: Vec<Question<'a>> = vec![];
        for question in packet.questions {
            if question.unicast_response {
                unicast_questions.push(question);
            } else {
                multicast_questions.push(question);
            }
        }
        // Prepare the response for unicast questions
        if !unicast_questions.is_empty() {
            let mut response_packet = listener.responder.answer_queries(unicast_questions);
            if !response_packet.answers.is_empty() && !response_packet.additional_records.is_empty()
            {
                // do answer suppression for answers and aditonal answers
                Responder::suppress_known_answers(&mut response_packet.answers, &packet.answers);
                Responder::suppress_known_answers(
                    &mut response_packet.additional_records,
                    &packet.answers,
                );
                // serialize the response packet
                if let Some(bytes) = super::serialize_packet(&mut response_packet) {
                    // send the response back to  the outer world
                    listener.send(ChannelMessage { ip, bytes }).await?;
                }
            }
        }
        // Prepare the response for multicast questions
        if !multicast_questions.is_empty() {
            let mut response_packet = listener.responder.answer_queries(multicast_questions);
            if !response_packet.answers.is_empty() && !response_packet.additional_records.is_empty()
            {
                // do answer suppression for answers and aditonal answers
                Responder::suppress_known_answers(&mut response_packet.answers, &packet.answers);
                Responder::suppress_known_answers(
                    &mut response_packet.additional_records,
                    &packet.additional_records,
                );
                // serialize the response packet
                if let Some(bytes) = super::serialize_packet(&mut response_packet) {
                    // send the response back to the outer world
                    listener
                        .send(ChannelMessage {
                            ip: super::multicast_addr_v4().clone(),
                            bytes: bytes.clone(),
                        })
                        .await?;
                    listener
                        .send(ChannelMessage {
                            ip: super::multicast_addr_v6().clone(),
                            bytes,
                        })
                        .await?;
                }
            }
        }
        Ok(())
    }

    // Method to handle incoming service discovery messages
    fn handle_message(&self, work_taker: async_channel::Receiver<ChannelMessage>) {
        let total_cpus = num_cpus::get_physical();
        for _ in 0..total_cpus {
            let work_taker_clone = work_taker.clone();
            let tracker_clone = self.tracker.clone();
            let poison_clone: Arc<Listener> = self.poison.get().unwrap().clone();
            tokio::spawn(async move {
                while let Ok(msg) = work_taker_clone.recv().await {
                    let tracker = tracker_clone.clone();
                    if let Ok(packet) = Packet::parse(&msg.bytes)
                        .map_err(|e| println!("Error parsing packet: {}", e))
                    {
                        if packet.has_flags(PacketFlag::RESPONSE) {
                            Self::handle_response(packet, tracker).await;
                        } else {
                            _ = Self::handle_equery(msg.ip, packet, poison_clone.clone()).await;
            
                        };
                    }
                }
            });
        }
    }

    // send a packet
    pub async fn send(&self, msg: ChannelMessage) -> Result<(), String> {
        let socket = match msg.ip {
            std::net::SocketAddr::V4(_) => self.v4_socket.clone(),
            std::net::SocketAddr::V6(_) => self.v6_socket.clone(),
        };
        if let Some(socket) = socket.as_ref() {
            if let Err(e) = socket.send_to(&msg.bytes, msg.ip).await {
                return Err(format!("Failed to send message: {}", e));
            }
        } else {
            return Err("Socket not initialized".to_string());
        }
        Ok(())
    }
}
