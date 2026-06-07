#[cfg(any(test, feature = "test-support"))]
pub mod test;

pub mod http;
pub mod user;

use anyhow::{anyhow, Result};
use futures::{future::LocalBoxFuture, FutureExt, StreamExt};
use gpui::{
    actions, serde_json::Value, AnyModelHandle, AnyViewHandle, AnyWeakModelHandle,
    AnyWeakViewHandle, AppContext, AppVersion, AsyncAppContext, Entity, ModelHandle,
    MutableAppContext, Task, View, ViewContext, ViewHandle,
};
use http::HttpClient;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use postage::watch;
use rand::prelude::*;
use rpc::proto::{AnyTypedEnvelope, EntityMessage, EnvelopedMessage, PeerId, RequestMessage};
use settings::TelemetrySettings;
use std::{
    any::TypeId,
    collections::HashMap,
    future::Future,
    marker::PhantomData,
    path::PathBuf,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
use thiserror::Error;
use util::{ResultExt, TryFutureExt};

pub use rpc::*;
pub use user::*;

lazy_static! {
    pub static ref ZED_APP_VERSION: Option<AppVersion> = std::env::var("ZED_APP_VERSION")
        .ok()
        .and_then(|v| v.parse().ok());
    pub static ref ZED_APP_PATH: Option<PathBuf> =
        std::env::var("ZED_APP_PATH").ok().map(PathBuf::from);
}

pub const INITIAL_RECONNECTION_DELAY: Duration = Duration::from_millis(100);

actions!(client, [SignIn, SignOut]);

pub fn init(client: Arc<Client>, cx: &mut MutableAppContext) {
    cx.add_global_action({
        let client = client.clone();
        move |_: &SignIn, cx| {
            let client = client.clone();
            cx.spawn(
                |cx| async move { client.authenticate_and_connect(true, &cx).log_err().await },
            )
            .detach();
        }
    });
    cx.add_global_action({
        let client = client.clone();
        move |_: &SignOut, cx| {
            let client = client.clone();
            cx.spawn(|cx| async move {
                client.disconnect(&cx);
            })
            .detach();
        }
    });
}

pub struct Client {
    id: usize,
    peer: Arc<Peer>,
    http: Arc<dyn HttpClient>,
    state: RwLock<ClientState>,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    authenticate: RwLock<
        Option<Box<dyn 'static + Send + Sync + Fn(&AsyncAppContext) -> Task<Result<Credentials>>>>,
    >,

    #[allow(clippy::type_complexity)]
    #[cfg(any(test, feature = "test-support"))]
    establish_connection: RwLock<
        Option<
            Box<
                dyn 'static
                    + Send
                    + Sync
                    + Fn(
                        &Credentials,
                        &AsyncAppContext,
                    ) -> Task<Result<Connection, EstablishConnectionError>>,
            >,
        >,
    >,
}

#[derive(Error, Debug)]
pub enum EstablishConnectionError {
    #[error("upgrade required")]
    UpgradeRequired,
    #[error("unauthorized")]
    Unauthorized,
    #[error("{0}")]
    Other(#[from] anyhow::Error),
    #[error("{0}")]
    Http(#[from] http::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl EstablishConnectionError {
    pub fn other(error: impl Into<anyhow::Error> + Send + Sync) -> Self {
        Self::Other(error.into())
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    SignedOut,
    UpgradeRequired,
    Authenticating,
    Connecting,
    ConnectionError,
    Connected {
        peer_id: PeerId,
        connection_id: ConnectionId,
    },
    ConnectionLost,
    Reauthenticating,
    Reconnecting,
    ReconnectionError {
        next_reconnection: Instant,
    },
}

impl Status {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn is_signed_out(&self) -> bool {
        matches!(self, Self::SignedOut | Self::UpgradeRequired)
    }
}

struct ClientState {
    credentials: Option<Credentials>,
    status: (watch::Sender<Status>, watch::Receiver<Status>),
    entity_id_extractors: HashMap<TypeId, fn(&dyn AnyTypedEnvelope) -> u64>,
    _reconnect_task: Option<Task<()>>,
    reconnect_interval: Duration,
    entities_by_type_and_remote_id: HashMap<(TypeId, u64), WeakSubscriber>,
    models_by_message_type: HashMap<TypeId, AnyWeakModelHandle>,
    entity_types_by_message_type: HashMap<TypeId, TypeId>,
    #[allow(clippy::type_complexity)]
    message_handlers: HashMap<
        TypeId,
        Arc<
            dyn Send
                + Sync
                + Fn(
                    Subscriber,
                    Box<dyn AnyTypedEnvelope>,
                    &Arc<Client>,
                    AsyncAppContext,
                ) -> LocalBoxFuture<'static, Result<()>>,
        >,
    >,
}

enum WeakSubscriber {
    Model(AnyWeakModelHandle),
    View(AnyWeakViewHandle),
    Pending(Vec<Box<dyn AnyTypedEnvelope>>),
}

enum Subscriber {
    Model(AnyModelHandle),
    View(AnyViewHandle),
}

#[derive(Clone, Debug)]
pub struct Credentials {
    pub user_id: u64,
    pub access_token: String,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            credentials: None,
            status: watch::channel_with(Status::SignedOut),
            entity_id_extractors: Default::default(),
            _reconnect_task: None,
            reconnect_interval: Duration::from_secs(5),
            models_by_message_type: Default::default(),
            entities_by_type_and_remote_id: Default::default(),
            entity_types_by_message_type: Default::default(),
            message_handlers: Default::default(),
        }
    }
}

pub enum Subscription {
    Entity {
        client: Weak<Client>,
        id: (TypeId, u64),
    },
    Message {
        client: Weak<Client>,
        id: TypeId,
    },
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match self {
            Subscription::Entity { client, id } => {
                if let Some(client) = client.upgrade() {
                    let mut state = client.state.write();
                    let _ = state.entities_by_type_and_remote_id.remove(id);
                }
            }
            Subscription::Message { client, id } => {
                if let Some(client) = client.upgrade() {
                    let mut state = client.state.write();
                    let _ = state.entity_types_by_message_type.remove(id);
                    let _ = state.message_handlers.remove(id);
                }
            }
        }
    }
}

pub struct PendingEntitySubscription<T: Entity> {
    client: Arc<Client>,
    remote_id: u64,
    _entity_type: PhantomData<T>,
    consumed: bool,
}

impl<T: Entity> PendingEntitySubscription<T> {
    pub fn set_model(mut self, model: &ModelHandle<T>, cx: &mut AsyncAppContext) -> Subscription {
        self.consumed = true;
        let mut state = self.client.state.write();
        let id = (TypeId::of::<T>(), self.remote_id);
        let Some(WeakSubscriber::Pending(messages)) =
            state.entities_by_type_and_remote_id.remove(&id)
        else {
            unreachable!()
        };

        state
            .entities_by_type_and_remote_id
            .insert(id, WeakSubscriber::Model(model.downgrade().into()));
        drop(state);
        for message in messages {
            self.client.handle_message(message, cx);
        }
        Subscription::Entity {
            client: Arc::downgrade(&self.client),
            id,
        }
    }
}

impl<T: Entity> Drop for PendingEntitySubscription<T> {
    fn drop(&mut self) {
        if !self.consumed {
            let mut state = self.client.state.write();
            if let Some(WeakSubscriber::Pending(messages)) = state
                .entities_by_type_and_remote_id
                .remove(&(TypeId::of::<T>(), self.remote_id))
            {
                for message in messages {
                    log::info!("unhandled message {}", message.payload_type_name());
                }
            }
        }
    }
}

impl Client {
    pub fn new(http: Arc<dyn HttpClient>, _cx: &AppContext) -> Arc<Self> {
        Arc::new(Self {
            id: 0,
            peer: Peer::new(0),
            http,
            state: Default::default(),

            #[cfg(any(test, feature = "test-support"))]
            authenticate: Default::default(),
            #[cfg(any(test, feature = "test-support"))]
            establish_connection: Default::default(),
        })
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn http_client(&self) -> Arc<dyn HttpClient> {
        self.http.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_id(&mut self, id: usize) -> &Self {
        self.id = id;
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn teardown(&self) {
        let mut state = self.state.write();
        state._reconnect_task.take();
        state.message_handlers.clear();
        state.models_by_message_type.clear();
        state.entities_by_type_and_remote_id.clear();
        state.entity_id_extractors.clear();
        self.peer.teardown();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn override_authenticate<F>(&self, authenticate: F) -> &Self
    where
        F: 'static + Send + Sync + Fn(&AsyncAppContext) -> Task<Result<Credentials>>,
    {
        *self.authenticate.write() = Some(Box::new(authenticate));
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn override_establish_connection<F>(&self, connect: F) -> &Self
    where
        F: 'static
            + Send
            + Sync
            + Fn(&Credentials, &AsyncAppContext) -> Task<Result<Connection, EstablishConnectionError>>,
    {
        *self.establish_connection.write() = Some(Box::new(connect));
        self
    }

    pub fn user_id(&self) -> Option<u64> {
        self.state
            .read()
            .credentials
            .as_ref()
            .map(|credentials| credentials.user_id)
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        if let Status::Connected { peer_id, .. } = &*self.status().borrow() {
            Some(*peer_id)
        } else {
            None
        }
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        self.state.read().status.1.clone()
    }

    fn set_status(self: &Arc<Self>, status: Status, cx: &AsyncAppContext) {
        log::info!("set status on client {}: {:?}", self.id, status);
        let mut state = self.state.write();
        *state.status.0.borrow_mut() = status;

        match status {
            Status::Connected { .. } => {
                state._reconnect_task = None;
            }
            Status::ConnectionLost => {
                let this = self.clone();
                let reconnect_interval = state.reconnect_interval;
                state._reconnect_task = Some(cx.spawn(|cx| async move {
                    #[cfg(any(test, feature = "test-support"))]
                    let mut rng = StdRng::seed_from_u64(0);
                    #[cfg(not(any(test, feature = "test-support")))]
                    let mut rng = StdRng::from_entropy();

                    let mut delay = INITIAL_RECONNECTION_DELAY;
                    while let Err(error) = this.authenticate_and_connect(true, &cx).await {
                        log::error!("failed to connect {}", error);
                        if matches!(*this.status().borrow(), Status::ConnectionError) {
                            this.set_status(
                                Status::ReconnectionError {
                                    next_reconnection: Instant::now() + delay,
                                },
                                &cx,
                            );
                            cx.background().timer(delay).await;
                            delay = delay
                                .mul_f32(rng.gen_range(1.0..=2.0))
                                .min(reconnect_interval);
                        } else {
                            break;
                        }
                    }
                }));
            }
            Status::SignedOut | Status::UpgradeRequired => {
                state._reconnect_task.take();
            }
            _ => {}
        }
    }

    pub fn add_view_for_remote_entity<T: View>(
        self: &Arc<Self>,
        remote_id: u64,
        cx: &mut ViewContext<T>,
    ) -> Subscription {
        let id = (TypeId::of::<T>(), remote_id);
        self.state
            .write()
            .entities_by_type_and_remote_id
            .insert(id, WeakSubscriber::View(cx.weak_handle().into()));
        Subscription::Entity {
            client: Arc::downgrade(self),
            id,
        }
    }

    pub fn subscribe_to_entity<T: Entity>(
        self: &Arc<Self>,
        remote_id: u64,
    ) -> PendingEntitySubscription<T> {
        let id = (TypeId::of::<T>(), remote_id);
        self.state
            .write()
            .entities_by_type_and_remote_id
            .insert(id, WeakSubscriber::Pending(Default::default()));

        PendingEntitySubscription {
            client: self.clone(),
            remote_id,
            consumed: false,
            _entity_type: PhantomData,
        }
    }

    pub fn add_message_handler<M, E, H, F>(
        self: &Arc<Self>,
        model: ModelHandle<E>,
        handler: H,
    ) -> Subscription
    where
        M: EnvelopedMessage,
        E: Entity,
        H: 'static
            + Send
            + Sync
            + Fn(ModelHandle<E>, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<()>>,
    {
        let message_type_id = TypeId::of::<M>();

        let mut state = self.state.write();
        state
            .models_by_message_type
            .insert(message_type_id, model.downgrade().into());

        let prev_handler = state.message_handlers.insert(
            message_type_id,
            Arc::new(move |handle, envelope, client, cx| {
                let handle = if let Subscriber::Model(handle) = handle {
                    handle
                } else {
                    unreachable!();
                };
                let model = handle.downcast::<E>().unwrap();
                let envelope = envelope.into_any().downcast::<TypedEnvelope<M>>().unwrap();
                handler(model, *envelope, client.clone(), cx).boxed_local()
            }),
        );
        if prev_handler.is_some() {
            panic!("registered handler for the same message twice");
        }

        Subscription::Message {
            client: Arc::downgrade(self),
            id: message_type_id,
        }
    }

    pub fn add_request_handler<M, E, H, F>(
        self: &Arc<Self>,
        model: ModelHandle<E>,
        handler: H,
    ) -> Subscription
    where
        M: RequestMessage,
        E: Entity,
        H: 'static
            + Send
            + Sync
            + Fn(ModelHandle<E>, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<M::Response>>,
    {
        self.add_message_handler(model, move |handle, envelope, this, cx| {
            Self::respond_to_request(
                envelope.receipt(),
                handler(handle, envelope, this.clone(), cx),
                this,
            )
        })
    }

    pub fn add_view_message_handler<M, E, H, F>(self: &Arc<Self>, handler: H)
    where
        M: EntityMessage,
        E: View,
        H: 'static
            + Send
            + Sync
            + Fn(ViewHandle<E>, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<()>>,
    {
        self.add_entity_message_handler::<M, E, _, _>(move |handle, message, client, cx| {
            if let Subscriber::View(handle) = handle {
                handler(handle.downcast::<E>().unwrap(), message, client, cx)
            } else {
                unreachable!();
            }
        })
    }

    pub fn add_model_message_handler<M, E, H, F>(self: &Arc<Self>, handler: H)
    where
        M: EntityMessage,
        E: Entity,
        H: 'static
            + Send
            + Sync
            + Fn(ModelHandle<E>, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<()>>,
    {
        self.add_entity_message_handler::<M, E, _, _>(move |handle, message, client, cx| {
            if let Subscriber::Model(handle) = handle {
                handler(handle.downcast::<E>().unwrap(), message, client, cx)
            } else {
                unreachable!();
            }
        })
    }

    fn add_entity_message_handler<M, E, H, F>(self: &Arc<Self>, handler: H)
    where
        M: EntityMessage,
        E: Entity,
        H: 'static
            + Send
            + Sync
            + Fn(Subscriber, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<()>>,
    {
        let model_type_id = TypeId::of::<E>();
        let message_type_id = TypeId::of::<M>();

        let mut state = self.state.write();
        state
            .entity_types_by_message_type
            .insert(message_type_id, model_type_id);
        state
            .entity_id_extractors
            .entry(message_type_id)
            .or_insert_with(|| {
                |envelope| {
                    envelope
                        .as_any()
                        .downcast_ref::<TypedEnvelope<M>>()
                        .unwrap()
                        .payload
                        .remote_entity_id()
                }
            });
        let prev_handler = state.message_handlers.insert(
            message_type_id,
            Arc::new(move |handle, envelope, client, cx| {
                let envelope = envelope.into_any().downcast::<TypedEnvelope<M>>().unwrap();
                handler(handle, *envelope, client.clone(), cx).boxed_local()
            }),
        );
        if prev_handler.is_some() {
            panic!("registered handler for the same message twice");
        }
    }

    pub fn add_model_request_handler<M, E, H, F>(self: &Arc<Self>, handler: H)
    where
        M: EntityMessage + RequestMessage,
        E: Entity,
        H: 'static
            + Send
            + Sync
            + Fn(ModelHandle<E>, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<M::Response>>,
    {
        self.add_model_message_handler(move |entity, envelope, client, cx| {
            Self::respond_to_request::<M, _>(
                envelope.receipt(),
                handler(entity, envelope, client.clone(), cx),
                client,
            )
        })
    }

    pub fn add_view_request_handler<M, E, H, F>(self: &Arc<Self>, handler: H)
    where
        M: EntityMessage + RequestMessage,
        E: View,
        H: 'static
            + Send
            + Sync
            + Fn(ViewHandle<E>, TypedEnvelope<M>, Arc<Self>, AsyncAppContext) -> F,
        F: 'static + Future<Output = Result<M::Response>>,
    {
        self.add_view_message_handler(move |entity, envelope, client, cx| {
            Self::respond_to_request::<M, _>(
                envelope.receipt(),
                handler(entity, envelope, client.clone(), cx),
                client,
            )
        })
    }

    async fn respond_to_request<T: RequestMessage, F: Future<Output = Result<T::Response>>>(
        receipt: Receipt<T>,
        response: F,
        client: Arc<Self>,
    ) -> Result<()> {
        match response.await {
            Ok(response) => {
                client.respond(receipt, response)?;
                Ok(())
            }
            Err(error) => {
                client.respond_with_error(
                    receipt,
                    proto::Error {
                        message: format!("{:?}", error),
                    },
                )?;
                Err(error)
            }
        }
    }

    pub fn has_keychain_credentials(&self, _cx: &AsyncAppContext) -> bool {
        false
    }

    pub async fn authenticate_and_connect(
        self: &Arc<Self>,
        _try_keychain: bool,
        cx: &AsyncAppContext,
    ) -> anyhow::Result<()> {
        self.set_status(Status::ConnectionError, cx);
        Err(anyhow!("cloud client networking is disabled in this build"))
    }

    async fn set_connection(
        self: &Arc<Self>,
        conn: Connection,
        cx: &AsyncAppContext,
    ) -> Result<()> {
        let executor = cx.background();
        log::info!("add connection to peer");
        let (connection_id, handle_io, mut incoming) = self
            .peer
            .add_connection(conn, move |duration| executor.timer(duration));
        let handle_io = cx.background().spawn(handle_io);

        let peer_id = async {
            log::info!("waiting for server hello");
            let message = incoming
                .next()
                .await
                .ok_or_else(|| anyhow!("no hello message received"))?;
            log::info!("got server hello");
            let hello_message_type_name = message.payload_type_name().to_string();
            let hello = message
                .into_any()
                .downcast::<TypedEnvelope<proto::Hello>>()
                .map_err(|_| {
                    anyhow!(
                        "invalid hello message received: {:?}",
                        hello_message_type_name
                    )
                })?;
            let peer_id = hello
                .payload
                .peer_id
                .ok_or_else(|| anyhow!("invalid peer id"))?;
            Ok(peer_id)
        };

        let peer_id = match peer_id.await {
            Ok(peer_id) => peer_id,
            Err(error) => {
                self.peer.disconnect(connection_id);
                return Err(error);
            }
        };

        log::info!(
            "set status to connected (connection id: {:?}, peer id: {:?})",
            connection_id,
            peer_id
        );
        self.set_status(
            Status::Connected {
                peer_id,
                connection_id,
            },
            cx,
        );
        cx.foreground()
            .spawn({
                let cx = cx.clone();
                let this = self.clone();
                async move {
                    while let Some(message) = incoming.next().await {
                        this.handle_message(message, &cx);
                        // Don't starve the main thread when receiving lots of messages at once.
                        smol::future::yield_now().await;
                    }
                }
            })
            .detach();

        let this = self.clone();
        let cx = cx.clone();
        cx.foreground()
            .spawn(async move {
                match handle_io.await {
                    Ok(()) => {
                        if this.status().borrow().clone()
                            == (Status::Connected {
                                connection_id,
                                peer_id,
                            })
                        {
                            this.set_status(Status::SignedOut, &cx);
                        }
                    }
                    Err(err) => {
                        log::error!("connection error: {:?}", err);
                        this.set_status(Status::ConnectionLost, &cx);
                    }
                }
            })
            .detach();

        Ok(())
    }

    fn authenticate(self: &Arc<Self>, cx: &AsyncAppContext) -> Task<Result<Credentials>> {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(callback) = self.authenticate.read().as_ref() {
            return callback(cx);
        }

        self.authenticate_with_browser(cx)
    }

    fn establish_connection(
        self: &Arc<Self>,
        _credentials: &Credentials,
        cx: &AsyncAppContext,
    ) -> Task<Result<Connection, EstablishConnectionError>> {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(callback) = self.establish_connection.read().as_ref() {
            return callback(credentials, cx);
        }

        cx.background().spawn(async {
            Err(EstablishConnectionError::other(anyhow!(
                "cloud client networking is disabled"
            )))
        })
    }

    pub fn authenticate_with_browser(
        self: &Arc<Self>,
        cx: &AsyncAppContext,
    ) -> Task<Result<Credentials>> {
        cx.background()
            .spawn(async { Err(anyhow!("authentication is disabled in this build")) })
    }

    pub fn disconnect(self: &Arc<Self>, cx: &AsyncAppContext) {
        self.peer.teardown();
        self.set_status(Status::SignedOut, cx);
    }

    fn connection_id(&self) -> Result<ConnectionId> {
        if let Status::Connected { connection_id, .. } = *self.status().borrow() {
            Ok(connection_id)
        } else {
            Err(anyhow!("not connected"))
        }
    }

    pub fn send<T: EnvelopedMessage>(&self, message: T) -> Result<()> {
        log::debug!("rpc send. client_id:{}, name:{}", self.id, T::NAME);
        self.peer.send(self.connection_id()?, message)
    }

    pub fn request<T: RequestMessage>(
        &self,
        request: T,
    ) -> impl Future<Output = Result<T::Response>> {
        let client_id = self.id;
        log::debug!(
            "rpc request start. client_id:{}. name:{}",
            client_id,
            T::NAME
        );
        let response = self
            .connection_id()
            .map(|conn_id| self.peer.request(conn_id, request));
        async move {
            let response = response?.await;
            log::debug!(
                "rpc request finish. client_id:{}. name:{}",
                client_id,
                T::NAME
            );
            response
        }
    }

    fn respond<T: RequestMessage>(&self, receipt: Receipt<T>, response: T::Response) -> Result<()> {
        log::debug!("rpc respond. client_id:{}. name:{}", self.id, T::NAME);
        self.peer.respond(receipt, response)
    }

    fn respond_with_error<T: RequestMessage>(
        &self,
        receipt: Receipt<T>,
        error: proto::Error,
    ) -> Result<()> {
        log::debug!("rpc respond. client_id:{}. name:{}", self.id, T::NAME);
        self.peer.respond_with_error(receipt, error)
    }

    fn handle_message(
        self: &Arc<Client>,
        message: Box<dyn AnyTypedEnvelope>,
        cx: &AsyncAppContext,
    ) {
        let mut state = self.state.write();
        let type_name = message.payload_type_name();
        let payload_type_id = message.payload_type_id();
        let sender_id = message.original_sender_id();

        let mut subscriber = None;

        if let Some(message_model) = state
            .models_by_message_type
            .get(&payload_type_id)
            .and_then(|model| model.upgrade(cx))
        {
            subscriber = Some(Subscriber::Model(message_model));
        } else if let Some((extract_entity_id, entity_type_id)) =
            state.entity_id_extractors.get(&payload_type_id).zip(
                state
                    .entity_types_by_message_type
                    .get(&payload_type_id)
                    .copied(),
            )
        {
            let entity_id = (extract_entity_id)(message.as_ref());

            match state
                .entities_by_type_and_remote_id
                .get_mut(&(entity_type_id, entity_id))
            {
                Some(WeakSubscriber::Pending(pending)) => {
                    pending.push(message);
                    return;
                }
                Some(weak_subscriber @ _) => subscriber = weak_subscriber.upgrade(cx),
                _ => {}
            }
        }

        let subscriber = if let Some(subscriber) = subscriber {
            subscriber
        } else {
            log::info!("unhandled message {}", type_name);
            self.peer.respond_with_unhandled_message(message).log_err();
            return;
        };

        let handler = state.message_handlers.get(&payload_type_id).cloned();
        // Dropping the state prevents deadlocks if the handler interacts with rpc::Client.
        // It also ensures we don't hold the lock while yielding back to the executor, as
        // that might cause the executor thread driving this future to block indefinitely.
        drop(state);

        if let Some(handler) = handler {
            let future = handler(subscriber, message, &self, cx.clone());
            let client_id = self.id;
            log::debug!(
                "rpc message received. client_id:{}, sender_id:{:?}, type:{}",
                client_id,
                sender_id,
                type_name
            );
            cx.foreground()
                .spawn(async move {
                    match future.await {
                        Ok(()) => {
                            log::debug!(
                                "rpc message handled. client_id:{}, sender_id:{:?}, type:{}",
                                client_id,
                                sender_id,
                                type_name
                            );
                        }
                        Err(error) => {
                            log::error!(
                                "error handling message. client_id:{}, sender_id:{:?}, type:{}, error:{:?}",
                                client_id,
                                sender_id,
                                type_name,
                                error
                            );
                        }
                    }
                })
                .detach();
        } else {
            log::info!("unhandled message {}", type_name);
            self.peer.respond_with_unhandled_message(message).log_err();
        }
    }

    pub fn start_telemetry(&self) {}

    pub fn report_event(
        &self,
        kind: &str,
        properties: Value,
        telemetry_settings: TelemetrySettings,
    ) {
        let _ = (kind, properties, telemetry_settings);
    }

    pub fn telemetry_log_file_path(&self) -> Option<PathBuf> {
        None
    }

    pub fn metrics_id(&self) -> Option<Arc<str>> {
        None
    }

    pub fn is_staff(&self) -> Option<bool> {
        None
    }
}

impl WeakSubscriber {
    fn upgrade(&self, cx: &AsyncAppContext) -> Option<Subscriber> {
        match self {
            WeakSubscriber::Model(handle) => handle.upgrade(cx).map(Subscriber::Model),
            WeakSubscriber::View(handle) => handle.upgrade(cx).map(Subscriber::View),
            WeakSubscriber::Pending(_) => None,
        }
    }
}

const WORKTREE_URL_PREFIX: &str = "zed://worktrees/";

pub fn encode_worktree_url(id: u64, access_token: &str) -> String {
    format!("{}{}/{}", WORKTREE_URL_PREFIX, id, access_token)
}

pub fn decode_worktree_url(url: &str) -> Option<(u64, String)> {
    let path = url.trim().strip_prefix(WORKTREE_URL_PREFIX)?;
    let mut parts = path.split('/');
    let id = parts.next()?.parse::<u64>().ok()?;
    let access_token = parts.next()?;
    if access_token.is_empty() {
        return None;
    }
    Some((id, access_token.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{FakeHttpClient, FakeServer};
    use gpui::{executor::Deterministic, TestAppContext};
    use parking_lot::Mutex;
    use std::future;

    #[gpui::test(iterations = 10)]
    async fn test_reconnection(cx: &mut TestAppContext) {
        cx.foreground().forbid_parking();

        let user_id = 5;
        let client = cx.update(|cx| Client::new(FakeHttpClient::with_404_response(), cx));
        let server = FakeServer::for_client(user_id, &client, cx).await;
        let mut status = client.status();
        assert!(matches!(
            status.next().await,
            Some(Status::Connected { .. })
        ));
        assert_eq!(server.auth_count(), 1);

        server.forbid_connections();
        server.disconnect();
        while !matches!(status.next().await, Some(Status::ReconnectionError { .. })) {}

        server.allow_connections();
        cx.foreground().advance_clock(Duration::from_secs(10));
        while !matches!(status.next().await, Some(Status::Connected { .. })) {}
        assert_eq!(server.auth_count(), 1); // Client reused the cached credentials when reconnecting

        server.forbid_connections();
        server.disconnect();
        while !matches!(status.next().await, Some(Status::ReconnectionError { .. })) {}

        // Clear cached credentials after authentication fails
        server.roll_access_token();
        server.allow_connections();
        cx.foreground().advance_clock(Duration::from_secs(10));
        while !matches!(status.next().await, Some(Status::Connected { .. })) {}
        assert_eq!(server.auth_count(), 2); // Client re-authenticated due to an invalid token
    }

    #[gpui::test(iterations = 10)]
    async fn test_connection_timeout(deterministic: Arc<Deterministic>, cx: &mut TestAppContext) {
        deterministic.forbid_parking();

        let user_id = 5;
        let client = cx.update(|cx| Client::new(FakeHttpClient::with_404_response(), cx));
        let mut status = client.status();

        // Time out when client tries to connect.
        client.override_authenticate(move |cx| {
            cx.foreground().spawn(async move {
                Ok(Credentials {
                    user_id,
                    access_token: "token".into(),
                })
            })
        });
        client.override_establish_connection(|_, cx| {
            cx.foreground().spawn(async move {
                future::pending::<()>().await;
                unreachable!()
            })
        });
        let auth_and_connect = cx.spawn({
            let client = client.clone();
            |cx| async move { client.authenticate_and_connect(false, &cx).await }
        });
        deterministic.run_until_parked();
        assert!(matches!(status.next().await, Some(Status::Connecting)));

        deterministic.advance_clock(CONNECTION_TIMEOUT);
        assert!(matches!(
            status.next().await,
            Some(Status::ConnectionError { .. })
        ));
        auth_and_connect.await.unwrap_err();

        // Allow the connection to be established.
        let server = FakeServer::for_client(user_id, &client, cx).await;
        assert!(matches!(
            status.next().await,
            Some(Status::Connected { .. })
        ));

        // Disconnect client.
        server.forbid_connections();
        server.disconnect();
        while !matches!(status.next().await, Some(Status::ReconnectionError { .. })) {}

        // Time out when re-establishing the connection.
        server.allow_connections();
        client.override_establish_connection(|_, cx| {
            cx.foreground().spawn(async move {
                future::pending::<()>().await;
                unreachable!()
            })
        });
        deterministic.advance_clock(2 * INITIAL_RECONNECTION_DELAY);
        assert!(matches!(
            status.next().await,
            Some(Status::Reconnecting { .. })
        ));

        deterministic.advance_clock(CONNECTION_TIMEOUT);
        assert!(matches!(
            status.next().await,
            Some(Status::ReconnectionError { .. })
        ));
    }

    #[gpui::test(iterations = 10)]
    async fn test_authenticating_more_than_once(
        cx: &mut TestAppContext,
        deterministic: Arc<Deterministic>,
    ) {
        cx.foreground().forbid_parking();

        let auth_count = Arc::new(Mutex::new(0));
        let dropped_auth_count = Arc::new(Mutex::new(0));
        let client = cx.update(|cx| Client::new(FakeHttpClient::with_404_response(), cx));
        client.override_authenticate({
            let auth_count = auth_count.clone();
            let dropped_auth_count = dropped_auth_count.clone();
            move |cx| {
                let auth_count = auth_count.clone();
                let dropped_auth_count = dropped_auth_count.clone();
                cx.foreground().spawn(async move {
                    *auth_count.lock() += 1;
                    let _drop = util::defer(move || *dropped_auth_count.lock() += 1);
                    future::pending::<()>().await;
                    unreachable!()
                })
            }
        });

        let _authenticate = cx.spawn(|cx| {
            let client = client.clone();
            async move { client.authenticate_and_connect(false, &cx).await }
        });
        deterministic.run_until_parked();
        assert_eq!(*auth_count.lock(), 1);
        assert_eq!(*dropped_auth_count.lock(), 0);

        let _authenticate = cx.spawn(|cx| {
            let client = client.clone();
            async move { client.authenticate_and_connect(false, &cx).await }
        });
        deterministic.run_until_parked();
        assert_eq!(*auth_count.lock(), 2);
        assert_eq!(*dropped_auth_count.lock(), 1);
    }

    #[test]
    fn test_encode_and_decode_worktree_url() {
        let url = encode_worktree_url(5, "deadbeef");
        assert_eq!(decode_worktree_url(&url), Some((5, "deadbeef".to_string())));
        assert_eq!(
            decode_worktree_url(&format!("\n {}\t", url)),
            Some((5, "deadbeef".to_string()))
        );
        assert_eq!(decode_worktree_url("not://the-right-format"), None);
    }

    #[gpui::test]
    async fn test_subscribing_to_entity(cx: &mut TestAppContext) {
        cx.foreground().forbid_parking();

        let user_id = 5;
        let client = cx.update(|cx| Client::new(FakeHttpClient::with_404_response(), cx));
        let server = FakeServer::for_client(user_id, &client, cx).await;

        let (done_tx1, mut done_rx1) = smol::channel::unbounded();
        let (done_tx2, mut done_rx2) = smol::channel::unbounded();
        client.add_model_message_handler(
            move |model: ModelHandle<Model>, _: TypedEnvelope<proto::JoinProject>, _, cx| {
                match model.read_with(&cx, |model, _| model.id) {
                    1 => done_tx1.try_send(()).unwrap(),
                    2 => done_tx2.try_send(()).unwrap(),
                    _ => unreachable!(),
                }
                async { Ok(()) }
            },
        );
        let model1 = cx.add_model(|_| Model {
            id: 1,
            subscription: None,
        });
        let model2 = cx.add_model(|_| Model {
            id: 2,
            subscription: None,
        });
        let model3 = cx.add_model(|_| Model {
            id: 3,
            subscription: None,
        });

        let _subscription1 = client
            .subscribe_to_entity(1)
            .set_model(&model1, &mut cx.to_async());
        let _subscription2 = client
            .subscribe_to_entity(2)
            .set_model(&model2, &mut cx.to_async());
        // Ensure dropping a subscription for the same entity type still allows receiving of
        // messages for other entity IDs of the same type.
        let subscription3 = client
            .subscribe_to_entity(3)
            .set_model(&model3, &mut cx.to_async());
        drop(subscription3);

        server.send(proto::JoinProject { project_id: 1 });
        server.send(proto::JoinProject { project_id: 2 });
        done_rx1.next().await.unwrap();
        done_rx2.next().await.unwrap();
    }

    #[gpui::test]
    async fn test_subscribing_after_dropping_subscription(cx: &mut TestAppContext) {
        cx.foreground().forbid_parking();

        let user_id = 5;
        let client = cx.update(|cx| Client::new(FakeHttpClient::with_404_response(), cx));
        let server = FakeServer::for_client(user_id, &client, cx).await;

        let model = cx.add_model(|_| Model::default());
        let (done_tx1, _done_rx1) = smol::channel::unbounded();
        let (done_tx2, mut done_rx2) = smol::channel::unbounded();
        let subscription1 = client.add_message_handler(
            model.clone(),
            move |_, _: TypedEnvelope<proto::Ping>, _, _| {
                done_tx1.try_send(()).unwrap();
                async { Ok(()) }
            },
        );
        drop(subscription1);
        let _subscription2 =
            client.add_message_handler(model, move |_, _: TypedEnvelope<proto::Ping>, _, _| {
                done_tx2.try_send(()).unwrap();
                async { Ok(()) }
            });
        server.send(proto::Ping {});
        done_rx2.next().await.unwrap();
    }

    #[gpui::test]
    async fn test_dropping_subscription_in_handler(cx: &mut TestAppContext) {
        cx.foreground().forbid_parking();

        let user_id = 5;
        let client = cx.update(|cx| Client::new(FakeHttpClient::with_404_response(), cx));
        let server = FakeServer::for_client(user_id, &client, cx).await;

        let model = cx.add_model(|_| Model::default());
        let (done_tx, mut done_rx) = smol::channel::unbounded();
        let subscription = client.add_message_handler(
            model.clone(),
            move |model, _: TypedEnvelope<proto::Ping>, _, mut cx| {
                model.update(&mut cx, |model, _| model.subscription.take());
                done_tx.try_send(()).unwrap();
                async { Ok(()) }
            },
        );
        model.update(cx, |model, _| {
            model.subscription = Some(subscription);
        });
        server.send(proto::Ping {});
        done_rx.next().await.unwrap();
    }

    #[derive(Default)]
    struct Model {
        id: usize,
        subscription: Option<Subscription>,
    }

    impl Entity for Model {
        type Event = ();
    }
}
