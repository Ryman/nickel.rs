//! Blanket impls for Middleware.
//! This is pre-implemented for any function which takes a
//! `Request` and `Response` parameter and returns anything
//! implementing the `Responder` trait. It is also
//! implemented for a tuple of a function and a type `T`.
//! The function must take a `Request`, a `Response` and a
//! `T`, returning anything that implements `Responder`.
//! The data of type `T` will then be shared and available
//! in any request.
//!
//! Please see the examples for usage.
use response::Response;
use hyper::status::{StatusCode, StatusClass};
use hyper::header;
use middleware::{MiddlewareResult, Halt, Continue};
use serialize::json;
use mimes::MediaType;
use std::io::Write;
use std::fmt::Debug;

/// This trait provides convenience for translating a number
/// of common return types into a `MiddlewareResult` while
/// also modifying the `Response` as required.
///
/// Please see the examples for some uses.
pub trait Responder {
    fn respond<'a>(self, Response<'a>) -> MiddlewareResult<'a>;
}

impl Responder for () {
    fn respond<'a>(self, res: Response<'a>) -> MiddlewareResult<'a> {
        Ok(Continue(res))
    }
}

impl Responder for json::Json {
    fn respond<'a>(self, mut res: Response<'a>) -> MiddlewareResult<'a> {
        maybe_set_type(&mut res, MediaType::Json);
        res.send(json::encode(&self)
                      .map_err(|e| format!("Failed to parse JSON: {}", e)))
    }
}

impl<T, E> Responder for Result<T, E>
        where T: Responder, E: Debug {
    fn respond<'a>(self, res: Response<'a>) -> MiddlewareResult<'a> {
        match self {
            Ok(data) => res.send(data),
            Err(e) => res.error(StatusCode::InternalServerError,
                                format!("{:?}", e))
        }
    }
}

macro_rules! dual_impl {
    ($view:ty, $alloc:ty, |$s:ident, $res:ident| $b:block) => (
        impl<'a> Responder for $view {
            #[allow(unused_mut)]
            #[inline]
            fn respond<'c>($s, mut $res: Response<'c>) -> MiddlewareResult<'c> $b
        }

        impl<'a> Responder for $alloc {
            #[allow(unused_mut)]
            #[inline]
            fn respond<'c>($s, mut $res: Response<'c>) -> MiddlewareResult<'c> $b
        }
    )
}

dual_impl!(&'a [u8],
           Vec<u8>,
            |self, res| {
                maybe_set_type(&mut res, MediaType::Bin);

                let mut stream = try!(res.start());
                match stream.write_all(&self[..]) {
                    Ok(()) => Ok(Halt(stream)),
                    Err(e) => stream.bail(format!("Failed to send: {}", e))
                }
            });

dual_impl!(&'static str,
           String,
            |self, res| {
                maybe_set_type(&mut res, MediaType::Html);
                res.send(self.as_bytes())
            });

dual_impl!((StatusCode, &'static str),
           (StatusCode, String),
            |self, res| {
                let (status, message) = self;

                match status.class() {
                    StatusClass::ClientError | StatusClass::ServerError => {
                        res.error(status, message)
                    },
                    _ => {
                        res.set(status);
                        res.send(message)
                    }
                }
            });

dual_impl!(&'a [&'a str],
           &'a [String],
            |self, res| {
                maybe_set_type(&mut res, MediaType::Html);

                let mut stream = try!(res.start());
                for ref s in self.iter() {
                    if let Err(e) = stream.write_all(s.as_bytes()) {
                        return stream.bail(format!("Failed to write to stream: {}", e))
                    }
                }
                Ok(Halt(stream))
            });

dual_impl!((u16, &'static str),
           (u16, String),
           |self, res| {
                let (status, message) = self;
                res.send((StatusCode::from_u16(status), message))
            });

// FIXME: Hyper uses traits for headers, so this needs to be a Vec of
// trait objects. But, a trait object is unable to have Foo + Bar as a bound.
//
// A better/faster solution would be to impl this for tuples,
// where each tuple element implements the Header trait, which would give a
// static dispatch.
// dual_impl!((StatusCode, &'a str, Vec<Box<ResponseHeader>>),
//            (StatusCode, String, Vec<Box<ResponseHeader>>)
//            |self, res| {
//                 let (status, data, headers) = self;

//                 res.origin.status = status;
//                 for header in headers.into_iter() {
//                     res.origin.headers_mut().set(header);
//                 }
//                 maybe_set_type(&mut res, MediaType::Html);
//                 res.send(data);
//                 Ok(Halt)
//             })

fn maybe_set_type(res: &mut Response, mime: MediaType) {
    res.set_header_fallback(|| header::ContentType(mime.into()));
}
