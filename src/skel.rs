use std::fmt;
use serde::{Deserialize, Serialize};

pub enum Responses {
    Code(StatCode),
    Data(StatCode, Payload),
}

#[allow(dead_code)]
pub enum StatCode {
    AckOk, // Data Recived OK
    AckDr, // Data Recived
    AckDs, // Data Data sent
    NoHnd, // Resource not foure or err occoured
    NoPer, // Invalid permission or registration
    SecFt, // Integrity check failed
}

pub enum Request {
    Code(RequestCode),
    Data(RequestData),
}

pub struct RequestData {
    pub command: String,
    pub data: String,
    pub requestid: String,
    pub integrity: bool,
}

pub struct RequestCode {
    pub command: String,
    pub requestid: String,
    pub integrity: bool,
}


pub enum Payload {
    Data(String, Integrity),
}

pub enum Integrity {
    Hash(String),
}

// ? Diffrent types of messages
#[derive(Serialize, Deserialize, Debug)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub body: String,
}

pub struct Message {
    pub uuid: String,
    pub message_type: String,
    pub message: String,
}

// Database credential struct
pub struct Database {
    pub username: String,
    pub password: String,
    pub hostaddr: String,
    pub database: String,
}

// Implementations
impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Request::Code(data) => write!(f, "{}", data),
            Request::Data(data) => write!(f, "{}", data),
        }
    }
}

impl fmt::Display for RequestData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{},{},{},{}", self.command, self.data, self.requestid, self.integrity )
    }
}

impl fmt::Display for RequestCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{},{}", self.command, self.requestid, self.integrity )
    }
}

impl fmt::Display for Responses {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Responses::Code(code) => write!(f, "{}", code),
            Responses::Data(code, data) => write!(f, "{},{}", code, data),
        }
    }
}

impl fmt::Display for StatCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StatCode::AckOk => write!(f, "200"), // ok
            StatCode::AckDr => write!(f, "201"), // ack recived data
            StatCode::AckDs => write!(f, "202"), // ack data in response
            StatCode::NoPer => write!(f, "400"), // client messed up
            StatCode::NoHnd => write!(f, "500"), // i messed up
            StatCode::SecFt => write!(f, "520"), // i refuse, security fault
        }
    }
}

impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Payload::Data(data, sec) => write!(f, "{}/{}", data, sec),
        }
    }
}

impl fmt::Display for Integrity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Integrity::Hash(data) => write!(f, "{}", data)
        }
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "To: {}\nSubject: {}\n\n{}",
            self.to, self.subject, self.body
        )
    }
}