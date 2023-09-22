use std::{net::TcpStream, io::Write};
use logging::append_log;
use mysql::prelude::Queryable;
use system::create_hash;

use crate::{database::create_conn, PROG, skel::{Responses, StatCode, Payload, Integrity}};

pub fn create_message_table(table_name: &str) -> bool {
    let mut conn = create_conn();
    match conn.query_drop(format!(
        r"CREATE TABLE Artisan_Messenger.{} (
            uuid VARCHAR(380) NOT NULL,
            message_type VARCHAR(1024) NOT NULL,
            message VARCHAR(4096) NOT NULL,
            processed BOOLEAN not null DEFAULT 0, 
            PRIMARY KEY (uuid)
        )",
        table_name
    )) {
        Ok(_) => {
            append_log(PROG, &format!("Table created for {}", table_name));
            true
        }
        Err(e) => {
            append_log(
                PROG,
                &format!("Creating table {}, FAILED: {}", table_name, e),
            );
            false
        }
    }
}

pub fn create_permission_table(table_name: &str) -> bool {
    let mut conn = create_conn();
    match conn.query_drop(format!(
        r"CREATE TABLE Artisan_Messenger.{}_permission (
            uuid VARCHAR(380) NOT NULL,
            PRIMARY KEY (uuid)
        )",
        table_name
    )) {
        Ok(_) => {
            append_log(PROG, &format!("Table created for {}", table_name));
            true
        }
        Err(e) => {
            append_log(
                PROG,
                &format!("Creating table {}_permission, FAILED: {}", table_name, e),
            );
            false
        }
    }
}

pub fn check_permission(table: &str, uuid: &str) -> bool {
    let mut conn = create_conn();
    let perm_query: String = format!(
        "SELECT COUNT(*) FROM {}_permission WHERE uuid = '{}'",
        table, uuid
    );
    let count: i32 = match conn.query_first(perm_query) {
        Ok(Some(result)) => result,
        Ok(None) => 0,
        Err(e) => {
            append_log(PROG, &format!("DATABASE UNAVAILABLE: {}", e));
            0
        }
    };
    
    // This is where the delivered messages get deleted
    let maintence_query: String = format!("DELETE FROM {} WHERE processed = '1'", table);
    let _ = match conn.query_drop(maintence_query) {
        Ok(_) => append_log(PROG, "Maintence drops"),
        Err(e) => append_log(PROG, &format!("Maintence Drops Failed: {}", e)),
    };

    match count {
        0 => false,
        _ => true,
    }
}

pub fn payload_integrity(payload: &str) -> bool {
    let data: Vec<String> = payload.split('_').map(|s| s.to_string()).collect();

    let body_data: &str = &data[2];
    let body_hash: &str = &data[3];
    let new_hash: &str = &create_hash(&body_data.to_string());

    let check: bool = body_hash == new_hash;
    match check {
        true => true,
        false => false,
    }
}

// ? WRITTING FUNCTIONS
pub fn sec_fault(tcp_stream: &TcpStream) {
    let sec_fault: Responses = Responses::Code(StatCode::SecFt);
    stream_write(sec_fault, tcp_stream);
}

pub fn send_ack_ds(data: String, tcp_stream: &TcpStream) {
    let ack: Responses = Responses::Data(
        StatCode::AckDs,
        Payload::Data(data.clone(), Integrity::Hash(create_hash(&data))),
    );
    stream_write(ack, tcp_stream);
}

pub fn send_ack_dr(tcp_stream: &TcpStream) {
    let ack: Responses = Responses::Code(StatCode::AckDr);
    stream_write(ack, tcp_stream);
}

pub fn send_ack_ok(tcp_stream: &TcpStream) {
    let ack: Responses = Responses::Code(StatCode::AckOk);
    stream_write(ack, tcp_stream);
}

pub fn no_handel(tcp_stream: &TcpStream) {
    let no_handel: Responses = Responses::Code(StatCode::NoHnd);
    stream_write(no_handel, tcp_stream);
}

pub fn no_permission(tcp_stream: &TcpStream) {
    let no_permission: Responses = Responses::Code(StatCode::NoPer);
    stream_write(no_permission, tcp_stream);
}

// Not response functions
pub fn stream_write(data: Responses, mut tcp_stream: &TcpStream) {
    tcp_stream
        .write(format!("{}", data).as_bytes())
        .expect("Failed at writing onto the unix stream");
}
