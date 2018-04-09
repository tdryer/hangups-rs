use futures::{Future, Stream};
use hyper;
use hyper::{header, Chunk, Client, Method, Request};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;
use serde_json;
use std::str;
use decoder;
use time;
use sha1::Sha1;
use std::fmt;
use hangouts;
use std::result;
use native_tls;
use channel_parser::{ChannelPayload, ContainerArray, ChannelArray};

const CHANNEL_URL: &str = "https://0.client-channel.google.com/client-channel/channel/bind";
const ORIGIN: &str = "https://hangouts.google.com";
const SERVICE_NAMES: &[&str] = &["babel", "babel_presence_last_seen"];
const DNS_WORKER_THREADS: usize = 1;

error_chain! {
    errors {
        Disconnected {
            description("disconnected"),
            display("disconnected"),
        }
        BadStatus(status: hyper::StatusCode) {
            description("bad status"),
            display("bad status: {}", status),
        }
    }
    foreign_links {
        Http(hyper::Error);
        Unicode(::std::str::Utf8Error);
        Io(::std::io::Error);
        Json(serde_json::Error);
        Tls(native_tls::Error);
    }
}

#[derive(Debug)]
struct Session {
    session_id: String,
    g_session_id: String,
}
impl Session {
    fn from_response(value: serde_json::Value) -> Result<Self> {
        let sid: Result<_> = value
            .pointer("/0/1/1")
            .and_then(|v| v.as_str())
            .ok_or("failed to parse sid".into());
        let gsid: Result<_> = value
            .pointer("/1/1/0/gsid")
            .and_then(|v| v.as_str())
            .ok_or("failed to parse gsid".into());
        Ok(Self {
            session_id: sid?.to_owned(),
            g_session_id: gsid?.to_owned(),
        })
    }
}

type Callback<'a> = &'a Fn(hangouts::StateUpdate);

struct ClientID(String);

pub struct Channel {
    cookies: header::Cookie,
    client_id: Option<ClientID>, // eventually needed for sending requests
    unicode_decoder: decoder::UnicodeDecoder,
    chunk_decoder: decoder::ChunkDecoder,
}
impl Channel {
    pub fn new(cookies: header::Cookie) -> Self {
        Channel {
            cookies: cookies,
            client_id: None,
            unicode_decoder: decoder::UnicodeDecoder::new(),
            chunk_decoder: decoder::ChunkDecoder::new(),
        }
    }

    pub fn listen(&mut self, on_state_update: Callback) -> Result<()> {
        let session = self.fetch_session().chain_err(|| ErrorKind::Disconnected)?;
        info!("Got new session: {:?}", session);
        loop {
            // TODO: Verify that this doesn't any lose data.
            self.open_long_polling_request(&session, on_state_update)
                .chain_err(|| ErrorKind::Disconnected)?;
        }
    }

    fn open_long_polling_request(
        &mut self,
        session: &Session,
        on_state_update: Callback,
    ) -> Result<()> {
        info!("Opening new long polling request");
        let (mut core, client) = get_client()?; // TODO: Error chaining?
        let request = self.get_request(Some(session), None)?; // TODO: Error chaining?

        // TODO: Add timeout for entire request.
        // TODO: Add timeout for each chunk.
        let work = client
            .request(request)
            .from_err::<Error>()
            .and_then(|res| {
                trace!("Response: {}", res.status());
                expect_ok(&res.status())?;
                Ok(res.body())
            })
            .and_then(|body| {
                body.from_err::<Error>().for_each(|c| {
                    // TODO: Remove unwrap.
                    for channel_array in self.handle_pushed_bytes(c).unwrap() {
                        match channel_array.payload {
                            ChannelPayload::NewClientID(new_client_id) => {
                                // TODO: Make parser return ClientID?
                                self.client_id = Some(ClientID(new_client_id));
                                self.add_services(session)?;
                            }
                            ChannelPayload::BatchUpdate(batch_update) => {
                                batch_update.state_update.and_then(|state_updates| {
                                    for state_update in state_updates {
                                        on_state_update(state_update);
                                    }
                                    Some(())
                                });
                            }
                            _ => {}
                        };
                    }
                    Ok(())
                })
            });

        core.run(work)
    }

