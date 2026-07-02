pub mod game_card;
pub mod setup_page;
pub mod welcome_page;

#[cfg(test)]
pub mod test_util;

pub const WIZARD_DEFAULT_WIDTH: i32 = 872;
pub const WIZARD_DEFAULT_HEIGHT: i32 = 666;

pub(crate) fn catch_ui_panic(label: &'static str, action: impl FnOnce()) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).map_err(|payload| {
        let message = panic_message(payload.as_ref());
        tracing::error!("UI callback panicked in {label}: {message}");
        message
    })
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn catch_ui_panic_reports_panics_without_unwinding() {
        let result = super::catch_ui_panic("test callback", || panic!("boom"));

        assert_eq!(result, Err("boom".to_string()));
    }

    #[test]
    fn catch_ui_panic_returns_ok_for_successful_callbacks() {
        assert_eq!(super::catch_ui_panic("test callback", || {}), Ok(()));
    }
}
