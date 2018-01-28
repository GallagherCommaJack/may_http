mod request;
mod response;
mod server_impl;

use std::io::Write;

pub use self::request::Request;
pub use self::response::Response;
pub use self::server_impl::HttpServer;

/// the http service trait
/// user code should supply a type that impl the `handle` method for the http server
///
pub trait HttpService {
    /// Receives a `Request`/`Response` pair, and should perform some action on them.
    ///
    /// This could reading from the request, and writing to the response.
    fn handle(&self, request: Request, Response);
}

impl<F> HttpService for F
where
    F: Fn(Request, Response),
    F: Sync + Send,
{
    fn handle(&self, req: Request, res: Response) {
        self(req, res)
    }
}

fn handle_expect(req: &Request, raw_rsp: &mut Write) {
    use http::header::*;
    use http::{StatusCode, Version};
    let expect = match req.headers().get(EXPECT) {
        Some(v) => v.as_bytes(),
        None => return,
    };
    if req.version() == Version::HTTP_11 && expect == b"100-continue" {
        write!(
            raw_rsp,
            "{:?} {}\r\n\r\n",
            Version::HTTP_11,
            StatusCode::CONTINUE
        ).expect("error writing 100-continue");
        raw_rsp.flush().expect("flush failed for expect");
    }
}
