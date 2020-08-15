pub mod utils {

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::sync::mpsc::{Receiver, Sender};
    use std::time::Duration;

    use crate::transport::{Transport, TransportBuilder, TransportHandle};
    use crate::{Bytes, Event};

    #[derive(Clone)]
    pub struct TestTransport {
        pub handle: Rc<RefCell<Option<TransportHandle>>>,
        pub internal_tx: Rc<Sender<Event<Bytes>>>,
        pub internal_rx: Rc<Receiver<Event<Bytes>>>,
        pub reconnect_timeout: Rc<RefCell<Duration>>,
    }

    impl TestTransport {
        #[allow(dead_code)]
        pub fn new() -> TestTransport {
            Default::default()
        }
    }

    impl TransportBuilder for TestTransport {
        fn start(&self) -> Transport {
            let TransportHandle { tx, rx } = self.handle.borrow_mut().take().unwrap();
            Transport::new(tx, rx)
        }

        fn set_reconnect_timeout(&mut self, timeout: Duration) {
            self.reconnect_timeout.replace(timeout);
        }
    }

    impl Default for TestTransport {
        fn default() -> Self {
            let (tx, internal_rx) = mpsc::channel();
            let (internal_tx, rx) = mpsc::channel();
            TestTransport {
                handle: Rc::new(RefCell::new(Some(TransportHandle { tx, rx }))),
                internal_tx: Rc::new(internal_tx),
                internal_rx: Rc::new(internal_rx),
                reconnect_timeout: Rc::new(RefCell::new(Duration::from_secs(30))),
            }
        }
    }
}
