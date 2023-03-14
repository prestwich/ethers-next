use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{ready, Context, Poll},
};

use ethers_pub_use::serde::{Deserialize, Serialize};
use jsonrpsee_types::ErrorObjectOwned;

use crate::{
    common::{Id, Request, RpcFuture, RpcOutcome},
    utils::{from_json, to_json_raw_value},
    Connection, TransportError,
};

pub enum CallState<T, Params> {
    Prepared {
        connection: T,
        method: &'static str,
        params: Params,
        id: Id<'static>,
    },
    AwaitingResponse {
        fut: RpcFuture,
    },
    Complete,
    Running,
}

impl<T, Params> CallState<T, Params> {
    pub fn new(
        connection: T,
        method: &'static str,
        params: Params,
        id: Id<'static>,
    ) -> CallState<T, Params> {
        Self::Prepared {
            connection,
            method,
            params,
            id,
        }
    }
}

impl<T, Params> CallState<T, Params>
where
    T: Connection + Unpin,
    Params: Serialize + Unpin,
{
    fn poll_prepared(&mut self, cx: &mut Context<'_>) -> Poll<RpcOutcome> {
        let this = std::mem::replace(self, CallState::Running);

        match this {
            CallState::Prepared {
                connection,
                method,
                params,
                id,
            } => {
                let params = to_json_raw_value(&params);
                if let Err(err) = params {
                    *self = CallState::Complete;
                    return Poll::Ready(Err(err));
                }
                let params = params.unwrap();
                let req = Request::owned(id, method, Some(params));
                let fut = connection.json_rpc_request(&req);
                *self = CallState::AwaitingResponse { fut };
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            _ => panic!(""),
        }
    }

    fn poll_awaiting(&mut self, cx: &mut Context<'_>) -> Poll<RpcOutcome> {
        let this = std::mem::replace(self, CallState::Running);
        match this {
            CallState::AwaitingResponse { mut fut } => {
                if let Poll::Ready(val) = fut.as_mut().poll(cx) {
                    *self = CallState::Complete;
                    return Poll::Ready(val);
                }
                *self = CallState::AwaitingResponse { fut };
                Poll::Pending
            }
            _ => panic!(""),
        }
    }
}

impl<T, Params> Future for CallState<T, Params>
where
    T: Connection + Unpin,
    Params: Serialize + Unpin,
{
    type Output = RpcOutcome;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = self.get_mut();
        match state {
            CallState::Prepared { .. } => state.poll_prepared(cx),
            CallState::AwaitingResponse { .. } => state.poll_awaiting(cx),
            _ => panic!("Polled in bad state"),
        }
    }
}

pub struct RpcCall<T, Params, Resp> {
    state: CallState<T, Params>,
    resp: PhantomData<fn() -> Resp>,
}

impl<T, Params, Resp> RpcCall<T, Params, Resp> {
    pub fn new(connection: T, method: &'static str, params: Params, id: Id<'static>) -> Self {
        Self {
            state: CallState::new(connection, method, params, id),
            resp: PhantomData,
        }
    }
}

impl<T, Params, Resp> Future for RpcCall<T, Params, Resp>
where
    T: Connection + Unpin,
    Params: Serialize + Unpin,
    Resp: for<'de> Deserialize<'de> + Unpin,
{
    type Output = Result<Result<Resp, ErrorObjectOwned>, TransportError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = Pin::new(&mut self.get_mut().state);
        let res = ready!(state.poll(cx));

        match res {
            Ok(Ok(val)) => Poll::Ready(from_json(val.get()).map(Result::Ok)),
            Ok(Err(err)) => Poll::Ready(Ok(Err(err))),
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
