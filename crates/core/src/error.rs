use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use thiserror::Error;

pub type Result<T, E = Infallible> = std::result::Result<T, Error<E>>;

#[derive(Debug, Deserialize, Serialize, Error)]
#[error("Infallible")]
pub struct Infallible;

#[macro_export]
macro_rules! anyhow {
    ($($arg:tt)*) => {
        $crate::Error::from(anyhow::anyhow!($($arg)*))
    };
}

#[derive(Debug, Deserialize, Serialize, Error)]
pub enum Error<E>
where
    E: Debug + Display + std::error::Error + Send + Sync + 'static,
{
    #[error(transparent)]
    BadRequest(E),

    #[serde(with = "anyhow_serde")]
    #[error("Internal server error")]
    Unexpected(
        #[from]
        #[source]
        anyhow::Error,
    ),
}

impl<E> From<std::io::Error> for Error<E>
where
    E: Debug + Display + std::error::Error + Send + Sync + 'static,
{
    fn from(err: std::io::Error) -> Self {
        Self::Unexpected(anyhow::Error::from(err))
    }
}

mod anyhow_serde {
    use serde::{
        de::{Error, Visitor},
        Deserializer, Serializer,
    };

    pub fn serialize<S: Serializer>(
        value: &anyhow::Error,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:?}", value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<anyhow::Error, D::Error> {
        struct Helper;

        impl<'de> Visitor<'de> for Helper {
            type Value = anyhow::Error;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "valid error string")
            }

            fn visit_str<E: Error>(self, value: &str) -> Result<anyhow::Error, E> {
                Ok(anyhow::anyhow!("{}", value))
            }
        }

        deserializer.deserialize_str(Helper)
    }
}
