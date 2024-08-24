use crate::auth::r#impl::basic_authenticator::BasicAuthenticator;
use crate::config::DashboardConfig;
use auth_framework::AuthFacade;
use commons::error::std_response::NodeClientResponse;
use std::collections::HashMap;
use std::sync::Arc;

pub type AuthenticationMethodList = Arc<HashMap<String, Box<dyn AuthFacade>>>;

pub fn init_authentication_methods(
    config: &DashboardConfig,
) -> NodeClientResponse<HashMap<String, Box<dyn AuthFacade>>> {
    let login_methods: Vec<Box<dyn AuthFacade>> = vec![Box::new(BasicAuthenticator)];

    let mut method_map: HashMap<String, Box<dyn AuthFacade>> = HashMap::new();

    for facade in login_methods {
        for method in config.login_methods.clone() {
            if facade.get_type() == method {
                method_map.insert(method.clone(), facade);
                break;
            }
        }
    }

    log::info!("Initialized the following authentication methods: {method_map:?}");

    Ok(method_map)
}
