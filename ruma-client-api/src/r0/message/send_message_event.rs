//! [PUT /_matrix/client/r0/rooms/{roomId}/send/{eventType}/{txnId}](https://matrix.org/docs/spec/client_server/r0.6.1#put-matrix-client-r0-rooms-roomid-send-eventtype-txnid)

use std::convert::TryFrom;

use ruma_api::{
    error::{
        FromHttpRequestError, FromHttpResponseError, IntoHttpError, RequestDeserializationError,
        ResponseDeserializationError, ServerError,
    },
    Endpoint, EndpointError, Metadata, Outgoing,
};
use ruma_events::{AnyMessageEventContent, EventContent as _};
use ruma_identifiers::{EventId, RoomId};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue as RawJsonValue;

/// Data for a request to the `create_message_event` API endpoint.
///
/// Send a message event to a room.
#[derive(Clone, Debug, Outgoing)]
#[incoming_no_deserialize]
pub struct Request<'a> {
    /// The room to send the event to.
    pub room_id: &'a RoomId,

    /// The transaction ID for this event.
    ///
    /// Clients should generate an ID unique across requests with the
    /// same access token; it will be used by the server to ensure
    /// idempotency of requests.
    pub txn_id: &'a str,

    /// The event content to send.
    pub content: &'a AnyMessageEventContent,
}

/// Data in the response from the `create_message_event` API endpoint.
#[derive(Clone, Debug, Outgoing)]
#[incoming_no_deserialize]
pub struct Response {
    /// A unique identifier for the event.
    pub event_id: EventId,
}

impl TryFrom<http::Request<Vec<u8>>> for IncomingRequest {
    type Error = FromHttpRequestError;

    fn try_from(request: http::Request<Vec<u8>>) -> Result<Self, Self::Error> {
        let path_segments: Vec<&str> = request.uri().path()[1..].split('/').collect();

        let room_id = {
            let decoded =
                match percent_encoding::percent_decode(path_segments[4].as_bytes()).decode_utf8() {
                    Ok(val) => val,
                    Err(err) => return Err(RequestDeserializationError::new(err, request).into()),
                };

            match RoomId::try_from(&*decoded) {
                Ok(val) => val,
                Err(err) => return Err(RequestDeserializationError::new(err, request).into()),
            }
        };

        let txn_id =
            match percent_encoding::percent_decode(path_segments[7].as_bytes()).decode_utf8() {
                Ok(val) => val.into_owned(),
                Err(err) => return Err(RequestDeserializationError::new(err, request).into()),
            };

        let content = {
            let request_body: Box<RawJsonValue> =
                match serde_json::from_slice(request.body().as_slice()) {
                    Ok(val) => val,
                    Err(err) => return Err(RequestDeserializationError::new(err, request).into()),
                };

            let event_type = {
                match percent_encoding::percent_decode(path_segments[6].as_bytes()).decode_utf8() {
                    Ok(val) => val,
                    Err(err) => return Err(RequestDeserializationError::new(err, request).into()),
                }
            };

            match AnyMessageEventContent::from_parts(&event_type, request_body) {
                Ok(content) => content,
                Err(err) => return Err(RequestDeserializationError::new(err, request).into()),
            }
        };

        Ok(Self { room_id, txn_id, content })
    }
}

/// Data in the response body.
#[derive(Debug, Deserialize, Serialize)]
struct ResponseBody {
    /// A unique identifier for the event.
    event_id: EventId,
}

impl TryFrom<Response> for http::Response<Vec<u8>> {
    type Error = IntoHttpError;

    fn try_from(response: Response) -> Result<Self, Self::Error> {
        let response = http::Response::builder()
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&ResponseBody { event_id: response.event_id })?)
            .unwrap();

        Ok(response)
    }
}

impl TryFrom<http::Response<Vec<u8>>> for Response {
    type Error = FromHttpResponseError<crate::Error>;

    fn try_from(response: http::Response<Vec<u8>>) -> Result<Self, Self::Error> {
        if response.status().as_u16() < 400 {
            let response_body: ResponseBody =
                match serde_json::from_slice(response.body().as_slice()) {
                    Ok(val) => val,
                    Err(err) => return Err(ResponseDeserializationError::new(err, response).into()),
                };

            Ok(Self { event_id: response_body.event_id })
        } else {
            match <crate::Error as EndpointError>::try_from_response(response) {
                Ok(err) => Err(ServerError::Known(err).into()),
                Err(response_err) => Err(ServerError::Unknown(response_err).into()),
            }
        }
    }
}

impl<'a> Endpoint for Request<'a> {
    type Response = Response;
    type ResponseError = crate::Error;
    type IncomingRequest = IncomingRequest;
    type IncomingResponse = Response;

    /// Metadata for the `#name` endpoint.
    const METADATA: Metadata = Metadata {
        description: "Send a message event to a room.",
        method: http::Method::PUT,
        name: "create_message_event",
        path: "/_matrix/client/r0/rooms/:room_id/send/:event_type/:txn_id",
        rate_limited: false,
        requires_authentication: true,
    };

    fn try_into_http_request(
        self,
        base_url: &str,
        access_token: Option<&str>,
    ) -> Result<http::Request<Vec<u8>>, IntoHttpError> {
        use http::header::{HeaderValue, AUTHORIZATION};
        use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

        let http_request = http::Request::builder()
            .method(http::Method::PUT)
            .uri(format!(
                "{}/_matrix/client/r0/rooms/{}/send/{}/{}",
                base_url.strip_suffix("/").unwrap_or(base_url),
                utf8_percent_encode(self.room_id.as_str(), NON_ALPHANUMERIC),
                utf8_percent_encode(self.content.event_type(), NON_ALPHANUMERIC),
                utf8_percent_encode(&self.txn_id, NON_ALPHANUMERIC),
            ))
            .header(
                AUTHORIZATION,
                HeaderValue::from_str(&format!(
                    "Bearer {}",
                    access_token.ok_or_else(IntoHttpError::needs_authentication)?
                ))?,
            )
            .body(serde_json::to_vec(&self.content)?)?;

        Ok(http_request)
    }
}
