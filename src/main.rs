pub mod commands;
pub mod database;
pub mod functions;
pub mod skel;

use {
    commands::{complex_processor, simple_processor},
    functions::sec_fault,
    logging::{append_log, start_log},
    skel::{Request, RequestCode, RequestData},
    std::{
        io::Read,
        net::{TcpListener, TcpStream},
        thread,
    },
    system::create_hash,
};

pub const PROG: &str = "IronPulse_server";

fn main() {
    start_log(PROG);

    let listen_addr = "0.0.0.0:9518"; // Change this to your desired address and port

    let tcp_listener = match TcpListener::bind(listen_addr) {
        Ok(tcp) => tcp,
        Err(e) => {
            append_log(PROG, &format!("Couldn't create tcp listener: {}", e));
            panic!("Listeing failed");
        }
    };

    append_log(PROG, &format!("Listening on {}", listen_addr));

    // Create a vector to store client handler threads
    let mut client_handlers = vec![];

    for stream_result in tcp_listener.incoming() {
        match stream_result {
            Ok(tcp_stream) => {
                let client_handlers_ref = &mut client_handlers;

                // Spawn a new thread to handle each client
                let handle = thread::spawn(move || {
                    handle_stream(tcp_stream);
                });

                client_handlers_ref.push(handle);
            }
            Err(err) => {
                append_log(PROG, &format!("Error accepting connection: {}", err));
                eprintln!("Check log");
                break;
            }
        }
    }

    // Wait for all client handler threads to finish
    for handle in client_handlers {
        handle.join().expect("Client handler thread panicked");
    }
}

fn handle_stream(mut tcp_stream: TcpStream) {
    // Input
    let mut request = String::new();
    tcp_stream
        .read_to_string(&mut request)
        .expect("Failed at reading the unix stream");

    // println!("Client Command: {}\nAck", request);
    // notice("Data recived");
    let request: Request = phrasing_request(request.clone()).unwrap();

    let integrity: bool = match &request {
        Request::Code(d) => d.integrity,
        Request::Data(d) => d.integrity,
    };

    if !integrity {
        sec_fault(&tcp_stream);
        return;
    }

    let command: String = match &request {
        Request::Code(data) => data.command.to_string(),
        Request::Data(data) => data.command.to_string(),
    };

    let command_string: Option<String> = match &request {
        Request::Code(_) => None,
        Request::Data(data) => Some(data.data.to_string()),
    };

    let registration_id: String = match &request {
        Request::Code(data) => data.requestid.to_string(),
        Request::Data(data) => data.requestid.to_string(),
    };

    // processing the code
    match command_string {
        Some(d) => complex_processor(&command, d, registration_id, &tcp_stream),
        _ => simple_processor(&command, registration_id, &tcp_stream),
    }
}

fn phrasing_request(data: String) -> Option<Request> {
    let split_data: Vec<String> = data.split(',').map(|s| s.to_string()).collect();
    let split_request: Vec<String> = split_data[0].split('/').map(|s| s.to_string()).collect();

    let request_command: String = split_request[0].to_string();
    let request_string: Option<String> = if split_request.len() > 1 {
        Some(split_request[1].to_owned())
    } else {
        None
    };

    let registration_id: String = split_data[1].to_string();
    let integrity_source: String = split_data[2].to_string();
    let integrity_match: String = create_hash(&format!("{}{}", split_data[1], split_data[0]));

    // Running the integrity Testing
    let integrity_check = if integrity_match == integrity_source {
        true
    } else {
        false
    };

    match request_string {
        Some(d) => {
            let request_data: RequestData = RequestData {
                command: request_command,
                data: d,
                requestid: registration_id,
                integrity: integrity_check,
            };
            return Some(Request::Data(request_data));
        }
        _ => {
            let request_code: RequestCode = RequestCode {
                command: request_command,
                requestid: registration_id,
                integrity: integrity_check,
            };
            return Some(Request::Code(request_code));
        }
    }
}
