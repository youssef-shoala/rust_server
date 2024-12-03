use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use http_body_util::Empty;
use hyper::body::Bytes;
use hyper::body::Frame;
use hyper::{Method, StatusCode}; 
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use sled::Db;
use std::sync::Arc;

//async fn hello(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
//    Ok(Response::new(Full::new(Bytes::from("Welcome to the Rust-powered web server!"))))
//}

async fn echo(
    req: Request<hyper::body::Incoming>, 
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    match (req.method(), req.uri().path()) {

        // If the request is a GET request to the root path, 
        //      return a welcome message
        (&Method::GET, "/") => Ok(Response::new(full(
            "Welcome to the Rust-powered web server!", 
        ))), 

        // If the request is a get request to /count, 
        //      return a count of the number of requests and increment by 1 
        (&Method::GET, "/count") => {
            // Read the count from the file 
            let data_path = "/workspaces/web-server-assignment-4/server/src/data/count.txt";
            let count: u32 = tokio::fs::read_to_string(data_path).await
                .expect("Failed to read count from file")
                .trim()
                .parse()
                .expect("Failed to parse count as u32");

            // Increment the count by 1
            let count = count + 1;

            // Write the new count to the file
            tokio::fs::write(data_path, count.to_string()).await
                .expect("Failed to write to file");

            // Return the count as a response
            Ok(Response::new(full(format!("Visit count: {}", count))))
        },

        // If the request is a POST request to /echo/uppercase, convert the body to uppercase
        (&Method::POST, "/echo/uppercase") => {
            // Map this body's frame to a different type
            let frame_stream = req.into_body().map_frame(|frame| {
                let frame = if let Ok(data) = frame.into_data() {
                    // Convert every byte in every Data frame to uppercase
                    data.iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect::<Bytes>()
                } else {
                    Bytes::new()
                };

                Frame::data(frame)
            });

            Ok(Response::new(frame_stream.boxed()))
        },

        // If the request is not a valid request, return a 404 Not Found
        _ => {
            let mut not_found = Response::new(empty());
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never{})
        .boxed()
}
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never{})
        .boxed()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    // We create a TcpListener and bind it to 127.0.0.1:8080
    let listener = TcpListener::bind(addr).await?;

    // We start a loop to continuously accept incoming connections
    println!("The server is currently listening on localhost:8080.");
    loop {

        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(echo))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
