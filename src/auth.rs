use hyper::header::Cookie;
use serde_json;
use std::fs::File;
use std::io::Read;

const COOKIE_FILE: &str = "cookies.json";

error_chain!{
    foreign_links {
        Io(::std::io::Error);
        Json(serde_json::Error);
    }
}

pub fn get_cookies() -> Result<Cookie> {
    || -> Result<_> {
        let mut file = File::open(COOKIE_FILE)?;
        let mut string = String::new();
        file.read_to_string(&mut string)?;
        let value: serde_json::Value = serde_json::from_str(&string)?;
        let file_object = value.as_object().ok_or("expected object")?;
        let mut cookie_header = Cookie::new();
        for (name, value) in file_object {
            let value_str = value.as_str().ok_or("invalid value")?;
            cookie_header.append(name.to_owned(), value_str.to_owned());
        }
        Ok(cookie_header)
    }()
        .chain_err(|| format!("failed to load cookie file: {}", COOKIE_FILE))
}
