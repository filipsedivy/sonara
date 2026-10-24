//! The live frames' channels and the `peaks:` URI scheme. The frames, the
//! feed's rates and the tiles are `sonara-app`'s (`live`, `peaks`); this module only carries them
//! to the web views.

use std::borrow::Cow;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sonara_app::peaks::TileRequest;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::shell::Shell;

/// A web view that receives the live frames.
struct Subscriber {
    label: String,
    channel: Channel<InvokeResponseBody>,
}

/// The web views subscribed to the live frames (one `Channel` each).
#[derive(Clone, Default)]
pub struct LiveChannels {
    subscribers: Arc<Mutex<Vec<Subscriber>>>,
    /// When each frame was sent (the meter benchmark's Rust side).
    #[cfg(feature = "bench")]
    stamps: crate::bench::FrameStamps,
}

impl LiveChannels {
    fn subscribers(&self) -> MutexGuard<'_, Vec<Subscriber>> {
        // A list of channels is consistent after any panic: entries are added or removed whole.
        self.subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// When each frame was sent.
    #[cfg(feature = "bench")]
    pub fn stamps(&self) -> &crate::bench::FrameStamps {
        &self.stamps
    }

    /// The labels of the subscribed web views.
    #[cfg(feature = "bench")]
    pub fn labels(&self) -> Vec<String> {
        self.subscribers()
            .iter()
            .map(|subscriber| subscriber.label.clone())
            .collect()
    }

    /// Sends the frames to `channel` from now on, replacing the web view `label`'s earlier one.
    pub fn subscribe(&self, label: &str, channel: Channel<InvokeResponseBody>) {
        let mut subscribers = self.subscribers();
        subscribers.retain(|subscriber| subscriber.label != label);
        subscribers.push(Subscriber {
            label: label.to_owned(),
            channel,
        });
    }

    /// Sends one frame to every subscriber; a web view that is gone is dropped.
    pub fn send(&self, frame: &[u8]) {
        #[cfg(feature = "bench")]
        self.stamps.record(frame);
        self.subscribers().retain(|subscriber| {
            subscriber
                .channel
                .send(InvokeResponseBody::Raw(frame.to_vec()))
                .is_ok()
        });
    }
}

/// The `peaks:` scheme: `peaks://localhost/tile/<asset>/<rate>/<level>/<index>` answers with
/// that tile of a ready pyramid (`sonara_app::peaks::encode_tile`), and 404 otherwise.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes the handler's arguments by value"
)]
pub fn peaks_protocol<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let shell = context.app_handle().state::<Shell>();
    let tile = TileRequest::parse(request.uri().path()).and_then(|tile| {
        shell
            .app()
            .peaks()
            .tile(&tile.asset_id, tile.sample_rate, tile.level, tile.index)
    });
    let response = match tile {
        Some(tile) => binary_response(StatusCode::OK, Cow::Owned(tile)),
        None => binary_response(StatusCode::NOT_FOUND, Cow::Borrowed(&[])),
    };
    responder.respond(response);
}

fn binary_response(status: StatusCode, body: Cow<'static, [u8]>) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/octet-stream"),
    );
    // The page's origin is tauri://localhost; the scheme is another origin.
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        header::HeaderValue::from_static("*"),
    );
    response
}
