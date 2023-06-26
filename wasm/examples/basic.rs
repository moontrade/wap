use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use log::info;
use ntex::http::header::HeaderValue;
use ntex::http::{HttpService, Response};
use ntex::{server::Server, time::Seconds, util::Ready};
use std::{env, io};

#[ntex::main]
async fn main() -> io::Result<()> {
    env::set_var("RUST_LOG", "ntex=trace,hello_world=info");
    env_logger::init();

    Server::build()
        .bind("hello-world", "127.0.0.1:8080", |_| {
            HttpService::build()
                .client_timeout(Seconds(1))
                .disconnect_timeout(Seconds(1))
                .finish(|_req| {
                    info!("{:?}", _req);
                    let mut res = Response::Ok();
                    res.header("x-head", HeaderValue::from_static("dummy value!"));
                    Ready::Ok::<_, io::Error>(res.body("Hello world!"))
                })
        })?
        .workers(1)
        .run()
        .await
}
