use super::{Inserter, InsertionError};

/// Routes Insertion during onboarding: while `onboarding` says first-run setup is not finished,
/// a Transcript goes to the onboarding's Try it field through `deliver` instead of into the
/// focused application, so the test works wherever keyboard focus is
/// (`settings-and-first-run.md` rule 2.4). Afterwards every Transcript goes to `real`.
pub struct TryItInserter {
    real: Box<dyn Inserter>,
    onboarding: Box<dyn Fn() -> bool + Send>,
    deliver: Box<Deliver>,
}

/// Hands a Transcript to the Try it field; an error fails the Insertion.
type Deliver = dyn FnMut(&str) -> Result<(), InsertionError> + Send;

impl TryItInserter {
    pub fn new(
        real: impl Inserter + 'static,
        onboarding: impl Fn() -> bool + Send + 'static,
        deliver: impl FnMut(&str) -> Result<(), InsertionError> + Send + 'static,
    ) -> Self {
        Self {
            real: Box::new(real),
            onboarding: Box::new(onboarding),
            deliver: Box::new(deliver),
        }
    }
}

impl Inserter for TryItInserter {
    fn insert(&mut self, text: &str) -> Result<(), InsertionError> {
        if (self.onboarding)() {
            (self.deliver)(text)
        } else {
            self.real.insert(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::super::FakeInserter;
    use super::*;

    fn routed(onboarding: bool) -> (FakeInserter, Arc<Mutex<Vec<String>>>, TryItInserter) {
        let real = FakeInserter::new();
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let sink = delivered.clone();
        let inserter = TryItInserter::new(
            real.clone(),
            move || onboarding,
            move |text| {
                sink.lock().unwrap().push(text.to_owned());
                Ok(())
            },
        );
        (real, delivered, inserter)
    }

    #[test]
    fn during_onboarding_the_transcript_goes_to_the_try_it_field_only() {
        let (real, delivered, mut inserter) = routed(true);

        inserter.insert("Hello Echo, can you hear me?").unwrap();

        assert_eq!(*delivered.lock().unwrap(), ["Hello Echo, can you hear me?"]);
        assert_eq!(real.inserted(), Vec::<String>::new());
    }

    #[test]
    fn after_onboarding_the_transcript_goes_to_the_focused_application() {
        let (real, delivered, mut inserter) = routed(false);

        inserter.insert("Dear Anna,").unwrap();

        assert_eq!(real.inserted(), ["Dear Anna,"]);
        assert!(delivered.lock().unwrap().is_empty());
    }

    #[test]
    fn the_route_is_decided_at_each_insertion() {
        let onboarding = Arc::new(AtomicBool::new(true));
        let flag = onboarding.clone();
        let real = FakeInserter::new();
        let mut inserter = TryItInserter::new(
            real.clone(),
            move || flag.load(Ordering::SeqCst),
            |_| Ok(()),
        );

        inserter.insert("first").unwrap();
        onboarding.store(false, Ordering::SeqCst);
        inserter.insert("second").unwrap();

        assert_eq!(real.inserted(), ["second"]);
    }

    #[test]
    fn a_failed_delivery_is_reported_like_a_failed_insertion() {
        let mut inserter = TryItInserter::new(
            FakeInserter::new(),
            || true,
            |_| Err(InsertionError::Failed("the window is closed".into())),
        );

        assert!(inserter.insert("text").is_err());
    }
}
