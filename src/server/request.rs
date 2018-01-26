use std::rc::Rc;
use std::io::Read;
use std::{fmt, io, slice, str};

use httparse;
use http::header::*;
use bytes::BytesMut;
use body::BodyReader;
use http::{Method, Version};

pub(crate) fn decode(buf: &mut BytesMut) -> io::Result<Option<Request>> {
    let (method, path, version, headers, amt) = {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut r = httparse::Request::new(&mut headers);
        let status = r.parse(buf).map_err(|e| {
            let msg = format!("failed to parse http request: {:?}", e);
            io::Error::new(io::ErrorKind::Other, msg)
        })?;

        let amt = match status {
            httparse::Status::Complete(amt) => amt,
            httparse::Status::Partial => return Ok(None),
        };

        let toslice = |a: &[u8]| {
            let start = a.as_ptr() as usize - buf.as_ptr() as usize;
            assert!(start < buf.len());
            (start, start + a.len())
        };

        (
            toslice(r.method.unwrap().as_bytes()),
            toslice(r.path.unwrap().as_bytes()),
            r.version.unwrap(),
            r.headers
                .iter()
                .map(|h| (toslice(h.name.as_bytes()), toslice(h.value)))
                .collect(),
            amt,
        )
    };

    Ok(Request {
        method: method,
        path: path,
        version: version,
        headers: headers,
        data: buf.split_to(amt),
        body: BodyReader::EmptyReader,
    }.into())
}

type Slice = (usize, usize);

/// server side http request headers
///
/// the static view of incoming http request
/// you can't mutate it but only get information from it.
pub struct RequestHeaders<'req> {
    headers: slice::Iter<'req, (Slice, Slice)>,
    req: &'req Request,
}

impl<'req> RequestHeaders<'req> {
    /// Returns a reference to the value associated with the key.
    ///
    /// If there are multiple values associated with the key, then the first one
    /// is returned. Use `get_all` to get all values associated with a given
    /// key. Returns `None` if there are no values associated with the key.
    pub fn get<K: AsHeaderName>(&self, _key: K) -> Option<&[u8]> {
        unimplemented!()
    }

    // fn get_all<K:AsHeaderName>(&self, key: K) -> GetAll<T>

    /// Returns true if the map contains a value for the specified key.
    ///
    pub fn contains_key<K: AsHeaderName>(&self, key: K) -> bool {
        self.get(key).is_some()
    }
}

impl<'req> Iterator for RequestHeaders<'req> {
    type Item = (&'req str, &'req [u8]);

    fn next(&mut self) -> Option<(&'req str, &'req [u8])> {
        self.headers.next().map(|&(ref a, ref b)| {
            let a = self.req.slice(a);
            let b = self.req.slice(b);
            (str::from_utf8(a).unwrap(), b)
        })
    }
}

/// server side http request
///
/// this is different from the `http::Request` that it's only a static view of
/// read in bytes. so you can't modify the http header information, the `body`
/// implements `Read`, so you can still read data from the underline connection
pub struct Request {
    method: Slice,
    path: Slice,
    version: u8,
    headers: Vec<(Slice, Slice)>,
    data: BytesMut,
    body: BodyReader,
}

impl Request {
    /// set the body reader
    ///
    /// this function would set a proper `BodyReader` according to the request
    pub fn set_reader(&mut self, reader: Rc<Read>) {
        let method = self.method();
        if method == Method::GET || method == Method::HEAD {
            return;
        }

        let size = self.headers().get(CONTENT_LENGTH).map(|v| unsafe {
            str::from_utf8_unchecked(v)
                .parse()
                .expect("failed to parse content length")
        });

        match size {
            Some(n) => {
                self.body = BodyReader::SizedReader(reader, n);
                return;
            }
            None => {}
        }
        // TODO: add chunked reader
        unimplemented!()
    }

    pub fn body(&self) -> &BodyReader {
        &self.body
    }

    pub fn method(&self) -> Method {
        Method::from_bytes(self.slice(&self.method)).expect("invalide method")
    }

    pub fn path(&self) -> &str {
        str::from_utf8(self.slice(&self.path)).unwrap()
    }

    pub fn version(&self) -> Version {
        if self.version == 0 {
            Version::HTTP_10
        } else {
            Version::HTTP_11
        }
    }

    pub fn headers(&self) -> RequestHeaders {
        RequestHeaders {
            headers: self.headers.iter(),
            req: self,
        }
    }

    fn slice(&self, slice: &Slice) -> &[u8] {
        &self.data[slice.0..slice.1]
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<HTTP Request {} {}>", self.method(), self.path())
    }
}
