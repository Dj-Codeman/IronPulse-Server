use {
    crate::database::create_conn,
    crate::functions::{
        check_permission, create_message_table, create_permission_table, no_handel, no_permission,
        payload_integrity, sec_fault, send_ack_dr, send_ack_ds, send_ack_ok,
    },
    crate::skel::Message,
    crate::PROG,
    logging::append_log,
    mysql::{prelude::Queryable, PooledConn},
    std::net::TcpStream,
    system::create_hash,
};

pub fn complex_processor(command: &str, data: String, register_id: String, tcp_stream: &TcpStream) {
    match command {
        "RegisterChannel" => register_channel(&data, register_id, tcp_stream),
        "DeleteChannel" => delete_channel(&data, tcp_stream),
        "CreateChannel" => create_channel(&data, register_id, tcp_stream),
        "Store" => match payload_integrity(&data) {
            true => store(data, tcp_stream, register_id),
            false => no_handel(tcp_stream),
        },
        "Check" => {
            check_msg(&data, &register_id, tcp_stream);
            append_log(
                PROG,
                &format!("Client {} has checked meessages", register_id),
            );
        }
        "Ack" => {
            append_log(
                PROG,
                &format!(
                    "Ack recived by {}, with this data hash: {}",
                    register_id,
                    create_hash(&data)
                ),
            );
            ack_msg(&data, register_id, tcp_stream);
        }
        &_ => no_handel(tcp_stream),
    }
}

pub fn simple_processor(command: &str, _: String, tcp_stream: &TcpStream) {
    match command {
        &_ => no_handel(tcp_stream),
    }
}

fn create_channel(data: &str, _reg: String, tcp_stream: &TcpStream) {
    let magic: (bool, bool) = (create_message_table(data), create_permission_table(data));

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
    let mut conn = create_conn();
    let result: bool = match conn.query_drop(format!(
        r"INSERT INTO Artisan_Messenger.{}_permission (uuid) VALUES ('{}')",
        data, reg
    )) {
        Ok(_) => {
            append_log(PROG, &format!("Client {} registered", reg));
            true
        }
        Err(e) => {
            append_log(
                PROG,
                &format!("Registering {} on {}_permission, FAILED: {}", reg, data, e),
            );
            false
        }
    };

    match result {
        true => send_ack_dr(tcp_stream),
        false => no_handel(tcp_stream),
    };
}

fn delete_channel(data: &str, tcp_stream: &TcpStream) {
    let mut conn = create_conn();
    let drop_message: String = format!("DROP TABLE Artisan_Messenger.{}", data);
    let drop_permission: String = format!("DROP TABLE Artisan_Messenger.{}_permission", data);

    let drop_tuple = (
        conn.query_drop(drop_message),
        conn.query_drop(drop_permission),
    );

    let result: bool = match drop_tuple {
        (Ok(_), Ok(_)) => {
            append_log(
                PROG,
                &format!("The channel {} has been dropped sucessfully", data),
            );
            true
        }
        (Ok(_), Err(_)) => {
            append_log(
                PROG,
                &format!("The channel {} has been partially dropped", data),
            );
            false
        }
        (Err(_), Ok(_)) => {
            append_log(
                PROG,
                &format!("The channel {} has been partially dropped", data),
            );
            false
        }
        #[allow(non_snake_case)]
        (Err(E1), Err(E2)) => {
            append_log(
                PROG,
                &format!(
                    "The channel {} could not be dropped: \n {} \n {}",
                    data, E1, E2
                ),
            );
            false
        }
    };

    match result {
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
            Err(e) => {
                append_log(PROG, &format!("Storing message failed with: {}", e));
                no_handel(tcp_stream);
            }
        },
        false => {
            append_log(
                PROG,
                &format!(
                    "Permission denied while acessing channel {}, by client {}",
                    channel, reg_id
                ),
            );
            no_permission(tcp_stream);
        }
    }
}

fn check_msg(channel: &str, reg: &str, tcp_stream: &TcpStream) {
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
                                // Marking message deliveredI
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

fn ack_msg(data: &str, _: String, tcp_stream: &TcpStream) {
    // No perm check because we mark done based on the message hex
    let mut conn: PooledConn = create_conn();
    let data_array: Vec<String> = data.split('_').map(|s| s.to_string()).collect();

    let channel: String = String::from(data_array[0].clone());
    let message: String = String::from(data_array[1].clone());

    let delivered: String = format!(
        r"UPDATE Artisan_Messenger.{} SET processed = '1' WHERE message = '{}'",
        channel, message
    );

    match conn.query_drop(delivered) {
        Ok(_) => {
            append_log(PROG, &format!("Delivered {}", message));
            send_ack_ok(tcp_stream);
        }
        Err(e) => {
            append_log(
                PROG,
                &format!("Couldn't mark {} as delivered got: {}", message, e),
            );
            no_handel(tcp_stream);
        }
    };
}
