use crate::trust_dns_ext;
use crate::trust_dns_ext::MyMessage;
use rmp_serde as rmps;
use serde::Deserialize;
use serde::Serialize;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::rc::Rc;
use trust_dns_client::error::ClientError;
use trust_dns_client::error::ClientErrorKind;
use trust_dns_client::op::Message;
use trust_dns_client::rr::Name;
use trust_dns_client::rr::RecordType;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Proto {
    UDP,
    TCP,
}

impl TryFrom<u8> for Proto {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Proto::UDP),
            2 => Ok(Proto::TCP),
            _ => Err(()),
        }
    }
}

impl From<Proto> for u8 {
    fn from(value: Proto) -> u8 {
        match value {
            Proto::UDP => 1,
            Proto::TCP => 2,
        }
    }
}

impl fmt::Display for Proto {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Proto::TCP => write!(f, "TCP"),
            Proto::UDP => write!(f, "UDP"),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct Question {
    #[serde(with = "trust_dns_ext::custom_serde::binary::name")]
    pub qname: Name,
    #[serde(with = "trust_dns_ext::custom_serde::binary::record_type")]
    pub qtype: RecordType,
    pub proto: Proto,
}

impl Question {
    pub fn new(qname: Name, qtype: RecordType, proto: Proto) -> Question {
        Question {
            qname,
            qtype,
            proto,
        }
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

impl From<&ClientError> for ErrorKind {
    fn from(err: &ClientError) -> Self {
        match err.kind() {
            ClientErrorKind::DnsSec(_)
            | ClientErrorKind::Message(_)
            | ClientErrorKind::Msg(_)
            | ClientErrorKind::SendError(_) => ErrorKind::Internal,
            ClientErrorKind::Io(_) => ErrorKind::Io,
            ClientErrorKind::Timeout => ErrorKind::Timeout,
            ClientErrorKind::Proto(_) => ErrorKind::Protocol,
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
        Result<(Rc<Message>, u16), (ErrorKind, Option<ClientError>)>,
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
    pub retry: u16,
    pub retrans: u16,
}

impl Net {
    pub fn lookup(
        &self,
        question: Question,
        server: IpAddr,
    ) -> (Vec<Failure>, u64, u32, Result<Vec<u8>, ClientError>) {
        use std::iter;
        use std::net::SocketAddr;
        use std::thread;
        use std::time::Duration;
        use std::time::SystemTime;
        use std::time::UNIX_EPOCH;
        use trust_dns_client::client::Client;
        use trust_dns_client::client::SyncClient;
        use trust_dns_client::rr::DNSClass;
        use trust_dns_client::tcp::TcpClientConnection;
        use trust_dns_client::udp::UdpClientConnection;

        let address = SocketAddr::new(server, 53);
        let mut tries_left = self.retry;
        let retrans = self.retrans;
        let mut queries = iter::repeat_with(|| {
            let start_time = SystemTime::now();
            let outcome = match question.proto {
                Proto::TCP => UdpClientConnection::new(address).and_then(|conn| {
                    let client = SyncClient::new(conn);
                    client.query(&question.qname, DNSClass::IN, question.qtype)
                }),
                Proto::UDP => TcpClientConnection::new(address).and_then(|conn| {
                    let client = SyncClient::new(conn);
                    client.query(&question.qname, DNSClass::IN, question.qtype)
                }),
            };
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
        });
        let failures: Vec<_> = queries
            .map_while(|(outcome, query_start, query_duration)| {
                tries_left = tries_left.saturating_sub(1);
                match outcome {
                    Err(failure) if tries_left > 0 => {
                        thread::sleep(Duration::from_secs(retrans as u64));
                        Some(Failure {
                            query_start,
                            query_duration,
                            kind: (&failure).into(),
                        })
                    }
                    _ => None,
                }
            })
            .collect();
        let (outcome, query_start, query_duration) = queries.next().unwrap();
        let bytes =
            outcome.map(|dns_response| dns_response.messages().next().unwrap().to_vec().unwrap());
        (failures, query_start, query_duration, bytes)
    }
}
