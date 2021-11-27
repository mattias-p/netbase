use crate::trust_dns_ext;
use crate::trust_dns_ext::MyMessage;
use rmp_serde as rmps;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::rc::Rc;
use trust_dns_client::error::ClientError;
use trust_dns_client::error::ClientErrorKind;
use trust_dns_client::op::Message;
use trust_dns_client::rr::Name;
use trust_dns_client::rr::RecordType;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct Question {
    #[serde(with = "trust_dns_ext::custom_serde::binary::name")]
    pub qname: Name,
    #[serde(with = "trust_dns_ext::custom_serde::binary::record_type")]
    pub qtype: RecordType,
}

impl Question {
    pub fn new(qname: Name, qtype: RecordType) -> Question {
        Question { qname, qtype }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ErrorKind {
    Io,
    Timeout,
    Protocol,
    Internal,
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
            ErrorKind::Internal => 1001,
            ErrorKind::Io => 1002,
            ErrorKind::Protocol => 1003,
            ErrorKind::Timeout => 1004,
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
        }
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
struct Response {
    /// Millis since epoch
    query_time: u64,
    /// Millis
    query_duration: u32,
    outcome: Result<MyMessage, ErrorKind>,
}

#[derive(Default)]
pub struct Cache {
    cache: HashMap<Question, HashMap<IpAddr, Rc<Response>>>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            cache: HashMap::new(),
        }
    }
    pub fn lookup_udp(
        &mut self,
        client: Option<Rc<Client>>,
        question: Question,
        server: IpAddr,
    ) -> Option<(
        u64,
        u32,
        Result<(Rc<Message>, u16), (ErrorKind, Option<ClientError>)>,
    )> {
        let mut details = None;
        match client {
            None => self
                .cache
                .get(&question)
                .and_then(|inner| inner.get(&server))
                .cloned(),
            Some(ref client) => {
                let response = self
                    .cache
                    .entry(question.clone())
                    .or_insert_with(HashMap::new)
                    .entry(server)
                    .or_insert_with(|| {
                        let (query_time, query_duration, bytes) =
                            client.lookup_udp(question, server);
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
                            query_time,
                            query_duration,
                            outcome,
                        })
                    })
                    .clone();
                Some(response)
            }
        }
        .map(|response| {
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
        self.cache
            .serialize(&mut rmps::Serializer::new(&mut buf))
            .unwrap();
        buf
    }
    pub fn from_vec(buf: Vec<u8>) -> Result<Cache, rmps::decode::Error> {
        let cache = HashMap::<Question, HashMap<IpAddr, Rc<Response>>>::deserialize(
            &mut rmps::Deserializer::new(&buf[..]),
        )?;
        Ok(Cache {
            cache,
        })
    }

    pub fn dns_requests(&self) -> impl Iterator<Item=(IpAddr, Question)> + '_ {
        self.cache.iter().flat_map(|(question, inner)| inner.keys().map(|server| (*server, question.clone())))
    }
}

pub struct Client;

impl Client {
    pub fn lookup_udp(
        &self,
        question: Question,
        server: IpAddr,
    ) -> (u64, u32, Result<Vec<u8>, ClientError>) {
        use std::net::SocketAddr;
        use std::time::SystemTime;
        use std::time::UNIX_EPOCH;
        use trust_dns_client::client::Client;
        use trust_dns_client::client::SyncClient;
        use trust_dns_client::rr::DNSClass;
        use trust_dns_client::udp::UdpClientConnection;

        let address = SocketAddr::new(server, 53);

        let start_time = SystemTime::now();
        let dns_response = UdpClientConnection::new(address).and_then(|conn| {
            let client = SyncClient::new(conn);
            client.query(&question.qname, DNSClass::IN, question.qtype)
        });
        let end_time = SystemTime::now();

        let query_time = start_time
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards before request")
            .as_millis() as u64;
        let query_duration = end_time
            .duration_since(start_time)
            .expect("Time went backwards during request")
            .as_millis() as u32;
        let bytes = dns_response
            .map(|dns_response| dns_response.messages().next().unwrap().to_vec().unwrap());

        (query_time, query_duration, bytes)
    }
}
