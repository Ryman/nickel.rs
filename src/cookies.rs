use request::Request;
use plugin::{Plugin, Pluggable};
use typemap::Key;
use hyper::header;

use cookie::CookieJar;

pub struct SecretKey(pub [u8; 32]);

// Plugin boilerplate
struct CookiePlugin;
impl Key for CookiePlugin { type Value = CookieJar<'static>; }

impl<'mw, 'conn, D> Plugin<Request<'mw, 'conn, D>> for CookiePlugin
        where D: AsRef<SecretKey> {
    type Error = ();

    fn eval(req: &mut Request<D>) -> Result<CookieJar<'static>, ()> {
        let key = req.data().as_ref();
        let jar = match req.origin.headers.get::<header::Cookie>() {
            Some(c) => c.to_cookie_jar(&key.0),
            None => CookieJar::new(&key.0)
        };

        Ok(jar)
    }
}

pub trait Cookies {
    fn cookies(&mut self) -> &CookieJar;
}

impl<'mw, 'conn, D> Cookies for Request<'mw, 'conn, D>
        where D: AsRef<SecretKey> {
    fn cookies(&mut self) -> &CookieJar {
        self.get_ref::<CookiePlugin>().unwrap()
    }
}