    // TODO: Add some logging to this.
    fn handle_pushed_bytes(&mut self, c: Chunk) -> Result<Vec<ChannelArray>> {
        let chunks = self.chunk_decoder
            .push_str(&self.unicode_decoder.push_bytes(&c));
        let mut res = Vec::new();
        for chunk in chunks {
            let container_array = ContainerArray::parse(&chunk)
                .chain_err(|| "failed to parse chunk")?;
            for channel_array in container_array.channel_arrays {
                res.push(channel_array);
            }
        }
        Ok(res)
    }

    fn fetch_session(&mut self) -> Result<Session> {
        info!("Creating new session");
        self.send_maps(None, vec![])
            .and_then(|response| Session::from_response(response))
            .map_err(|e| Error::with_chain(e, "failed to create session"))
    }

    fn add_services(&mut self, session: &Session) -> Result<()> {
        info!("Adding services");
        let maps = SERVICE_NAMES
            .iter()
            .map(|service_name| json!({"p": {"3": {"1": {"1": service_name}}}}))
            .collect();
        self.send_maps(Some(session), maps)
            .and_then(|response| {
                response
                    .as_array()
                    .and_then(|array| array.get(0))
                    .and_then(|number| number.as_u64())
                    .ok_or("failed to parse response".into())
            })
            .and_then(|status| match status {
                1 => Ok(()),
                _ => Err("request failed".into()),
            })
            .chain_err(|| "failed to add services")
    }

    fn get_request(
        &mut self,
        session: Option<&Session>,
        maps: Option<Vec<serde_json::Value>>,
    ) -> Result<hyper::Request> {
        let request_id = match maps {
            Some(_) => "0",
            None => "rpc",
        };
        let uri = {
            let mut query = vec![
                ("VER", "8"),
                ("ctype", "hangouts"), // "unknown client type"
                ("RID", request_id),
            ];
            match session {
                Some(session) => {
                    query.push(("gsessionid", &session.g_session_id));
                    query.push(("SID", &session.session_id));
                }
                _ => {}
            };
            if maps.is_none() {
                query.push(("TYPE", "xmlhttp"));
                query.push(("t", "1"));
                query.push(("CI", "0"));
            }
            // TODO: Remove unwrap.
            format!("{}{}", CHANNEL_URL, get_query_string(query))
                .parse()
                .unwrap()
        };
        let method = match maps {
            Some(_) => Method::Post,
            None => Method::Get,
        };
        let mut req = Request::new(method, uri);
        match maps {
            Some(maps) => {
                let mut body = String::new();
                body.push_str(&format!("count={}&ofs=0&", maps.len()));
                for (num, map) in maps.iter().enumerate() {
                    // TODO: Remove unwrap.
                    for (key, val) in map.as_object().unwrap() {
                        // TODO: Remove unwrap.
                        let val_bytes = serde_json::to_vec(val).unwrap();
                        body.push_str(&format!(
                            "req{}_{}={}&",
                            num,
                            key,
                            PercentEncodedString { s: val_bytes }
                        ));
                    }
                }
                req.set_body(body);
            }
            None => {}
        }
        req.headers_mut().set(self.cookies.clone());
        req.headers_mut()
            .set(header::ContentType::form_url_encoded());
        // Add authorization headers. Will get "Bad SID" error if these are incorrect.
        req.headers_mut().set(XGoogAuthUser("0".to_owned()));
        req.headers_mut().set(XOrigin(ORIGIN.to_owned()));
        req.headers_mut().set(self.get_authorization_header());
        trace!("Request: {:?}", req);
        Ok(req)
    }

    fn send_maps(
        &mut self,
        session: Option<&Session>,
        maps: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        trace!("Sending maps: {:?}", maps);
        let (mut core, client) = get_client()?; // TODO: Error chaining?

        self.get_request(session, Some(maps))
            .and_then(|req| Ok(client.request(req)))
            .and_then(|future| {
                Ok(future.and_then(|response| {
                    let status = response.status();
                    response.body().concat2().join(Ok(status))
                }))
            })
            .and_then(|future| core.run(future).chain_err(|| "request error"))
            .and_then(move |(body, status)| {
                let body_str =
                    String::from_utf8(body.to_vec()).chain_err(|| "response is not utf8")?;
                // Log status and body before returning error.
                trace!("Response status: {}", status);
                trace!("Response body: {:?}", body_str);
                expect_ok(&status)?;
                let first_chunk = decoder::ChunkDecoder::new()
                    .push_str(&body_str)
                    .pop()
                    .ok_or::<Error>("failed to decode chunk from response".into())?;
                serde_json::from_str(&first_chunk).chain_err(|| "failed to parse chunk as json")
            })
            .chain_err(|| "failed to send maps")
    }

