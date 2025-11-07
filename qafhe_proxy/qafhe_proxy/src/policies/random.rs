use rand::random;
use qafhe_proxy_lib::{
    policy::{ProxyPolicy, Request, RequestContext},
    server::Endpoints
};
use uuid::Uuid;
use super::{process_locally, read_models, Model};
use anyhow::Result;

#[derive(Default)]
pub struct RandomPolicy {
    models: Vec<Model>,
    local_uuid: Uuid
}

impl RandomPolicy {
    pub fn new(path: &str, local_uuid: Uuid) -> Result<Self> {
        Ok(Self {
            models: read_models(path)?,
            local_uuid
        })
    }
}

impl<R: RequestContext> ProxyPolicy<R> for RandomPolicy {
     
    async fn choose_target(&self, request: &Request<R>, endpoints: &Endpoints<R>) -> Uuid {
         
        println!("Random NJUMPS: {}", request.jumps);
        println!("Endpoints: {:?}", endpoints);
        if request.jumps > 0 {
            println!("Local target");
            return self.local_uuid;
        }

        let n_nodes = endpoints.len();
        let node_index = rand::random::<usize>() % n_nodes;
        let node_uuid = endpoints.keys().nth(node_index).cloned().unwrap();
        node_uuid 
    }

    async fn process_locally(&self, request: &Request<R>) -> Result<Vec<u8>> {

        let model = self.models.iter()
            .min_by_key(|model| model.perf)
            .unwrap();

        process_locally(request, model).await
    }
}
