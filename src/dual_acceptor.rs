use std::future::Future;
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::pin::Pin;
use std::slice;
use std::task::Context;
use std::task::Poll;

use axum_server::accept::Accept;
use axum_server::tls_rustls::RustlsAcceptor;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Server;
use hyper::server::conn::AddrStream;
use pin_project_lite::pin_project;
use tokio::io::ReadBuf;
use tokio_rustls::server::TlsStream;
use tokio_util::either::Either;

#[must_use]
pub fn bind_dual(addr: SocketAddr, config: RustlsConfig) -> Server<DualAcceptor> {
    let acceptor = DualAcceptor::new(config);

    Server::bind(addr).acceptor(acceptor)
}

#[derive(Debug, Clone)]
pub struct DualAcceptor {
    rustls: RustlsAcceptor,
}

impl DualAcceptor {
    #[must_use]
    pub fn new(config: RustlsConfig) -> Self {
        Self {
            rustls: RustlsAcceptor::new(config),
        }
    }
}

impl<Service> Accept<AddrStream, Service> for DualAcceptor {
    type Stream = Either<TlsStream<AddrStream>, AddrStream>;
    type Service = Service;
    type Future = DualAcceptorFuture<Service>;

    fn accept(&self, stream: AddrStream, service: Service) -> Self::Future {
        DualAcceptorFuture::new(stream, service, self.rustls.clone())
    }
}

pin_project! {
    #[project = DualAcceptorFutureProj]
    pub struct DualAcceptorFuture<Service> {
        #[pin]
        inner: DualAcceptorFutureInner<Service>,
    }
}

pin_project! {
    #[project = DualAcceptorFutureInnerProj]
    enum DualAcceptorFutureInner<Service> {
        Peek {
            inner: Option<PeekInner<Service>>,
        },
        Https {
            #[pin]
            future: <RustlsAcceptor as Accept<AddrStream, Service>>::Future
        },
    }
}

struct PeekInner<Service> {
    stream: AddrStream,
    service: Service,
    rustls: RustlsAcceptor,
}

impl<Service> DualAcceptorFuture<Service> {
    const fn new(stream: AddrStream, service: Service, rustls: RustlsAcceptor) -> Self {
        Self {
            inner: DualAcceptorFutureInner::Peek {
                inner: Some(PeekInner {
                    stream,
                    service,
                    rustls,
                }),
            },
        }
    }
}

impl<Service> DualAcceptorFutureProj<'_, Service> {
    fn upgrade(&mut self, future: <RustlsAcceptor as Accept<AddrStream, Service>>::Future) {
        self.inner.set(DualAcceptorFutureInner::Https { future });
    }
}

impl<Service> Future for DualAcceptorFuture<Service> {
    type Output = io::Result<(Either<TlsStream<AddrStream>, AddrStream>, Service)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        // After successfully peeking, continue without unnecessary yielding.
        loop {
            match this.inner.as_mut().project() {
                DualAcceptorFutureInnerProj::Peek { inner } => {
                    let peek = inner.as_mut().expect("polled again after `Poll::Ready`");

                    let mut byte = 0;
                    let mut buffer = ReadBuf::new(slice::from_mut(&mut byte));

                    match peek.stream.poll_peek(cx, &mut buffer) {
                        // If `MSG_PEEK` returns `0`, the socket was closed.
                        Poll::Ready(Ok(0)) => {
                            return Poll::Ready(Err(ErrorKind::UnexpectedEof.into()))
                        }
                        Poll::Ready(Ok(_)) => {
                            let PeekInner {
                                stream,
                                service,
                                rustls,
                            } = inner.take().expect("`inner` was already consumed");

                            // The first byte in the TLS protocol is always `0x16`.
                            if byte == 0x16 {
                                this.upgrade(rustls.accept(stream, service));
                            } else {
                                return Poll::Ready(Ok((Either::Right(stream), service)));
                            }
                        }
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                DualAcceptorFutureInnerProj::Https { future } => {
                    return future
                        .poll(cx)
                        .map_ok(|(stream, service)| (Either::Left(stream), service))
                }
            }
        }
    }
}
