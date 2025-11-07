// This policy is based on the following paper:
// https://ieeexplore.ieee.org/abstract/document/8672614

use std::sync::atomic::Ordering;

use ringbuffer::RingBuffer;
use qafhe_proxy_lib::{
    policy::{ProxyPolicy, Request},
    server::{Endpoint, Endpoints}
};
use uuid::Uuid;
use crate::{policies::requisitos::est_tiempo_para_acc, utils::{calcular_hw, cola_estimada_ms}};

use super::{process_locally, read_models, Model, TritonContext};
use anyhow::{Context, Result};

#[derive(Default)]
pub struct DetourPolicy {
    models: Vec<Model>,
    self_uuid: Uuid
}

impl DetourPolicy {
    pub fn new(path: &str, self_uuid: Uuid) -> Result<Self> {
        Ok(Self {
            models: read_models(path)?,
            self_uuid
        })
    }
}

// Returns True if the task should be offloaded.
fn u_off_k(request: &Request<TritonContext>, ep_local: &Endpoint<TritonContext>) -> bool {

    let est_local = est_tiempo_para_acc(ep_local, request.context.accuracy);
    match est_local {
        Some(t_local) => t_local > request.context.priority,
        None => false
    }
}

fn u_fog_jk(request: &Request<TritonContext>, ep: &Endpoint<TritonContext>) -> f32 {
    
    log::info!("u_fog_jk: {:?}", ep);
    let t1 = 0.5 * cola_estimada_ms(ep) as f32;
    let t2 = 0.5 * ( request.context.accuracy / calcular_hw(ep) as u32 ) as f32;
    log::info!("u_fog_jk : t1({t1}), t2={t2}");
    t1 + t2
}

impl ProxyPolicy<TritonContext> for DetourPolicy {

    async fn choose_target(&self, request: &Request<TritonContext>, endpts: &Endpoints<TritonContext>) -> Uuid {
        
        if request.jumps > 0 {
            return self.self_uuid;
        }

        let self_ep = endpts.get(&self.self_uuid).expect("Failed to get self endpoint.");
        if endpts.len() > 1 || u_off_k(request, self_ep) {
            *endpts
                .iter()
                .filter(|(uid, _)| *uid != &self.self_uuid) // Filter out local endpoint.
                .filter(|ep| ep.1.pending_request.load(Ordering::SeqCst) == 0)
                .map(|(uuid, ep)| (uuid, u_fog_jk(request, ep)))
                .min_by(|ep1, ep2| ep1.1.total_cmp(&ep2.1))
                .expect("Failed to get another endpoint.")
                .0
        }
        else {
            self.self_uuid
        }
    }

    async fn process_locally(&self, request: &Request<TritonContext>) -> Result<Vec<u8>> {

        let model = self.models.iter()
            .filter(|m| m.accuracy >= request.context.accuracy)
            .min_by_key(|m| m.perf)
            .unwrap_or(self.models.iter().max_by_key(|m| m.accuracy).unwrap());

        process_locally(request, model).await
    }
}

