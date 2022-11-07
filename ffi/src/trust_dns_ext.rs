use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde_bytes::ByteBuf;
use std::fmt;
use std::rc::Rc;
use trust_dns_client::op::Message;
use trust_dns_client::proto::error::ProtoError;

pub struct DigMessage<'a>(&'a Message);

impl<'a> fmt::Display for DigMessage<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use trust_dns_client::op::MessageType;
        use trust_dns_client::rr::rdata::opt::EdnsOption;

        let header = self.0.header();
        writeln!(
            f,
            ";; ->>HEADER<<- opcode: {:?}, status: {:?}, id: {}",
            header.op_code(),
            header.response_code(),
            header.id()
        )?;
        write!(f, ";; flags:")?;
        write!(f, " {:?}", header.message_type())?;
        if header.message_type() == MessageType::Query {
            write!(f, " qr")?;
        }
        if header.authoritative() {
            write!(f, " aa")?;
        }
        if header.truncated() {
            write!(f, " tc")?;
        }
        if header.recursion_desired() {
            write!(f, " rd")?;
        }
        if header.recursion_available() {
            write!(f, " ra")?;
        }
        writeln!(
            f,
            "; QUERY: {}, ANSWER: {}, AUTHORITY: {}, ADDITIONAL: {}",
            header.query_count(),
            header.answer_count(),
            header.name_server_count(),
            header.additional_count()
        )?;

        if let Some(edns) = self.0.extensions() {
            writeln!(f)?;
            writeln!(f, ";; OPT PSEUDOSECTION:")?;
            write!(f, "; EDNS: version: {}, flags:", edns.version())?;
            if edns.dnssec_ok() {
                write!(f, " do")?;
            }
            writeln!(f, "; udp: {}:", edns.max_payload())?;
            for (code, data) in edns.options().as_ref() {
                match data {
                    EdnsOption::DAU(algo) | EdnsOption::DHU(algo) | EdnsOption::N3U(algo) => {
                        writeln!(f, "{:?}: {:?}", code, algo)?
                    }
                    EdnsOption::Unknown(_, data) => {
                        write!(f, "{:?}: ", code)?;
                        for byte in data {
                            write!(f, "{:02x}", byte)?;
                        }
                        writeln!(f)?;
                    }
                    data => writeln!(f, "{{unrecognized {:?}}}>", data)?,
                }
            }
        }

        if header.query_count() > 0 {
            if self.0.extensions().is_none() {
                writeln!(f)?;
            }
            writeln!(f, ";; QUESTION SECTION:")?;
            for query in self.0.queries() {
                writeln!(
                    f,
                    "{}  {}  {}",
                    query.name(),
                    query.query_type(),
                    query.query_class()
                )?;
            }
        }
        if header.answer_count() > 0 {
            writeln!(f)?;
            writeln!(f, ";; ANSWER SECTION:")?;
            for record in self.0.answers() {
                writeln!(f, "{}", record)?;
            }
        }
        if header.name_server_count() > 0 {
            writeln!(f)?;
            writeln!(f, ";; AUTHORITY SECTION:")?;
            for record in self.0.name_servers() {
                writeln!(f, "{}", record)?;
            }
        }
        if header.additional_count() > 0 {
            writeln!(f)?;
            writeln!(f, ";; ADDITIONAL SECTION:")?;
            for record in self.0.additionals() {
                writeln!(f, "{}", record)?;
            }
        }

        Ok(())
    }
}

pub trait MessageExt {
    fn as_dig(&self) -> DigMessage;
}

impl MessageExt for Message {
    fn as_dig(&self) -> DigMessage {
        DigMessage(self)
    }
}

#[derive(Debug)]
pub struct MyMessage {
    pub encoded: Vec<u8>,
    pub decoded: Option<Rc<Message>>,
}

impl PartialEq for MyMessage {
    fn eq(&self, other: &Self) -> bool {
        self.encoded == other.encoded
    }
}
impl Eq for MyMessage {}

