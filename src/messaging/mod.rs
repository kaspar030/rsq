pub mod channel;
pub mod msg;
pub mod peer;
pub mod router;
mod util;

mod test {
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
        tx: UnboundedSender<Msg>,
        rx: UnboundedReceiver<Msg>,
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
        fn get_sink(&self) -> &UnboundedSender<Msg> {
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
        let mut channel = Channel::new(ChannelId("test_channel".to_string()));
        let mut peer = TestPeer::new("test_peer");
        let msg = Msg::new(
            peer.get_id().clone(),
            channel.get_id().clone(),
            Bytes::from("test_data"),
        );
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
        let msg = Msg::new(
            peer.get_id().clone(),
            channel.get_id().clone(),
            Bytes::from("test_data"),
        );
        channel.attach(&peer);
        channel.attach(&peer2);

        assert_eq!(peer.num_received, 0);
        assert_eq!(peer2.num_received, 0);
        channel.send(msg);
        peer.poll();
        peer2.poll();
        assert_eq!(peer.num_received, 0);
        assert_eq!(peer2.num_received, 1);
    }

    #[test]
    fn router_basic() {
        let mut router = Router::new();
        let mut peer = Box::new(TestPeer::new("test_peer"));
        let mut peer2 = Box::new(TestPeer::new("test_peer2"));
        let mut channel = Channel::new(ChannelId("test_channel".to_string()));
        channel.attach(peer.as_ref());
        channel.attach(peer2.as_ref());
    }
}
