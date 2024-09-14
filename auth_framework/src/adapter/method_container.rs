use crate::adapter::r#impl::basic_authenticator::BasicAuthenticator;
use crate::AuthFacade;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use std::collections::HashMap;
use std::sync::Arc;
use data::dto::config::CatIdAppConfiguration;
use crate::adapter::r#impl::catid_authenticator::{CATID_TYPE_IDENTIFIER, CatIdAuthenticator};

pub type AuthenticationMethodList = Arc<HashMap<String, Box<dyn AuthFacade>>>;
pub type AuthMethodMap = HashMap<String, Box<dyn AuthFacade>>;

pub fn init_authentication_methods(
    config_login_methods: Vec<String>,
    cat_id_app_configuration: Option<CatIdAppConfiguration>
) -> NodeClientResponse<AuthMethodMap> {
    let catid = cat_id_app_configuration.unwrap_or(CatIdAppConfiguration {
        app_id: "NONE".to_string(),
        secret: "".to_string(),
    });

    let login_methods: Vec<Box<dyn AuthFacade>> = vec![Box::new(BasicAuthenticator), Box::new(
        CatIdAuthenticator::new(catid.clone())
    )];

    if config_login_methods.contains(&CATID_TYPE_IDENTIFIER.to_string()) && catid.app_id == "NONE" {
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
