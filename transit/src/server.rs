use crate::proxy::App;
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use std::{error::Error, io, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinSet};

pub async fn serve(listener: TcpListener, app: Arc<App>) -> io::Result<()> {
    let maximum = app
        .snapshot
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .config
        .server
        .max_connections;
    let permits = Arc::new(Semaphore::new(maximum));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _=app.stopping.cancelled()=>break,
            outcome=connections.join_next(),if !connections.is_empty()=>{
                if outcome.is_some_and(|r|r.is_err()){tracing::error!(event="connection_task_panicked");}
            },
            accepted=listener.accept()=>{
                let (stream,_)=match accepted {Ok(accepted)=>accepted,Err(error)=>{tracing::error!(event="accept_failed",error_kind=?error.kind());return Err(error)}};
                let permit=match permits.clone().try_acquire_owned(){Ok(permit)=>permit,Err(_)=>{tracing::warn!(event="connection_limit_reached");drop(stream);continue}};
                let app=app.clone();
                connections.spawn(async move {
                    let _permit=permit;
                    let mut builder=Builder::new(TokioExecutor::new());
                    builder.http1().max_buf_size(64*1024).max_headers(128).timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(30));
                    builder.http2().max_header_list_size(64*1024).max_concurrent_streams(100);
                    let service=TowerToHyperService::new(app.router());
                    let connection=builder.serve_connection(TokioIo::new(stream),service);
                    tokio::pin!(connection);
                    tokio::select! {
                        result=&mut connection=>if let Err(error)=result{log_connection_error(error.as_ref());},
                        _=app.force_stop.cancelled()=>{},
                        _=app.stopping.cancelled()=>{
                            connection.as_mut().graceful_shutdown();
                            tokio::select!{
                                result=&mut connection=>if let Err(error)=result{log_connection_error(error.as_ref());},
                                _=app.force_stop.cancelled()=>{},
                            }
                        }
                    }
                });
            }
        }
    }
    drop(listener);
    while let Some(result) = connections.join_next().await {
        if result.is_err() {
            tracing::error!(event = "connection_task_panicked");
        }
    }
    Ok(())
}

fn log_connection_error(error: &(dyn Error + 'static)) {
    let mut source = Some(error);
    while let Some(error) = source {
        if let Some(error) = error.downcast_ref::<hyper::Error>()
            && (error.is_timeout() || error.is_closed() || error.is_incomplete_message())
        {
            tracing::debug!(
                event = "http_connection_closed",
                reason = "idle timeout or client disconnect; payload omitted"
            );
            return;
        }
        source = error.source();
    }
    tracing::warn!(
        event = "http_connection_failed",
        reason = "invalid HTTP or downstream transport failure; payload omitted"
    );
}
