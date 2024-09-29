use crate::adapter::r#impl::basic_authenticator::BasicAuthenticator;
use crate::adapter::r#impl::catid_authenticator::{CatIdAuthenticator, CATID_TYPE_IDENTIFIER};
use crate::AuthFacade;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::dto::config::GeneralConfiguration;
use std::collections::HashMap;
use std::sync::Arc;

pub type AuthenticationMethodList = Arc<HashMap<String, Box<dyn AuthFacade>>>;
pub type AuthMethodMap = HashMap<String, Box<dyn AuthFacade>>;

pub fn init_authentication_methods(
    config_login_methods: Vec<String>,
    config: GeneralConfiguration,
) -> NodeClientResponse<AuthMethodMap> {
    let mut login_methods: Vec<Box<dyn AuthFacade>> = vec![Box::new(BasicAuthenticator)];
    let catid_config_exists = config.cat_id_config.is_some();
    if catid_config_exists {
        login_methods.push(Box::new(CatIdAuthenticator::new(config)))
    }

    if config_login_methods.contains(&CATID_TYPE_IDENTIFIER.to_string()) && !catid_config_exists {
        return Err(NodeClientError::NotFound);
    }

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
