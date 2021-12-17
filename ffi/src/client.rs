use crate::trust_dns_ext;
use crate::trust_dns_ext::MyMessage;
use rmp_serde as rmps;
use serde::Deserialize;
use serde::Serialize;
use std::cell::Cell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::runtime::Handle;
use tokio::runtime::Runtime;
use trust_dns_client::client::AsyncClient;
use trust_dns_client::op::Message;
use trust_dns_client::op::Query;
use trust_dns_client::rr::Name;
use trust_dns_client::rr::RecordType;
use trust_dns_client::udp::UdpClientStream;
use trust_dns_proto::error::ProtoError;
use trust_dns_proto::error::ProtoErrorKind;
use trust_dns_proto::xfer::DnsRequest;
use trust_dns_proto::xfer::DnsResponse;

use tokio::net::TcpStream;
use trust_dns_client::rr::dnssec::Signer;
use trust_dns_client::tcp::TcpClientStream;
use trust_dns_proto::iocompat::AsyncIoTokioAsStd;
use trust_dns_proto::xfer::DnsMultiplexer;
use trust_dns_proto::xfer::DnsMultiplexerConnect;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Protocol {
    Udp,
    Tcp,
}

impl TryFrom<u8> for Protocol {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Protocol::Udp),
            2 => Ok(Protocol::Tcp),
            _ => Err(()),
        }
    }
}

