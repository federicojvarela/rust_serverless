use uuid::Uuid;
use validator::ValidationError;

pub fn validate_not_default_uuid(username: &Uuid) -> Result<(), ValidationError> {
    if username == &Uuid::default() {
        // the value of the username will automatically be added later
        return Err(ValidationError::new("invalid Uuid"));
    }
    Ok(())
}
