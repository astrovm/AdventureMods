use std::any::Any;

use anyhow::Result;

pub fn spawn_result<T>(result: std::thread::Result<T>) -> Result<T> {
    result.map_err(|payload| anyhow::anyhow!("spawn error: {}", panic_message(&*payload)))
}

pub fn flatten_spawn_result<T>(result: std::thread::Result<Result<T>>) -> Result<T> {
    spawn_result(result)?
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }

    if let Some(error) = payload.downcast_ref::<anyhow::Error>() {
        return error.to_string();
    }

    "blocking task panicked".to_string()
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::*;

    #[test]
    fn flatten_spawn_result_returns_inner_error() {
        let result = flatten_spawn_result::<()>(Ok(Err(anyhow!("bad archive"))));

        assert_eq!(result.unwrap_err().to_string(), "bad archive");
    }

    #[test]
    fn spawn_result_returns_plain_value() {
        let result = spawn_result(Ok::<_, Box<dyn Any + Send>>(42_u32));

        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn flatten_spawn_result_returns_anyhow_panic_message() {
        let result = flatten_spawn_result::<()>(Err(Box::new(anyhow!("download failed"))));

        assert_eq!(
            result.unwrap_err().to_string(),
            "spawn error: download failed"
        );
    }

    #[test]
    fn flatten_spawn_result_returns_string_panic_message() {
        let result = flatten_spawn_result::<()>(Err(Box::new(String::from("boom"))));

        assert_eq!(result.unwrap_err().to_string(), "spawn error: boom");
    }

    #[test]
    fn flatten_spawn_result_falls_back_for_non_string_panics() {
        let result = flatten_spawn_result::<()>(Err(Box::new(123_u32)));

        assert_eq!(
            result.unwrap_err().to_string(),
            "spawn error: blocking task panicked"
        );
    }
}