impl MyMessage {
    pub fn from_vec(encoded: Vec<u8>) -> (Self, Option<ProtoError>) {
        match Message::from_vec(encoded.as_slice()) {
            Ok(decoded) => (
                MyMessage {
                    encoded,
                    decoded: Some(Rc::new(decoded)),
                },
                None,
            ),
            Err(err) => (
                MyMessage {
                    encoded,
                    decoded: None,
                },
                Some(err),
            ),
        }
    }
}

impl Serialize for MyMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.encoded)
    }
}

impl<'de> Deserialize<'de> for MyMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = ByteBuf::deserialize(deserializer)?;
        Ok(MyMessage::from_vec(value.to_vec()).0)
    }
}

pub mod custom_serde {
    pub mod binary {
        pub mod record_type {
            use serde::Deserialize;
            use serde::Deserializer;
            use serde::Serializer;
            use trust_dns_client::rr::RecordType;
            use trust_dns_client::serialize::binary::BinEncodable;

            pub fn serialize<S>(value: &RecordType, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let s = value
                    .to_bytes()
                    .expect("could not serialize record type value to bytes");
                let value = u16::from_be_bytes([s[0], s[1]]);
                serializer.serialize_u16(value)
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<RecordType, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = u16::deserialize(deserializer)?;
                Ok(value.into())
            }
        }

        pub mod name {
            use serde::de;
            use serde::de::SeqAccess;
            use serde::de::Unexpected;
            use serde::de::Visitor;
            use serde::ser::SerializeSeq;
            use serde::Deserializer;
            use serde::Serializer;
            use serde_bytes::ByteBuf;
            use std::fmt;
            use trust_dns_client::rr::Name;

            pub fn serialize<S>(value: &Name, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let len = if value.is_fqdn() {
                    value.num_labels() + 1
                } else {
                    value.num_labels()
                };
                let mut seq = serializer.serialize_seq(Some(len as usize))?;
                for e in value.iter() {
                    seq.serialize_element(&ByteBuf::from(e.to_vec()))?;
                }
                if value.is_fqdn() {
                    seq.serialize_element(&ByteBuf::new())?;
                }
                seq.end()
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<Name, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct LabelVisitor;

                impl<'de> Visitor<'de> for LabelVisitor {
                    type Value = Name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a sequence of byte strings")
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        let mut name = Name::new();
                        while let Some(e) = seq.next_element::<ByteBuf>()? {
                            let len = e.len();
                            if name.is_fqdn() {
                                return Err(de::Error::invalid_value(
                                    Unexpected::Bytes(e.to_vec().as_slice()),
                                    &"end of sequence",
                                ));
                            } else if len == 0 {
                                name.set_fqdn(true);
                            } else {
                                match name.append_label(e.into_vec()) {
                                Ok(new_name) => name = new_name,
                                Err(_) => return Err(de::Error::invalid_length(len, &"each label must be at most 63 characters for a total of at most 253 characters")),
                            }
                            }
                        }
                        Ok(name)
                    }
                }

                deserializer.deserialize_seq(LabelVisitor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp_serde as rmps;
    use trust_dns_client::rr::Name;
    use trust_dns_client::rr::RecordType;

    #[test]
    fn test_serde() {
        #[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
        struct Question {
            #[serde(with = "custom_serde::binary::name")]
            qname: Name,
            #[serde(with = "custom_serde::binary::record_type")]
            qtype: RecordType,
        }

        fn round_trip(input: &Question) -> Question {
            let mut buf = Vec::new();
            input
                .serialize(&mut rmps::Serializer::new(&mut buf))
                .unwrap();
            Question::deserialize(&mut rmps::Deserializer::new(&buf[..])).unwrap()
        }

        {
            let input = Question {
                qname: "example.com".parse().unwrap(),
                qtype: RecordType::SOA.into(),
            };
            assert_eq!(&input, &round_trip(&input));
        }

        {
            let input = Question {
                qname: "example.com.".parse().unwrap(),
                qtype: RecordType::NS.into(),
            };
            assert_eq!(&input, &round_trip(&input));
        }
    }
}
