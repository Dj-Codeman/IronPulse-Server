use logging::append_log;
use mysql::{prelude::Queryable, PooledConn};
use std::{io::Write, net::TcpStream};
use system::create_hash;

use crate::{
    database::create_conn,
    functions::{
        check_permission, create_message_table, create_permission_table, del_channel,
        payload_integrity, register_to_channel,
    },
    skel::{Integrity, Message, Payload, StatCode},
    Responses, PROG,
};

pub fn complex_processor(
    command: &str,
    data: String,
    register_id: String,
    tcp_stream: &TcpStream,
) {
    match command {
        "RegisterChannel" => register_channel(&data, register_id, tcp_stream),
        "DeleteChannel" => delete_channel(&data, tcp_stream),
        "CreateChannel" => create_channel(&data, register_id, tcp_stream),
        "Store" => match payload_integrity(&data) {
            true => store(data, tcp_stream, register_id),
            false => no_handel(tcp_stream),
        },
        "Check" => check_msg(&data, register_id, tcp_stream),
        &_ => no_handel(tcp_stream),
    }
}

pub fn simple_processor(command: &str, register_id: String, tcp_stream: &TcpStream) {
    match command {
        "Ack" => ack_msg(register_id, tcp_stream),
        &_ => no_handel(tcp_stream),
    }
}

fn create_channel(data: &str, _reg: String, tcp_stream: &TcpStream) {
    let magic = (create_message_table(data), create_permission_table(data));

    let result: bool = match magic {
        (true, true) => true,
        (true, false) => {
            append_log(
                PROG,
                &format!("Unable to create permission table for {}", data),
            );
            false
        }
        (false, true) => {
            append_log(
                PROG,
                &format!("Unable to create message table for {}", data),
            );
            false
        }
        (false, false) => {
            append_log(
                PROG,
                &format!("{}: Database could not be interacted with", data),
            );
            false
        }
    };

    match result {
        true => send_ack_dr(tcp_stream),
        false => no_handel(tcp_stream),
    }
}

fn register_channel(data: &str, reg: String, tcp_stream: &TcpStream) {
    // insert regid into the data_permission table
    match register_to_channel(data, &reg) {
        true => send_ack_dr(tcp_stream),
        false => no_handel(tcp_stream),
    };
}

fn delete_channel(data: &str, tcp_stream: &TcpStream) {
    match del_channel(&data) {
        true => send_ack_ok(tcp_stream),
        false => no_handel(tcp_stream),
    };
}

fn store(data: String, tcp_stream: &TcpStream, reg_id: String) {
    let mut conn: PooledConn = create_conn();
    let message: Vec<String> = data.split('_').map(|s| s.to_string()).collect();

    let channel: String = message[0].to_owned();
    let message_type: String = message[1].to_owned();
    let encoded_message: String = message[2].to_owned();
    let message_hash: String = message[3].to_owned(); // will be used as the uuid in the database. messages will need a timestamp in them by default

    #[allow(unused)] // We use this to allow error checking before writing to the database
    let decoded_message: String = match hex::decode(&encoded_message) {
        Ok(data) => {
            let hex_decoded_data: String = match String::from_utf8(data) {
                Ok(string) => string,
                Err(e) => {
                    append_log(PROG, &format!("Error decoded hex string: {}", e));
                    panic!("{}", e);
                }
            };
            format!("{}", hex_decoded_data)
        }
        Err(e) => {
            append_log(PROG, &format!("Error decoded hex string: {}", e));
            panic!("{}", e);
        }
    };

    let commit_query: String = format!(
        r"INSERT INTO Artisan_Messenger.{} (uuid, message_type, message) VALUES ( '{}', '{}', '{}' )",
        &channel, message_hash, message_type, &encoded_message
    );

    // Check permissions and write to database
    match check_permission(&channel, &reg_id) {
        true => match conn.query_drop(commit_query) {
            Ok(_) => {
                append_log(PROG, "Message Saved");
                send_ack_dr(tcp_stream);
            }
            // if the query failes
            Err(e) => {
                no_handel(tcp_stream);
                append_log(PROG, &format!("Storing message failed with: {}", e));
            }
        },
        false => no_permission(tcp_stream),
    }
}

fn check_msg(channel: &str, reg: String, tcp_stream: &TcpStream) {
    let mut conn: PooledConn = create_conn();

    match check_permission(&channel, &reg) {
        true => {
            // read the latest message in the database
            let check_query: &str = &format!(
                r"SELECT uuid, message_type, message FROM Artisan_Messenger.{} WHERE processed = '0' LIMIT 1",
                channel
            );

            // Reding the data from db
            let packed_messages =
                conn.query_map(check_query, |(uuid, message_type, message)| Message {
                    uuid,
                    message_type,
                    message,
                });

            // repacking into an array of messages
            let messages: Option<Vec<Message>> = match packed_messages {
                Ok(message_array) => Some(message_array),
                Err(_) => None,
            };

            match messages {
                Some(mes) => {
                    for message in mes {
                        let message_data: String = message.message;

                        let message_integrity: String = message.uuid;
                        let new_integrity: String = create_hash(&message_data);

                        let hash_check: bool = new_integrity == message_integrity;
                        match hash_check {
                            true => {
                                send_ack_ds(message_data, tcp_stream);
                                // Marking message delivered
                                let delivered: String = format!(
                                    r"UPDATE Artisan_Messenger.{} SET processed = '1' WHERE uuid = '{}'",
                                    &channel, message_integrity
                                );
                                // Go lang logic here ? love it
                                match conn.query_drop(delivered) {
                                    Ok(_) => {
                                        append_log(PROG, &format!("Delivered {}", message_integrity))
                                    }
                                    Err(e) => append_log(
                                        PROG,
                                        &format!(
                                            "Couldn't mark {} as delivered got: {}",
                                            message_integrity, e
                                        ),
                                    ),
                                };
                            }
                            false => sec_fault(tcp_stream),
                        }
                    }
                }
                None => send_ack_ok(tcp_stream), // no data
            }
        }
        false => no_permission(tcp_stream),
    }
}

fn ack_msg(hash: String, tcp_stream: &TcpStream) {
    send_ack_ok(tcp_stream);
    match hash {
        _ => todo!(),
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
