use logging::append_log;
use mysql::prelude::Queryable;
use system::create_hash;

use crate::{database::create_conn, PROG};

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

pub fn register_to_channel(table: &str, client: &str) -> bool {
    let mut conn = create_conn();
    match conn.query_drop(format!(
        r"INSERT INTO Artisan_Messenger.{}_permission (uuid) VALUES ('{}')",
        table, client
    )) {
        Ok(_) => {
            append_log(PROG, &format!("Client {} registered", client));
            true
        }
        Err(e) => {
            append_log(
                PROG,
                &format!(
                    "Registering {} on {}_permission, FAILED: {}",
                    client, table, e
                ),
            );
            false
        }
    }
}

pub fn del_channel(table: &str) -> bool {
    let mut conn = create_conn();
    let drop_message: String = format!("DROP TABLE Artisan_Messenger.{}", table);
    let drop_permission: String = format!("DROP TABLE Artisan_Messenger.{}_permission", table);

    let drop_tuple = (
        conn.query_drop(drop_message),
        conn.query_drop(drop_permission),
    );

    match drop_tuple {
        (Ok(_), Ok(_)) => {
            append_log(
                PROG,
                &format!("The channel {} has been dropped sucessfully", table),
            );
            true
        }
        (Ok(_), Err(_)) => {
            append_log(
                PROG,
                &format!("The channel {} has been partially dropped", table),
            );
            false
        }
        (Err(_), Ok(_)) => {
            append_log(
                PROG,
                &format!("The channel {} has been partially dropped", table),
            );
            false
        }
        #[allow(non_snake_case)]
        (Err(E1), Err(E2)) => {
            append_log(
                PROG,
                &format!(
                    "The channel {} could not be dropped: \n {} \n {}",
                    table, E1, E2
                ),
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

// let hex_array: Vec<u8> = hex::decode(data).expect("Failed to decode");
// let ugly_data: String = String::from_utf8(hex_array).expect("Bad sequence");

// // Adding a system to diffrenciate the types of data
// // let data: Result<Email, serde_json::Error> = serde_json::from_str(&ugly_data);
