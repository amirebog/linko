use crate::error::AppError;
use axum::extract::{FromRequest, Request};
use axum::Json;
use serde::de::DeserializeOwned;

pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(_) => Err(AppError::bad_request("Malformed JSON request.")),
        }
    }
}