    fn get_authorization_header(&self) -> header::Authorization<ApiHash> {
        header::Authorization(ApiHash {
            time: time::get_time(),
            // TODO: Remove unwrap.
            sapisid_cookie: self.cookies.get("SAPISID").unwrap().to_owned(),
        })
    }
}

fn get_client() -> Result<
    (
        Core,
        Client<HttpsConnector<hyper::client::HttpConnector>, hyper::Body>,
    ),
> {
    let core = Core::new()?;
    let client = Client::configure()
        .connector(HttpsConnector::new(DNS_WORKER_THREADS, &core.handle())?)
        .build(&core.handle());
    Ok((core, client))
}

struct PercentEncodedString {
    s: Vec<u8>,
}
impl fmt::Display for PercentEncodedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        header::parsing::http_percent_encode(f, self.s.as_slice())
    }
}

fn get_query_string(params: Vec<(&str, &str)>) -> String {
    // TODO: Handle escaping.
    format!(
        "?{}",
        params
            .iter()
            .map(|&(key, val)| format!("{}={}", key, val))
            .collect::<Vec<String>>()
            .join("&")
    )
}

#[derive(Debug, Clone)]
struct ApiHash {
    time: time::Timespec,
    sapisid_cookie: String,
}
impl header::Scheme for ApiHash {
    fn scheme() -> Option<&'static str> {
        Some("SAPISIDHASH")
    }
    fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let auth_string = format!("{} {} {}", self.time.sec, self.sapisid_cookie, ORIGIN);
        let auth_hash = Sha1::from(auth_string).hexdigest();
        write!(f, "{}_{}", self.time.sec, auth_hash)
    }
}
impl str::FromStr for ApiHash {
    type Err = ();
    fn from_str(_s: &str) -> result::Result<Self, Self::Err> {
        panic!("not implemented");
    }
}

fn expect_ok(status_code: &hyper::StatusCode) -> Result<()> {
    match status_code {
        &hyper::StatusCode::Ok => Ok(()),
        status_code => Err(ErrorKind::BadStatus(*status_code).into()),
    }
}

header! { (XOrigin, "X-Origin") => [String] }

header! { (XGoogAuthUser, "X-Goog-AuthUser") => [String] }

#[cfg(test)]
mod tests {

    use serde_json;
    use hyper::header;
    use channel;
    use time;

    #[test]
    fn test_api_hash() {
        let now = time::Timespec {
            sec: 1519452159,
            nsec: 0,
        };
        let header = header::Authorization(channel::ApiHash {
            time: now,
            sapisid_cookie: String::from("jBoR10LFQqxvjDQy/Azg6q-5kgeQ-MiaKF"),
        });
        let expected = "SAPISIDHASH 1519452159_a5813881ad9a05006c22d2e1e28347b4fa4c4205";
        assert_eq!(format!("{}", header), expected);
    }

    #[test]
    fn test_parse_sid_response() {
        let value = serde_json::from_str(
            "[[0,[\"c\",\"EXAMPLE_SID\",\"\",8]\n]\n,[1,[{\"gsid\":\"EXAMPLE_GSID\"}]]\n]\n",
        ).unwrap();
        let session = channel::Session::from_response(value).unwrap();
        assert_eq!(session.session_id, "EXAMPLE_SID");
        assert_eq!(session.g_session_id, "EXAMPLE_GSID");
    }

    #[test]
    fn test_percent_encoded_string() {
        assert_eq!(
            format!(
                "{}",
                channel::PercentEncodedString {
                    s: "foo bar".as_bytes().to_vec(),
                }
            ),
            "foo%20bar"
        )
    }

    #[test]
    fn test_get_query_string() {
        let params = vec![("foo", "bar"), ("fizz", "buzz")];
        assert_eq!(channel::get_query_string(params), "?foo=bar&fizz=buzz");
    }

}
