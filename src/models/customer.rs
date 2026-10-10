use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use validator::Validate;

use super::id_to_string;

/// Perfil do cliente. O username vive no `user` de mesma chave.
#[derive(Debug, Clone, SurrealValue)]
pub struct CustomerRecord {
    pub id: RecordId,
    pub first_name: String,
    pub last_name: String,
    pub birthday: NaiveDate,
}

#[derive(Debug, Serialize)]
pub struct Customer {
    pub id: String,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub birthday: NaiveDate,
}

/// Usado na substituição completa (PUT). O cadastro é feito por `sign-up`.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCustomer {
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 100, message = "at most 100 characters")
    )]
    pub first_name: String,
    #[validate(
        custom(
            function = "crate::models::validate_not_blank",
            message = "must not be blank"
        ),
        length(max = 100, message = "at most 100 characters")
    )]
    pub last_name: String,
    #[validate(custom(
        function = "crate::models::validate_not_future",
        message = "must not be in the future"
    ))]
    pub birthday: NaiveDate,
}

#[derive(Debug, SurrealValue)]
pub struct NewCustomer {
    pub first_name: String,
    pub last_name: String,
    pub birthday: NaiveDate,
}

impl Customer {
    /// Junta o perfil (customer) com o username que está no user.
    pub fn from_parts(record: CustomerRecord, username: String) -> Self {
        Customer {
            id: id_to_string(&record.id),
            username,
            first_name: record.first_name,
            last_name: record.last_name,
            birthday: record.birthday,
        }
    }
}
