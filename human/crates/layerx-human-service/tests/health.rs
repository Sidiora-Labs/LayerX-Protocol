use std::time::Duration;

use layerx_human_service::health::{
    ComponentHealth, HealthComponent, ServiceHealth, ServiceReadiness,
};
use layerx_paxeer_client::{
    BoundaryStatus, ChainSignal, ConfirmationProgress, EndpointError, EndpointFailure,
    EndpointFault, EndpointSignal, FinalityReport, FinalityStage, TransactionHash,
};

fn status(endpoint: EndpointSignal, signal: ChainSignal) -> BoundaryStatus {
    BoundaryStatus::from_report(
        &FinalityReport {
            transaction: TransactionHash::new([7; 32]),
            stage: FinalityStage::Missing { head: 41 },
            signal,
            endpoint,
            progress: ConfirmationProgress {
                confirmed: 0,
                required: 3,
            },
            displacements: 0,
            polls: 2,
        },
        Duration::from_secs(2),
    )
}

fn ready_components(paxeer: BoundaryStatus) -> ServiceHealth {
    ServiceHealth {
        human_service: ComponentHealth::Ready,
        agent_layer: ComponentHealth::Ready,
        paxeer,
    }
}

#[test]
fn readiness_names_paxeer_as_the_unavailable_component() {
    let error = EndpointError {
        failures: vec![EndpointFailure {
            url: "http://127.0.0.1:1".to_owned(),
            fault: EndpointFault::Connect {
                detail: "connection refused".to_owned(),
            },
        }],
    };
    let health = ready_components(status(
        EndpointSignal::Unreachable {
            error: error.clone(),
        },
        ChainSignal::Unreachable { error },
    ));

    assert_eq!(
        health.readiness(),
        ServiceReadiness {
            ready: false,
            unavailable: vec![HealthComponent::Paxeer],
        }
    );
}

#[test]
fn degraded_paxeer_timing_remains_ready_but_visible() {
    let health = ready_components(status(
        EndpointSignal::Serving,
        ChainSignal::Delayed {
            stalled_polls: 3,
            threshold: 3,
            stalled_for: Duration::from_secs(6),
            delayed_after: Duration::from_secs(6),
        },
    ));

    assert_eq!(
        health.readiness(),
        ServiceReadiness {
            ready: true,
            unavailable: Vec::new(),
        }
    );
    assert_eq!(
        health.paxeer.health,
        layerx_paxeer_client::BoundaryHealth::Degraded
    );
}

#[test]
fn service_and_agent_failures_are_reported_separately() {
    let health = ServiceHealth {
        human_service: ComponentHealth::Unavailable {
            reason: "store unavailable".to_owned(),
        },
        agent_layer: ComponentHealth::Unavailable {
            reason: "agent boundary unavailable".to_owned(),
        },
        paxeer: status(EndpointSignal::Serving, ChainSignal::Progressing),
    };

    assert_eq!(
        health.readiness(),
        ServiceReadiness {
            ready: false,
            unavailable: vec![HealthComponent::HumanService, HealthComponent::AgentLayer],
        }
    );
}
