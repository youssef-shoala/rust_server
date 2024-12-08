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

use sled;
use bincode;
use serde::{Serialize, Deserialize};
use serde_json;



#[derive(Serialize, Deserialize)]
struct Count { 
    count: u64, 
}

#[derive(Serialize, Deserialize)]
struct SongPostDetails {
    title: String,
    artist: String,
    genre: String,
}
struct Song {
    id: u64,
    title: String,
    artist: String,
    genre: String,
    play_count: u64,
}



//async fn hello(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
//    Ok(Response::new(Full::new(Bytes::from("Welcome to the Rust-powered web server!"))))
//}

// Define the routes and the response for each route
async fn handle_request(
    req: Request<hyper::body::Incoming>, 
    db: sled::Db,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {


    match (req.method(), req.uri().path()) {


        // If the request is a GET request to the root path, return a welcome message
        (&Method::GET, "/") => Ok(Response::new(full(
            "Welcome to the Rust-powered web server!", 
        ))), 


        // If the request is a GET request to /count, return a count of the number of requests and increment by 1 
        (&Method::GET, "/count") => {

            let result: Result<Response<BoxBody<hyper::body::Bytes, hyper::Error>>, sled::transaction::TransactionError> = db.transaction(|db| {
                if let Some(count) = db.get(b"count")? {
                    let count_byte_slice = count.as_ref();
                    let deserialized_count: Count = bincode::deserialize(count_byte_slice).unwrap();
                    let new_count = Count { count: deserialized_count.count + 1 };
                    let serialized_new_count = bincode::serialize(&new_count).unwrap();
                    db.insert(b"count", serialized_new_count)?;
                    Ok(Response::new(full(format!("Visit count: {}", deserialized_count.count))))
                } else {
                    Ok(Response::new(full("Visit count not found")))
                }
            });

            match result {
                Ok(response) => Ok(response),
                Err(_) => Ok(Response::new(full("Visit count error"))),
            }

        },

        // If the request is a POST request to /songs/new with a JSON body, add a new song to the database
        (&Method::POST, "/songs/new") => {
            let frame_stream = req.into_body().map_frame(move |frame| {
                let frame = if let Ok(data) = frame.into_data() {
                    let json_vec = data.to_vec();
                    let json_slice = json_vec.as_slice();
                    let song_post_details: SongPostDetails = serde_json::from_slice(json_slice).unwrap();
                    let id = u64::from_be_bytes(db.fetch_and_update(
                        b"song_id", 
                        increment)
                        .unwrap().expect("error").to_vec().try_into().unwrap());

                    let song: Song = Song {
                        id: id,
                        title: song_post_details.title,
                        artist: song_post_details.artist,
                        genre: song_post_details.genre,
                        play_count: 0,
                    };

                    println!("\nSong id: {:?}\n Tile: {:?}\n Artist: {:?}\n Genre: {:?}\n Play count: {:?}", song.id, song.title, song.artist, song.genre, song.play_count);
                    data
                } else {
                    Bytes::new()
                };
                Frame::data(frame)
            });

            Ok(Response::new(frame_stream.boxed()))
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



fn increment(old: Option<&[u8]>) -> Option<Vec<u8>> {
    let number = match old {
        Some(bytes) => {
            let array: [u8; 8] = bytes.try_into().unwrap();
            let number = u64::from_be_bytes(array);
            number + 1
        },
        None => 0,
    };

    println!("Incremented number: {:?}", number);
    Some(number.to_be_bytes().to_vec())

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

    // stateful db management
    let db_path = "/workspaces/web-server-assignment-4/server/src/data/db";

    let db = sled::open(db_path)?;

    if !db.was_recovered() {
        println!("No count found in database, initializing to 0.");
        let count = Count { count: 0 };
        let serialized_count = bincode::serialize(&count)?;
        let zero: u64 = 0;
        let serialized_zero = zero.to_be_bytes().to_vec();
        db.insert(b"count", serialized_count)?;
        println!("No songs found in database, initializing to empty.");
        db.insert(b"song_id", serialized_zero)?;
    }

    //println!("Tree names: {:?}", db.tree_names());
    //println!("Tree names: {:?}", db.tree_names().iter().map(|name| String::from_utf8(name.to_vec()).unwrap()).collect::<Vec<_>>());
    //println!("Size on disk: {:?}", db.size_on_disk());
    //println!("Size on disk: {:?}", db.size_on_disk().iter().map(|(name, size)| (String::from_utf8(name.to_vec()).unwrap(), size)).collect::<Vec<_>>());
    //println!("Count on server start: {:?}", db.get(b"count").unwrap().unwrap().to_vec().iter()); 



    // Set server listening on localhost:8080
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr).await?;
    println!("The server is currently listening on localhost:8080.");
    loop {
        let (stream, _) = listener.accept().await?;
        // Use an adapter to access something implementing `tokio::io` traits as if they implement `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        let db_clone = db.clone();
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(move |req| handle_request(req, db_clone.clone())))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
