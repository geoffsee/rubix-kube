use rubix_supervisor::{
    Adapter, AdapterContext, AdapterError, AdapterFuture, ComponentKind, ComponentSpec,
    FailurePolicy, Registration, StopPhase,
};
use std::time::Duration;

use crate::config::DatastoreConfig;
use crate::engine::DatastoreEngine;

#[derive(Clone, Debug)]
pub struct DatastoreAdapter {
    config: DatastoreConfig,
    engine: Option<DatastoreEngine>,
}

impl DatastoreAdapter {
    pub fn new(config: DatastoreConfig) -> Self {
        Self {
            config,
            engine: None,
        }
    }

    pub fn from_engine(engine: DatastoreEngine) -> Self {
        Self {
            config: engine.config().clone(),
            engine: Some(engine),
        }
    }

    pub fn registration(
        id: impl Into<String>,
        config: DatastoreConfig,
        timeout: Duration,
    ) -> Registration {
        let spec = ComponentSpec {
            id: id.into(),
            prerequisites: Vec::new(),
            kind: ComponentKind::LongRunning,
            failure_policy: FailurePolicy::Fatal,
            startup_timeout: timeout,
        };
        Registration::new(spec, Self::new(config))
    }

    pub fn registration_for_engine(
        id: impl Into<String>,
        engine: DatastoreEngine,
        timeout: Duration,
    ) -> Registration {
        let spec = ComponentSpec {
            id: id.into(),
            prerequisites: Vec::new(),
            kind: ComponentKind::LongRunning,
            failure_policy: FailurePolicy::Fatal,
            startup_timeout: timeout,
        };
        Registration::new(spec, Self::from_engine(engine))
    }
}

impl Adapter for DatastoreAdapter {
    fn run(self: Box<Self>, mut context: AdapterContext) -> AdapterFuture {
        Box::pin(async move {
            let engine = match self.engine {
                Some(eng) => eng,
                None => match DatastoreEngine::open(self.config) {
                    Ok((eng, _summary)) => eng,
                    Err(err) => {
                        return Err(AdapterError {
                            code: err.diagnostic_code(),
                        });
                    },
                },
            };

            // Signal usable readiness to supervisor coordinator
            if !context.ready() {
                return Err(AdapterError {
                    code: "datastore-readiness-rejected",
                });
            }

            // Await stop signal
            loop {
                match context.changed().await {
                    StopPhase::Running => {},
                    StopPhase::Graceful | StopPhase::Force => {
                        let _ = engine.checkpoint_snapshot().await;
                        break;
                    },
                }
            }

            Ok(())
        })
    }
}
