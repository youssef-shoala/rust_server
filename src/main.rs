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
use form_urlencoded;
use dashmap::DashMap;



#[derive(Serialize, Deserialize, Debug)]
struct Count { 
    count: u64, 
}

#[derive(Serialize, Deserialize, Debug)]
struct SongPostDetails {
    title: String,
    artist: String,
    genre: String,
}
#[derive(Serialize, Deserialize, Debug)]
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
    cache: std::sync::Arc<DashMap<String, Vec<Song>>>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {


    match (req.method(), req.uri().path()) {


        // If the request is a GET request to the root path, return a welcome message
        (&Method::GET, "/") => Ok(Response::new(full(
            "Welcome to the Rust-powered web server!", 
        ))), 


        // If the request is a GET request to /count, return a count of the number of requests and increment by 1 
        (&Method::GET, "/count") => {

//            let result: Result<Response<BoxBody<hyper::body::Bytes, hyper::Error>>, sled::transaction::TransactionError> = db.transaction(|db| {
//                if let Some(count) = db.get(b"count")? {
//                    let count_byte_slice = count.as_ref();
//                    let deserialized_count: Count = bincode::deserialize(count_byte_slice).unwrap();
//                    let new_count = Count { count: deserialized_count.count + 1 };
//                    let serialized_new_count = bincode::serialize(&new_count).unwrap();
//                    db.insert(b"count", serialized_new_count)?;
//                    Ok(Response::new(full(format!("Visit count: {}", deserialized_count.count))))
//                } else {
//                    Ok(Response::new(full("Visit count not found")))
//                }
//            });
//
//            match result {
//                Ok(response) => Ok(response),
//                Err(_) => Ok(Response::new(full("Visit count error"))),
//            }

            let count = db.fetch_and_update(b"count", increment).unwrap().map(|iv| iv.to_vec()).unwrap();
            let deserialized_count = u64::from_be_bytes(count.as_slice().try_into().unwrap());
            Ok(Response::new(full(format!("Visit count: {:?}", deserialized_count + 1))))

        },

    
        // If the request is a GET requst to /songs/play/{song_id} increment the play count and return the updated json body
        (&Method::GET, path) if path.starts_with("/songs/play/") => {
            let song_id_str = path.trim_start_matches("/songs/play/");
            if let Ok(song_id) = song_id_str.parse::<u64>() {
                let result: Result<Response<BoxBody<hyper::body::Bytes, hyper::Error>>, sled::transaction::TransactionError> = db.transaction(|db| {
                    if let Some(song_serialized) = db.get(format!("song_{}", song_id).as_bytes())? {
                        let mut song: Song = bincode::deserialize(&song_serialized).unwrap();
                        song.play_count += 1;
                        let serialized_song = bincode::serialize(&song).unwrap();
                        db.insert(format!("song_{}", song_id).as_bytes(), serialized_song)?;
                        let json_song = serde_json::to_string(&song).unwrap();
                        Ok(Response::new(full(json_song)))
                    } else {
                        let error_response = serde_json::json!({ "error": "Song not found" });
                        Ok(Response::new(full(error_response.to_string())))
                    }
                });

                match result {
                    Ok(response) => Ok(response),
                    Err(_) => Ok(Response::new(full("Error incrementing play count"))),
                }
            } else {
                let error_response = serde_json::json!({ "error": "Invalid song ID" });
                Ok(Response::new(full(error_response.to_string())))
            }
        },


        // If the requst is a GET request to /songs/search with parameters? title, artist, genre, return a list of songs that match the search criteria
        (&Method::GET, "/songs/search") => {
            let query = req.uri().query().unwrap();
            let query_pairs = form_urlencoded::parse(query.as_bytes());
            let mut title = String::new();
            let mut artist = String::new();
            let mut genre = String::new();
            for (key, value) in query_pairs {
                match key.as_ref() {
                    "title" => title = value.to_string(),
                    "artist" => artist = value.to_string(),
                    "genre" => genre = value.to_string(),
                    _ => (),
                }
            }
            //let title_vec = title.clone().as_bytes().to_vec();
            //let artist_vec = artist.clone().as_bytes().to_vec();
            //let genre_vec = genre.clone().as_bytes().to_vec();
            //println!("\nSearching for: Title: {:?}, Artist: {:?}, Genre: {:?}", title, artist, genre);
            //println!("\nSearching for: Title: {:?}, Artist: {:?}, Genre: {:?}", title_vec, artist_vec, genre_vec);

            // add search cache for performace
            let cache_key = format!("{}{}{}", title, artist, genre);
            if let Some(cached_songs) = cache.get(&cache_key) {
                let songs = cached_songs.value();
                let json_songs = serde_json::to_string(songs).unwrap();
                return Ok(Response::new(full(json_songs)));
            }

//            //change such that it searches by value not key
//            //could make it so that the bytes of the search term are directldy compared to bytes in db
//            let songs_vec = db.iter()
//                .filter_map(|item| {
//                    let (key, value) = item.unwrap();
//                    println!("\nKey: {:?} Value: {:?}\n", key, value);
//
//                    let haystack = value;
//                    //split the haystack into fragments that represent the song id, title, artist, genre, and play count
//                    let haystacks = [haystack];
//                    let needles = [&title_vec, &artist_vec, &genre_vec];
//
//                    for haystack in haystacks {
//                        let lowercase_haystack = bytes_to_lowercase(haystack.to_vec().as_slice());
//                        for needle in needles {
//                            if needle.is_empty() {
//                                continue;
//                            }
//                            let lowercase_needle = bytes_to_lowercase(needle);
//                            println!("Needle: {:?}", needle);
//                            let curr_needle = lowercase_needle.as_slice();
//                            if lowercase_haystack.windows(curr_needle.len()).any(|window| window == curr_needle) {
//                                println!("Found needle: {:?}", curr_needle);
//                                return Some(haystack.to_vec());
//                            }
//                        }
//                           
//                    }
//
//                    None
//
//                    //let key_str = String::from_utf8(key.to_vec()).unwrap();
//                    //if key_str.starts_with("song_") {
//                    //    if (title.is_empty() || key_str.contains(title.as_str())) && (artist.is_empty() || key_str.contains(artist.as_str())) && (genre.is_empty() || key_str.contains(genre.as_str())) {
//                    //        let song: Song = bincode::deserialize(value.as_ref()).unwrap();
//                    //        Some(song)
//                    //    } else {
//                    //        None
//                    //    }
//                    //} else {
//                    //    None
//                    //}
//                })
//                .collect::<Vec<Vec<u8>>>();


//            let songs = db.iter()
//                .filter_map(|item| {
//                    let (key, value) = item.unwrap();
//                    let key_str = String::from_utf8(key.to_vec()).unwrap();
//                    let value_str = String::from_utf8(value.to_vec()).unwrap();
//                    if key_str.starts_with("song_") {
//                        let song: Song = bincode::deserialize(value_str.as_bytes()).unwrap();
//                        if (title.is_empty() || song.title.to_lowercase().contains(&title.to_lowercase())) && (artist.is_empty() || song.artist.to_lowercase().contains(&artist.to_lowercase())) && (genre.is_empty() || song.genre.to_lowercase().contains(&genre.to_lowercase())) {
//                            Some(song)
//                        } else {
//                            None
//                        }
//                    } else {
//                        None
//                    }
//                })
//                .collect::<Vec<Song>>();
//
//            let json_songs = serde_json::to_string(&songs).unwrap();
//            println!("Converted JSON: {:?}", json_songs);
//            cache.insert(cache_key, songs);
//            println!("Cache size: {:?}", cache.len());
//            Ok(Response::new(full(json_songs)))

            




            //let keys = (0..u64::from_be_bytes(db.get(b"song_id").unwrap().unwrap().to_vec().try_into().unwrap())).collect::<Vec<u64>>();
            //let songs = keys
            //    .par_iter()
            //    .filter_map(|song_id| {
            //        let song_serialized = db.get(format!("song_{}", song_id).as_bytes()).unwrap().expect("Error").to_vec();
            //        let song: Song = bincode::deserialize(song_serialized.as_slice()).unwrap();
            //        if (title.is_empty() || song.title.to_lowercase().contains(&title.to_lowercase())) && (artist.is_empty() || song.artist.to_lowercase().contains(&artist.to_lowercase())) && (genre.is_empty() || song.genre.to_lowercase().contains(&genre.to_lowercase())) {
            //            Some(song)
            //        } else {
            //            None
            //        }
            //    })
            //    .collect::<Vec<Song>>();
            //let json_songs = serde_json::to_string(&songs).unwrap();
            //Ok(Response::new(full(json_songs)))



            //let result: Result<Response<BoxBody<hyper::body::Bytes, hyper::Error>>, sled::transaction::TransactionError> = db.transaction(|db| {
            //    let mut songs: Vec<Song> = Vec::new();
            //    let mut song_id = u64::from_be_bytes(db.get(b"song_id").unwrap().unwrap().to_vec().try_into().unwrap());
            //    while song_id != 0 {
            //        song_id -= 1;
            //        let song_serialized = db.get(format!("song_{}", song_id).as_bytes()).unwrap().expect("Error").to_vec();
            //        let song: Song = bincode::deserialize(song_serialized.as_slice()).unwrap();
            //        if (title.is_empty() || song.title.to_lowercase().contains(&title.to_lowercase())) && (artist.is_empty() || song.artist.to_lowercase().contains(&artist.to_lowercase())) && (genre.is_empty() || song.genre.to_lowercase().contains(&genre.to_lowercase())) {
            //            songs.push(song);
            //        }
            //    }

            //    //println!("Songs found: {:?}", songs.len());
            //    //for song in &songs {
            //    //    println!("Song id: {:?} Tile: {:?} Artist: {:?} Genre: {:?} Play count: {:?}", song.id, song.title, song.artist, song.genre, song.play_count);
            //    //}

            //    //create a json body that contains all songs in songs
            //    let json_songs = serde_json::to_string(&songs).unwrap();
            //    //let serialized_songs = bincode::serialize(&songs).unwrap();
            //    Ok(Response::new(full(json_songs)))
            //});

            //match result {
            //    Ok(response) => Ok(response),
            //    Err(_) => Ok(Response::new(full("Search error"))),
            //}



            let mut songs: Vec<Song> = Vec::new();
            let mut song_id = u64::from_be_bytes(db.get(b"song_id").unwrap().unwrap().to_vec().try_into().unwrap());
            println!("Song id: {:?}", song_id);
            while song_id != 0 {
                let song_full_id = format!("song_{}", song_id);
                let song_serialized = db.get(song_full_id.as_bytes()).unwrap().expect("Error").to_vec();
                let song: Song = bincode::deserialize(song_serialized.as_slice()).unwrap();
                if (title.is_empty() || song.title.to_lowercase().contains(&title.to_lowercase())) && (artist.is_empty() || song.artist.to_lowercase().contains(&artist.to_lowercase())) && (genre.is_empty() || song.genre.to_lowercase().contains(&genre.to_lowercase())) {
                    songs.push(song);
                }
                song_id -= 1;
            }
            let json_songs = serde_json::to_string(&songs).unwrap();
            Ok(Response::new(full(json_songs)))

        },


        // If the request is a POST request to /songs/new with a JSON body, add a new song to the database
        (&Method::POST, "/songs/new") => {
            let frame_stream = req.into_body().map_frame(move |frame| {
                let frame = if let Ok(data) = frame.into_data() {
                    let json_vec = data.to_vec();
                    let json_slice = json_vec.as_slice();
                    let song_post_details: SongPostDetails = serde_json::from_slice(json_slice).unwrap();

                    let result: Result<Song, sled::transaction::TransactionError<sled::Error>> = db.transaction(|db| {

                        let id = u64::from_be_bytes(db.get(b"song_id").unwrap().unwrap().to_vec().try_into().unwrap());
                        let next_id = id + 1;
                        db.insert(b"song_id", next_id.to_be_bytes().to_vec()).unwrap();

                        let song: Song = Song {
                            id: next_id,
                            title: song_post_details.title.clone(),
                            artist: song_post_details.artist.clone(),
                            genre: song_post_details.genre.clone(),
                            play_count: 0,
                        };
                        let serialized_song = bincode::serialize(&song).unwrap();
                        //let full_id = format!("song_{}_{}_{}_{}_{}", next_id, song.title, song.artist, song.genre, song.play_count);
                        let full_id = format!("song_{}", next_id);
                        //println!("\nSong id: {:?}\n Tile: {:?}\n Artist: {:?}\n Genre: {:?}\n Play count: {:?}", song.id, song.title, song.artist, song.genre, song.play_count);
                        db.insert(full_id.as_bytes(), serialized_song).unwrap();
                        Ok(song)

                    });

                    let response = match result {
                        Ok(song) => {
                            //println!("Song Added: {:?}", song);
                            let json_songs = serde_json::to_string(&song).unwrap();
                            Bytes::from(json_songs)
                        },
                        Err(_) => {
                            Bytes::new()
                        }
                    };
                    response
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



//fn bytes_to_lowercase(bytes: &[u8]) -> Vec<u8> {
//    bytes.iter().map(|byte| byte.to_ascii_lowercase()).collect::<Vec<u8>>()
//}
fn increment(old: Option<&[u8]>) -> Option<Vec<u8>> {
    let number = match old {
        Some(bytes) => {
            let array: [u8; 8] = bytes.to_vec().try_into().unwrap();
            let number = u64::from_be_bytes(array);
            number + 1
        },
        None => 0,
    };
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

    println!("Initializing count to 0.");
    let count = Count { count: 0 };
    let serialized_count = bincode::serialize(&count)?;
    let zero: u64 = 0;
    let serialized_zero = zero.to_be_bytes().to_vec();
    db.insert(b"count", serialized_count)?;

    let cache: std::sync::Arc<DashMap<String, Vec<Song>>> = std::sync::Arc::new(DashMap::new());

    if !db.was_recovered() {
        println!("Database was not recovered correctly, initializing to empty.");
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
        let cache = cache.clone();
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(move |req| handle_request(req, db_clone.clone(), cache.clone())))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
