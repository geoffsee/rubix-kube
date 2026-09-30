use rubix_supervisor::{
    Adapter, AdapterContext, AdapterError, AdapterFuture, ComponentKind, ComponentSpec,
    FailurePolicy, Registration, StopPhase,
};
use std::time::Duration;

use crate::service::ApiserverService;

pub const COMPONENT_APISERVER: &str = "apiserver";
pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_mins(1);

#[derive(Clone, Debug)]
pub struct ApiserverAdapter {
    service: ApiserverService,
}

impl ApiserverAdapter {
    pub fn new(service: ApiserverService) -> Self {
        Self { service }
    }

    pub fn registration(
        id: impl Into<String>,
        service: ApiserverService,
        prerequisites: Vec<String>,
        timeout: Duration,
    ) -> Registration {
        let spec = ComponentSpec {
            id: id.into(),
            prerequisites,
            kind: ComponentKind::LongRunning,
            failure_policy: FailurePolicy::Fatal,
            startup_timeout: timeout,
        };
        Registration::new(spec, Self::new(service))
    }
}

impl Adapter for ApiserverAdapter {
    fn run(self: Box<Self>, mut context: AdapterContext) -> AdapterFuture {
        Box::pin(async move {
            // 1. Check prerequisites (PKI credentials and storage availability)
            if let Err(err) = self.service.check_prerequisites().await {
                return Err(AdapterError {
                    code: err.diagnostic_code(),
                });
            }

            // 2. Start service
            if let Err(err) = self.service.start() {
                return Err(AdapterError {
                    code: err.diagnostic_code(),
                });
            }

            // 3. Check readiness
            match self.service.check_readiness().await {
                Ok(report) if report.is_healthy => {},
                Ok(_) => {
                    return Err(AdapterError {
                        code: "apiserver-readiness-failed",
                    });
                },
                Err(err) => {
                    return Err(AdapterError {
                        code: err.diagnostic_code(),
                    });
                },
            }

            // 4. Signal readiness to supervisor coordinator
            if !context.ready() {
                return Err(AdapterError {
                    code: "apiserver-readiness-rejected",
                });
            }

            // 5. Await shutdown
            loop {
                match context.changed().await {
                    StopPhase::Running => {},
                    StopPhase::Graceful | StopPhase::Force => {
                        self.service.stop();
                        break;
                    },
                }
            }

            Ok(())
        })
    }
}
