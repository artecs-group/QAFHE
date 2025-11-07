use std::sync::atomic::Ordering;

use ringbuffer::RingBuffer;
use qafhe_proxy_lib::{
    policy::{ProxyPolicy, Request},
    server::{Endpoint, Endpoints}
};
use uuid::Uuid;
use super::{process_locally, read_models, Model, TritonContext};
use anyhow::{Context, Result};

#[derive(Default)]
/// Reglas:
/// - Si hay algun endpoint que no se ha usado nunca, escoge ese.
/// - Si no, escoge que tenga una latencia media (5 ultimos intentos) más baja.
///
/// **Problema**: si localhost se considera óptimo y le spameo peticiones, 
/// localhost se va a sobrecargar, y se empezará a enviar a otro vecino.
/// Pero cuando localhost vuelve a la normalidad, ya no se le vuelven a enviar
/// peticiones.
pub struct MintimePolicy {
    models: Vec<Model>,
    local_uuid: Uuid
}

impl MintimePolicy {
    pub fn new(path: &str, local_uuid: Uuid) -> Result<Self> {
        Ok(Self {
            models: read_models(path)?,
            local_uuid
        })
    }
}

impl ProxyPolicy<TritonContext> for MintimePolicy {

    async fn choose_target(&self, request: &Request<TritonContext>, endpts: &Endpoints<TritonContext>) -> Uuid {
        
        if request.jumps > 0 {
            return self.local_uuid;
        }

        let node_uuid = endpts.iter()
            // Avoid cycles.
            .filter(|ep| !request.previous_nodes.contains(ep.0))
            .filter(|ep| ep.1.pending_request.load(Ordering::SeqCst) == 0)
            .min_by_key(|(_, ep)| calculate_weight(ep))
            .map(|(uuid, _)| *uuid)
            .unwrap();
       
       node_uuid
    }

    async fn process_locally(&self, request: &Request<TritonContext>) -> Result<Vec<u8>> {
        
        let model = request.context.model
            .as_ref()
            .and_then(|model_name| {
                self.models.iter()
                    .find(|m| m.name.contains(model_name))
            })
            .unwrap_or_else(|| {
                log::error!("Failed to find model that contains the pattern.");
                self.models.iter()
                    .min_by_key(|model| model.perf)
                    .unwrap()
            });

        process_locally(request, model).await
    }
}

/// Returns 0 for endpoints never used.
fn calculate_weight(endp: &Endpoint<TritonContext>) -> u32 {
    
    let num_endps = endp.last_results.len() as u32;
    let sum_latency: u32 = endp.last_results.iter()
        .map(|res| {

            match res.duration {
                Some(dur) => dur.as_millis() as u32,
                None => 10 * 1000
            }
        })
        .sum();
     
    let avg = sum_latency.checked_div(num_endps).unwrap_or(0);
    log::info!("Average pod latencies{}: {}", endp.name, avg);
    avg
}
