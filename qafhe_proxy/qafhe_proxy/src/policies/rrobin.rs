use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use itertools::Itertools;
use tokio::sync::Mutex;
use qafhe_proxy_lib::{policy::{ProxyPolicy, Request}, server::Endpoints};
use uuid::Uuid;
use super::{process_locally, read_models, Model, TritonContext, TritonEndpoints, TritonRequest};

#[derive(Debug, Default)]
pub struct RrobinPolicy {
    models: Vec<Model>,
    siguiente: AtomicUsize,
    local_uuid: Uuid
}
impl RrobinPolicy {
    pub fn new(path: &str, local_uuid: Uuid) -> Result<Self> {
        Ok(Self {
            siguiente: AtomicUsize::new(0),
            models: read_models(path)?,
            local_uuid
        })
    }
}

impl ProxyPolicy<TritonContext> for RrobinPolicy {

    async fn choose_target(&self, request: &TritonRequest, endpoints: &TritonEndpoints) -> Uuid {
        
        println!("NJUMPS: {}", request.jumps);
        println!("Endpoints: {:?}", endpoints);
        if request.jumps > 0 {
            println!("Local target");
            return self.local_uuid;
        }

        let siguiente = self.siguiente.fetch_add(1, Ordering::Relaxed);
        log::info!("rrobin: siguiente={}", siguiente);
        let n_endps = endpoints.len();
        let target = *endpoints.iter()
            .nth(siguiente % n_endps)
            .unwrap().0;

        println!("Target: {target}");
        target
    }

    async fn process_locally(&self, request: &TritonRequest) -> Result<Vec<u8>> {

        let model = self.models.iter()
            .min_by_key(|m| m.perf)
            .unwrap();

        process_locally(request, model).await
    }
}
