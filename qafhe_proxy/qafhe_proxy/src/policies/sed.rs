use rand::random;
use qafhe_proxy_lib::{
    policy::{ProxyPolicy, Request, RequestContext},
    server::Endpoints
};
use uuid::Uuid;
use super::{process_locally, read_models, Model};
use anyhow::Result;

#[derive(Default)]
pub struct SedPolicy {
    models: Vec<Model>,
    local_ep: Uuid
}

impl SedPolicy {
    pub fn new(path: &str, local_ep: Uuid) -> Result<Self> {
        Ok(Self {
            models: read_models(path)?,
            local_ep
        })
    }
}

impl<R: RequestContext> ProxyPolicy<R> for SedPolicy {
     
    async fn choose_target(&self, _request: &Request<R>, endpoints: &Endpoints<R>) -> Uuid {
        self.local_ep
    }

    async fn process_locally(&self, request: &Request<R>) -> Result<Vec<u8>> {

        let model = self.models.iter()
            .min_by_key(|model| model.perf)
            .unwrap();

        process_locally(request, model).await
    }
}
