use layerx_paxeer_client::{BoundaryHealth, BoundaryStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentHealth {
    Ready,
    Degraded { reason: String },
    Unavailable { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthComponent {
    HumanService,
    AgentLayer,
    Paxeer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceHealth {
    pub human_service: ComponentHealth,
    pub agent_layer: ComponentHealth,
    pub paxeer: BoundaryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceReadiness {
    pub ready: bool,
    pub unavailable: Vec<HealthComponent>,
}

impl ServiceHealth {
    #[must_use]
    pub fn readiness(&self) -> ServiceReadiness {
        let mut unavailable = Vec::new();
        if matches!(self.human_service, ComponentHealth::Unavailable { .. }) {
            unavailable.push(HealthComponent::HumanService);
        }
        if matches!(self.agent_layer, ComponentHealth::Unavailable { .. }) {
            unavailable.push(HealthComponent::AgentLayer);
        }
        if self.paxeer.health == BoundaryHealth::Unavailable {
            unavailable.push(HealthComponent::Paxeer);
        }
        ServiceReadiness {
            ready: unavailable.is_empty(),
            unavailable,
        }
    }
}
