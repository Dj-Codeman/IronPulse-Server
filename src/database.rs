use core::panic;

use logging::append_log;
use mysql::*;
use pretty::halt;
use recs::retrive;
use std::fs;
use system::del_file;

use crate::{skel::Database, PROG};

pub fn create_pool() -> Pool {
    let (db_username, db_password, db_host, database) = match read_credentials() {
        Some(db) => (db.username, db.password, db.hostaddr, db.database),
        None => {
            halt("No database credentials found, use rescs and store database credentials as Ironpulse, database, /tmp/database.dk");
            panic!()
        }
    };

    let url: String = format!(
        "mysql://{}:{}@{}:3306/{}",
        db_username, db_password, db_host, database
    );
    let pool: Pool =
        Pool::new_manual(4, 8, url).expect("Failed to create the database connection pool");
    pool
}

pub fn create_conn() -> PooledConn {
    return create_pool().get_conn().unwrap();
}

// ! make a struct for the database creds
fn read_credentials() -> Option<Database> {
    let owner: String = String::from("ironpulse");
    let name: String = String::from("database");
    match retrive(owner, name) {
        Some(bol) => match bol {
            true => {
                // retrive the data
                let database_creds: String = match fs::read_to_string("/tmp/database.dk") {
                    Ok(data) => data,
                    Err(e) => {
                        append_log(PROG, &format!("Error reading database credentials: {}", e));
                        panic!();
                    }
                };
                del_file("/tmp/database.dk");
                // unpack and map
                let data: Vec<String> = database_creds.split('/').map(|s| s.to_string()).collect();
                return Some(Database {
                    username: data[0].to_owned(),
                    password: data[1].to_owned(),
                    hostaddr: data[2].to_owned(),
                    database: data[3].to_owned(),
                });
            }
            false => {
                append_log(PROG, "Could not read database credentials from recs");
                panic!("Check logs");
            }
        },
        None => panic!("Unable to communicate with recs"),
    }
}