impl From<Protocol> for u8 {
    fn from(value: Protocol) -> u8 {
        match value {
            Protocol::Udp => 1,
            Protocol::Tcp => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct EdnsConfig {
    pub version: u8,
    pub dnssec_ok: bool,
    pub option_code: u16,
    pub option_value: Vec<u8>,
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
                edns.options_mut().insert(EdnsOption::Unknown(
                    edns_config.option_code,
                    edns_config.option_value,
                ));
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
struct RetriedResponse {
    failures: Vec<Failure>,
    /// Millis since epoch
    started: u64,
    /// Millis
    duration: u32,
    outcome: Result<MyMessage, ErrorKind>,
}

pub struct SingleResponse {
    /// Millis since epoch
    pub started: u64,
    /// Millis
    pub duration: u32,
    pub outcome: Result<(Rc<Message>, u16), ErrorKind>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct Cache {
    cache: HashMap<Question, HashMap<IpAddr, Rc<RetriedResponse>>>,
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
        servers: &HashSet<IpAddr>,
    ) -> HashMap<IpAddr, SingleResponse> {
        use futures::future;
        use futures_util::future::FutureExt;

        let mut results = Vec::new();
        match net {
            None => results.extend(servers.iter().filter_map(|server| {
                self.cache
                    .get(&question)
                    .and_then(|inner| inner.get(server))
                    .cloned()
                    .map(|response| (server, response))
            })),
            Some(ref net) => {
                let question_bucket = self
                    .cache
                    .entry(question.clone())
                    .or_insert_with(HashMap::new);
                let runtime = Runtime::new().unwrap();
                let mut queries = Vec::new();
                for server in servers {
                    if self.is_reading.get() {
                        results.push((
                            server,
                            Rc::new(RetriedResponse {
                                started: 0,
                                duration: 0,
                                outcome: Err(ErrorKind::Lock),
                                failures: Vec::new(),
                            }),
                        ));
                        continue;
                    }
                    if let Some(response) = question_bucket.get(server) {
                        results.push((server, response.clone()));
                    } else {
                        queries.push(net.lookup(question.clone(), *server).map(
                            move |(failures, started, duration, bytes)| {
                                let outcome = match bytes {
                                    Ok(bytes) => {
                                        let (message, parse_err) = MyMessage::from_vec(bytes);
                                        if let Some(parse_err) = parse_err {
                                            Self::perror(started, &parse_err);
                                        }
                                        Ok(message)
                                    }
                                    Err(lookup_err) => {
                                        Self::perror(started, &lookup_err);
                                        Err((&lookup_err).into())
                                    }
                                };
                                let response = Rc::new(RetriedResponse {
                                    failures,
                                    started,
                                    duration,
                                    outcome,
                                });
                                (server, response)
                            },
                        ));
                    }
                }
                let _guard = runtime.enter();
                let responses = runtime.block_on(future::join_all(queries));
                for (server, response) in responses {
                    question_bucket.insert(*server, response.clone());
                    results.push((server, response));
                }
            }
        };
        results
            .into_iter()
            .map(|(server, response)| {
                (
                    *server,
                    SingleResponse {
                        started: response.started,
                        duration: response.duration,
                        outcome: match &response.outcome {
                            Ok(mymessage) => match mymessage.decoded {
                                Some(ref message) => {
                                    Ok((message.clone(), mymessage.encoded.len() as u16))
                                }
                                None => Err(ErrorKind::Protocol),
                            },
                            Err(error_kind) => Err(*error_kind),
                        },
                    },
                )
            })
            .collect()
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

    fn perror<E: fmt::Debug>(started: u64, error: &E) {
        use chrono::TimeZone;
        use chrono::Utc;
        eprintln!(
            "{} netbase: {:?}",
            Utc.timestamp_millis(started as i64)
                .format("%F %H:%M:%S%.3f"),
            error
        );
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
    pub async fn lookup(
        &self,
        question: Question,
        server: IpAddr,
    ) -> (Vec<Failure>, u64, u32, Result<Vec<u8>, ProtoError>) {
        use chrono::Utc;

        let address = SocketAddr::new(server, 53);
        let timeout = Duration::from_millis(self.timeout as u64);
        let retrans = Duration::from_millis(self.retrans as u64);
        let conn_start = Utc::now().timestamp_millis();
        match Self::connect(question.proto, address, timeout).await {
            Ok(mut conn) => {
                let (failures, outcome, query_start, query_duration) =
                    Self::query_retry(&mut conn, &question, self.retry, retrans).await;
                let bytes = outcome
                    .map(|dns_response| dns_response.messages().next().unwrap().to_vec().unwrap());
                (failures, query_start, query_duration, bytes)
            }
            Err(err) => {
                let finished = Utc::now().timestamp_millis();
                let duration = finished - conn_start;
                (vec![], conn_start as u64, duration as u32, Err(err))
            }
        }
    }

    async fn query_retry(
        client: &mut AsyncClient,
        question: &Question,
        tries: u16,
        retrans: Duration,
    ) -> (Vec<Failure>, Result<DnsResponse, ProtoError>, u64, u32) {
        use tokio::time;

        let mut failures = Vec::new();
        let mut final_outcome = None;
        for tries_left in (0..tries.max(1)).rev() {
            let (outcome, query_start, query_duration) =
                Self::query(client, question.clone()).await;
            match outcome {
                Err(failure) if tries_left > 0 => {
                    failures.push(Failure {
                        query_start,
                        query_duration,
                        kind: (&failure).into(),
                    });
                    time::sleep(retrans).await;
                }
                outcome => {
                    final_outcome = Some((outcome, query_start, query_duration));
                    break;
                }
            }
        }

        let (outcome, query_start, query_duration) = final_outcome.unwrap();
        (failures, outcome, query_start, query_duration)
    }

    async fn query(
        client: &mut AsyncClient,
        question: Question,
    ) -> (Result<DnsResponse, ProtoError>, u64, u32) {
        use chrono::Utc;
        use trust_dns_proto::DnsHandle;

        let query = client.send(question);
        let started = Utc::now().timestamp_millis();
        let outcome = query.await;
        let finished = Utc::now().timestamp_millis();
        let duration = finished - started;
        (outcome, started as u64, duration as u32)
    }

    async fn connect(
        proto: Protocol,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<AsyncClient, ProtoError> {
        match proto {
            Protocol::Udp => Self::connect_udp(address, timeout).await,
            Protocol::Tcp => Self::connect_tcp(address, timeout).await,
        }
    }

    async fn connect_udp(
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<AsyncClient, ProtoError> {
        let stream = UdpClientStream::<UdpSocket>::with_timeout(address, timeout);
        AsyncClient::connect(stream).await.map(|(conn, bg)| {
            Handle::current().spawn(bg);
            conn
        })
    }

    async fn connect_tcp(
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<AsyncClient, ProtoError> {
        let (tcp_client_stream, handle) =
            TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::with_timeout(address, timeout);
        let stream: DnsMultiplexerConnect<_, _, Signer> =
            DnsMultiplexer::new(tcp_client_stream, handle, None);
        AsyncClient::connect(stream).await.map(|(conn, bg)| {
            Handle::current().spawn(bg);
            conn
        })
    }
}
