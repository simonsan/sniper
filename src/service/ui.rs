use crate::{event_log, persistence::SharedPersistence, service::LoopService};
use anyhow::{format_err, Context, Result};
use axum::{handler::get, Router};
use tokio::{runtime::Runtime, sync::oneshot};

pub struct Ui {
    persistence: SharedPersistence,
    even_writer: event_log::SharedWriter,

    // cancels all tasks on read
    _runtime: Runtime,
    server_rx: oneshot::Receiver<Result<()>>,
}

async fn run_http_server() -> Result<()> {
    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    // run it with hyper on localhost:3000
    axum::Server::try_bind(&"0.0.0.0:3000".parse()?)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

impl Ui {
    pub fn new(
        persistence: SharedPersistence,
        even_writer: event_log::SharedWriter,
    ) -> Result<Self> {
        let runtime = Runtime::new()?;

        let (tx, rx) = oneshot::channel();

        runtime.spawn(async {
            tx.send(
                run_http_server()
                    .await
                    .with_context(|| format!("Failed to run http server")),
            )
            .expect("send to work");
        });

        Ok(Self {
            persistence,
            even_writer,
            _runtime: runtime,
            server_rx: rx,
        })
    }
}

impl LoopService for Ui {
    fn run_iteration<'a>(&mut self) -> Result<()> {
        // don't hog the cpu
        std::thread::sleep(std::time::Duration::from_millis(100));

        match self.server_rx.try_recv() {
            Ok(res) => res,
            Err(oneshot::error::TryRecvError::Empty) => Ok(()),
            Err(oneshot::error::TryRecvError::Closed) => {
                Err(format_err!("ui server died with leaving a response?!"))
            }
        }
    }
}
