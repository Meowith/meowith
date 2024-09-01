use crate::adapter::r#impl::basic_authenticator::BasicAuthenticator;
use crate::AuthFacade;
use commons::error::std_response::NodeClientResponse;
use std::collections::HashMap;
use std::sync::Arc;

pub type AuthenticationMethodList = Arc<HashMap<String, Box<dyn AuthFacade>>>;
pub type AuthMethodMap = HashMap<String, Box<dyn AuthFacade>>;

pub fn init_authentication_methods(
    config_login_methods: Vec<String>,
) -> NodeClientResponse<AuthMethodMap> {
    let login_methods: Vec<Box<dyn AuthFacade>> = vec![Box::new(BasicAuthenticator)];

    let mut method_map: HashMap<String, Box<dyn AuthFacade>> = HashMap::new();

    for facade in login_methods {
        for method in config_login_methods.clone() {
            if facade.get_type() == method {
                method_map.insert(method.clone(), facade);
                break;
            }
        }
    }

    log::info!("Initialized the following authentication methods: {method_map:?}");

    Ok(method_map)
}
