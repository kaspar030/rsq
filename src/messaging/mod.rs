pub mod channel;
pub mod msg;
pub mod peer;
pub mod router;
mod util;

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use super::channel::*;
    use super::msg::*;
    use super::peer::*;
    use super::router::Router;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::sync::mpsc::UnboundedSender;

    struct TestPeer {
        id: PeerId,
        num_received: usize,
        tx: UnboundedSender<Arc<Msg>>,
        rx: UnboundedReceiver<Arc<Msg>>,
    }

    impl TestPeer {
        pub fn new(id: &str) -> TestPeer {
            let (tx, rx) = unbounded_channel();
            TestPeer {
                id: PeerId::new(id),
                num_received: 0,
                tx,
                rx,
            }
        }

        pub fn poll(&mut self) {
            loop {
                match self.rx.try_recv() {
                    Ok(_) => self.num_received += 1,
                    _ => return,
                }
            }
        }
    }

    impl Peer for TestPeer {
        fn get_id(&self) -> &PeerId {
            &self.id
        }
        fn get_sink(&self) -> &UnboundedSender<Arc<Msg>> {
            &self.tx
        }
    }

    #[test]
    fn basic() {
        let peerid = "test_peer";
        let peer = TestPeer::new(peerid);
        assert_eq!(peer.get_id(), &PeerId::new(peerid));
    }

    #[test]
    fn peer_send() {
        let channel = Channel::new(ChannelId("test_channel".to_string()));
        let mut peer = TestPeer::new("test_peer");
        let msg = Arc::new(Msg::new_channel_msg(
            peer.get_id().clone(),
            channel.get_id().clone(),
            bytes::Bytes::from("test_data"),
        ));
        assert_eq!(peer.num_received, 0);
        peer.get_sink().send(msg).unwrap();
        peer.poll();
        assert_eq!(peer.num_received, 1);
    }

    #[test]
    fn channel_send() {
        let mut channel = Channel::new(ChannelId("test_channel".to_string()));
        let mut peer = TestPeer::new("test_peer");
        let mut peer2 = TestPeer::new("test_peer2");
        let msg = Msg::new_channel_msg(
            peer.get_id().clone(),
            channel.get_id().clone(),
            bytes::Bytes::from("test_data"),
        );
        channel.subscribe(&peer);
        channel.subscribe(&peer2);

        assert_eq!(peer.num_received, 0);
        assert_eq!(peer2.num_received, 0);
        channel.forward(Arc::new(msg), peer.get_id());
        peer.poll();
        peer2.poll();
        assert_eq!(peer.num_received, 0);
        assert_eq!(peer2.num_received, 1);
    }

    #[test]
    fn router_basic() {
        let router = Router::new();
        let peer = Box::new(TestPeer::new("test_peer"));
        let peer2 = Box::new(TestPeer::new("test_peer2"));
        let mut channel = Channel::new(ChannelId("test_channel".to_string()));
        channel.subscribe(peer.as_ref());
        channel.subscribe(peer2.as_ref());
    }
}
