
use actix_web::{
    web::{
        self
    }
};
use handlebars::{
    Handlebars
};
use actix_identity::{
    Identity
};
use tap::{
    prelude::{
        *
    }
};
use tracing::{
    instrument,
    error
    // debug_span, 
    // debug,
};
use crate::{
    error::{
        AppError
    },
    database::{
        UserInfo
    },
    app_params::{
        AppEnvParams
    },
    constants::{
        self
    }
};

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[instrument(skip(handlebars), fields(user_id = %full_info.user_uuid))]
pub async fn index(handlebars: web::Data<Handlebars<'_>>, 
                   app_params: web::Data<AppEnvParams>,
                   full_info: UserInfo) -> Result<web::HttpResponse, AppError> {
    
    let mut game_url = app_params.game_url.clone();
    game_url.query_pairs_mut().append_pair("uuid", &full_info.user_uuid);
    game_url.query_pairs_mut().append_pair("facebook_uid", &full_info.facebook_uid.as_deref().unwrap_or(""));
    game_url.query_pairs_mut().append_pair("google_uid", &full_info.google_uid.as_deref().unwrap_or(""));
                
    let template_data = serde_json::json!({
        "uuid": full_info.user_uuid,
        "facebook_uid": full_info.facebook_uid,
        "google_uid": full_info.google_uid,
        "game_url": game_url.as_str()
    });

    // Рендерим шаблон
    let body = handlebars.render(constants::INDEX_TEMPLATE, &template_data)
        .tap_err(|err|{
            error!("Template render failed: {}", err);
        })?;

    Ok(web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[instrument(skip(handlebars))]
pub async fn login_page(handlebars: web::Data<Handlebars<'_>>) -> Result<web::HttpResponse, AppError> {
    let body = handlebars.render(constants::LOGIN_TEMPLATE, &serde_json::json!({}))
        .tap_err(|err|{
            error!("Template render failed: {}", err);
        })?;

    Ok(web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[instrument(skip(id))]
pub async fn logout(id: Identity) -> Result<web::HttpResponse, AppError> {
    id.forget();

    // Возвращаем код 302 и Location в заголовках для перехода
    return Ok(web::HttpResponse::Found()
                .header(actix_web::http::header::LOCATION, constants::LOGIN_PATH)
                .finish())
}