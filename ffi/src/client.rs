use crate::trust_dns_ext;
use crate::trust_dns_ext::MyMessage;
use rmp_serde as rmps;
use serde::Deserialize;
use serde::Serialize;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;
use tokio::runtime::Runtime;
use trust_dns_client::client::AsyncClient;
use trust_dns_client::op::Message;
use trust_dns_client::op::Query;
use trust_dns_client::rr::Name;
use trust_dns_client::rr::RecordType;
use trust_dns_proto::error::ProtoError;
use trust_dns_proto::error::ProtoErrorKind;
use trust_dns_proto::xfer::DnsRequest;
use trust_dns_proto::xfer::DnsResponse;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Protocol {
    UDP,
    TCP,
}

impl TryFrom<u8> for Protocol {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Protocol::UDP),
            2 => Ok(Protocol::TCP),
            _ => Err(()),
        }
    }
}

impl From<Protocol> for u8 {
    fn from(value: Protocol) -> u8 {
        match value {
            Protocol::UDP => 1,
            Protocol::TCP => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct EdnsConfig {
    pub version: u8,
    pub dnssec_ok: bool,
    pub option_code: u16,
    //pub set_z_flag: bool, TODO: implement this
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct Question {
    #[serde(with = "trust_dns_ext::custom_serde::binary::name")]
    pub qname: Name,
    #[serde(with = "trust_dns_ext::custom_serde::binary::record_type")]
    pub qtype: RecordType,
    pub proto: Protocol,
    pub recursion_desired: bool,
    pub edns_config: Option<EdnsConfig>,
}

impl From<Question> for DnsRequest {
    fn from(question: Question) -> DnsRequest {
        use trust_dns_client::op::MessageType;
        use trust_dns_client::op::OpCode;
        use trust_dns_client::rr::rdata::opt::EdnsOption;
        use trust_dns_proto::xfer::DnsRequestOptions;

        let query = Query::query(question.qname.clone(), question.qtype);

        // build the message
        let mut message: Message = Message::new();

        // TODO: This is not the final ID, it's actually set in the poll method of DNS future
        //  should we just remove this?
        //let id: u16 = rand::random();

        message.add_query(query);
        message
            //.set_id(id)
            .set_message_type(MessageType::Query)
            .set_op_code(OpCode::Query)
            .set_recursion_desired(question.recursion_desired);

        let mut request_options = DnsRequestOptions::default();
        // Extended dns
        if let Some(edns_config) = question.edns_config {
            request_options.use_edns = true;
            let edns = message.edns_mut();
            edns.set_max_payload(512);
            edns.set_version(edns_config.version);
            edns.set_dnssec_ok(edns_config.dnssec_ok);
            if edns_config.option_code != 0 {
                edns.options_mut()
                    .insert(EdnsOption::Unknown(edns_config.option_code, vec![]));
            }
        }

        DnsRequest::new(message, request_options)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ErrorKind {
    Io,
    Timeout,
    Protocol,
    Internal,
    Lock,
}

impl From<&ProtoError> for ErrorKind {
    fn from(err: &ProtoError) -> Self {
        match err.kind() {
            ProtoErrorKind::Io(_) => ErrorKind::Io,
            ProtoErrorKind::Timeout => ErrorKind::Timeout,
            ProtoErrorKind::CharacterDataTooLong { .. }
            | ProtoErrorKind::IncorrectRDataLengthRead { .. } => ErrorKind::Protocol,
            _ => ErrorKind::Internal,
        }
    }
}

impl From<ErrorKind> for u16 {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::Internal => 1,
            ErrorKind::Io => 2,
            ErrorKind::Protocol => 3,
            ErrorKind::Timeout => 4,
            ErrorKind::Lock => 5,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::Internal => write!(f, "INTERNAL_ERROR"),
            ErrorKind::Io => write!(f, "IO_ERROR"),
            ErrorKind::Protocol => write!(f, "PROTOCOL_ERROR"),
            ErrorKind::Timeout => write!(f, "TIMEOUT_ERROR"),
            ErrorKind::Lock => write!(f, "LOCK_ERROR"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
struct Response {
    failures: Vec<Failure>,
    /// Millis since epoch
    query_time: u64,
    /// Millis
    query_duration: u32,
    outcome: Result<MyMessage, ErrorKind>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct Cache {
    cache: HashMap<Question, HashMap<IpAddr, Rc<Response>>>,
    #[serde(skip)]
    is_reading: Cell<bool>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            cache: HashMap::new(),
            is_reading: Cell::new(false),
        }
    }

    pub fn lookup(
        &mut self,
        net: Option<Rc<Net>>,
        question: Question,
        server: IpAddr,
    ) -> Option<(
        u64,
        u32,
        Result<(Rc<Message>, u16), (ErrorKind, Option<ProtoError>)>,
    )> {
        match net {
            None => self
                .cache
                .get(&question)
                .and_then(|inner| inner.get(&server))
                .cloned()
                .map(|response| (response, None)),
            Some(ref net) => {
                if self.is_reading.get() {
                    return Some((0, 0, Err((ErrorKind::Lock, None))));
                }
                let mut details = None;
                let response = self
                    .cache
                    .entry(question.clone())
                    .or_insert_with(HashMap::new)
                    .entry(server)
                    .or_insert_with(|| {
                        let (failures, query_time, query_duration, bytes) =
                            net.lookup(question, server);
                        let outcome = match bytes {
                            Ok(bytes) => {
                                let (message, parse_err) = MyMessage::from_vec(bytes);
                                details = parse_err.map(Into::into);
                                Ok(message)
                            }
                            Err(lookup_err) => {
                                let err_kind = (&lookup_err).into();
                                details = Some(lookup_err);
                                Err(err_kind)
                            }
                        };
                        Rc::new(Response {
                            failures,
                            query_time,
                            query_duration,
                            outcome,
                        })
                    })
                    .clone();
                Some((response, details))
            }
        }
        .map(|(response, details)| {
            (
                response.query_time,
                response.query_duration,
                match &response.outcome {
                    Ok(mymessage) => match mymessage.decoded {
                        Some(ref message) => Ok((message.clone(), mymessage.encoded.len() as u16)),
                        None => Err((ErrorKind::Protocol, details)),
                    },
                    Err(error_kind) => Err((error_kind.clone(), None)),
                },
            )
        })
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut rmps::Serializer::new(&mut buf))
            .unwrap();
        buf
    }

    pub fn from_vec(buf: Vec<u8>) -> Result<Cache, rmps::decode::Error> {
        Cache::deserialize(&mut rmps::Deserializer::new(&buf[..]))
    }

    pub fn for_each_request(&self, callback: impl FnMut((Question, IpAddr))) {
        let old_val = self.is_reading.replace(true);
        self.cache
            .iter()
            .flat_map(|(question, inner)| inner.keys().map(|server| (question.clone(), *server)))
            .for_each(callback);
        self.is_reading.set(old_val);
    }

    pub fn for_each_retry(
        &self,
        question: &Question,
        server: &IpAddr,
        mut callback: impl FnMut(u64, u32, ErrorKind),
    ) {
        let old_val = self.is_reading.replace(true);
        self.cache
            .get(question)
            .and_then(|inner| inner.get(server))
            .iter()
            .flat_map(|response| &response.failures)
            .for_each(|failure| {
                callback(failure.query_start, failure.query_duration, failure.kind)
            });
        self.is_reading.set(old_val);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Failure {
    query_start: u64,
    query_duration: u32,
    kind: ErrorKind,
}

#[derive(Debug)]
pub struct Net {
    pub timeout: u32,
    pub retry: u16,
    pub retrans: u32,
}

impl Net {
    pub fn lookup(
        &self,
        question: Question,
        server: IpAddr,
    ) -> (Vec<Failure>, u64, u32, Result<Vec<u8>, ProtoError>) {
        use std::thread;

        let address = SocketAddr::new(server, 53);
        let timeout = Duration::from_millis(self.timeout as u64);
        let runtime = Runtime::new().unwrap();
        let mut client: AsyncClient = match question.proto {
            Protocol::UDP => Self::new_udp_client(&runtime, address, timeout),
            Protocol::TCP => Self::new_tcp_client(&runtime, address, timeout),
        };
        let mut failures = Vec::new();
        let mut final_outcome = None;
        for tries_left in (0..self.retry.max(1)).rev() {
            let (outcome, query_start, query_duration) =
                Self::query(&runtime, &mut client, question.clone());
            match outcome {
                Err(failure) if tries_left > 0 => {
                    failures.push(Failure {
                        query_start,
                        query_duration,
                        kind: (&failure).into(),
                    });
                    thread::sleep(Duration::from_millis(self.retrans as u64));
                }
                outcome => {
                    final_outcome = Some((outcome, query_start, query_duration));
                    break;
                }
            }
        }

        let (outcome, query_start, query_duration) = final_outcome.unwrap();
        let bytes =
            outcome.map(|dns_response| dns_response.messages().next().unwrap().to_vec().unwrap());

        (failures, query_start, query_duration, bytes)
    }

    fn query(
        runtime: &Runtime,
        client: &mut AsyncClient,
        question: Question,
    ) -> (Result<DnsResponse, ProtoError>, u64, u32) {
        use std::time::SystemTime;
        use std::time::UNIX_EPOCH;
        use trust_dns_proto::DnsHandle;

        let query = client.send(question);
        let start_time = SystemTime::now();
        let outcome = runtime.block_on(query);
        let end_time = SystemTime::now();
        let query_duration = end_time
            .duration_since(start_time)
            .expect("Time went backwards during request")
            .as_millis() as u32;
        let query_time = start_time
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards before request")
            .as_millis() as u64;
        (outcome, query_time, query_duration)
    }

    fn new_udp_client(runtime: &Runtime, address: SocketAddr, timeout: Duration) -> AsyncClient {
        use tokio::net::UdpSocket;
        use trust_dns_client::udp::UdpClientStream;

        let stream = UdpClientStream::<UdpSocket>::with_timeout(address, timeout);
        let client = AsyncClient::connect(stream);
        let (client, bg) = runtime.block_on(client).expect("connection failed");
        runtime.spawn(bg);
        client
    }

    fn new_tcp_client(runtime: &Runtime, address: SocketAddr, timeout: Duration) -> AsyncClient {
        use tokio::net::TcpStream;
        use trust_dns_client::rr::dnssec::Signer;
        use trust_dns_client::tcp::TcpClientStream;
        use trust_dns_proto::iocompat::AsyncIoTokioAsStd;
        use trust_dns_proto::tcp::TcpClientConnect;
        use trust_dns_proto::xfer::DnsMultiplexer;
        use trust_dns_proto::xfer::DnsMultiplexerConnect;

        let (tcp_client_stream, handle) =
            TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::with_timeout(address, timeout);
        let stream: DnsMultiplexerConnect<
            TcpClientConnect<AsyncIoTokioAsStd<TcpStream>>,
            TcpClientStream<AsyncIoTokioAsStd<TcpStream>>,
            Signer,
        > = DnsMultiplexer::new(tcp_client_stream, handle, None);
        let client = AsyncClient::connect(stream);
        let (client, bg) = runtime.block_on(client).expect("connection failed");
        runtime.spawn(bg);
        client
    }
}
