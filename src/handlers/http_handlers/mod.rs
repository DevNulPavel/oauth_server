
use actix_web::{
    web::{
        self
    },
    HttpMessage
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
use serde::{
    Deserialize
};
use tracing::{
    instrument,
    error,
    info
    // debug_span, 
    // debug,
};
use tracing_error::{
    SpanTrace
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

#[derive(Deserialize, Debug)]
pub struct IndexQueryParams{
    custom_client_url: Option<String>
}

#[instrument(err, skip(handlebars, app_params), fields(app_user_id = %full_info.app_user_uuid))]
pub async fn index(req: web::HttpRequest, 
                   handlebars: web::Data<Handlebars<'_>>, 
                   app_params: web::Data<AppEnvParams>,
                   query: web::Query<IndexQueryParams>,
                   full_info: UserInfo) -> Result<web::HttpResponse, AppError> {
    
    // Custom application url?
    let mut game_url = match query.custom_client_url.as_ref() {
        Some(query) => {
            let url = url::Url::parse(query.as_str())?;

            // Сверим, что доменное имя такое же, как и у нормальной ссылки
            if app_params.game_url.domain() != url.domain(){
                return Err(AppError::Custom(SpanTrace::capture(), "Custom url must be from the same domain".to_owned()));
            }

            url
        },
        None => {
            app_params.game_url.clone()
        }
    };

    // Client url parameters
    game_url.query_pairs_mut().append_pair("uuid", &full_info.app_user_uuid);
    game_url.query_pairs_mut().append_pair("facebook_uid", &full_info.facebook_uuid.as_deref().unwrap_or(""));
    game_url.query_pairs_mut().append_pair("google_uid", &full_info.google_uuid.as_deref().unwrap_or(""));
                
    let template_data = serde_json::json!({
        "uuid": full_info.app_user_uuid,
        "facebook_uid": full_info.facebook_uuid,
        "google_uid": full_info.google_uuid,
        "game_url": game_url.as_str()
    });

    // Рендерим шаблон
    let body = handlebars.render(constants::INDEX_TEMPLATE, &template_data)
        .tap_err(|err|{
            error!("Template render failed: {}", err);
        })?;

    info!("Index rendered");

    let mut response = web::HttpResponse::Ok();
    response.content_type("text/html; charset=utf-8");
    if let Some(target_client_url_cookie) = req.cookie("target_client_url"){
        response
            .del_cookie(&target_client_url_cookie);
    };
    Ok(response.body(body))
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[instrument(err, skip(handlebars))]
pub async fn login_page(req: web::HttpRequest, 
                        handlebars: web::Data<Handlebars<'_>>, 
                        query: web::Query<IndexQueryParams>) -> Result<web::HttpResponse, AppError> {
    let body = handlebars.render(constants::LOGIN_TEMPLATE, &serde_json::json!({}))
        .tap_err(|err|{
            error!("Template render failed: {}", err);
        })?;

    info!("Login rendered");

    let mut response = web::HttpResponse::Ok();
    response.content_type("text/html; charset=utf-8");

    // Сохраним в куку значение кастомного адреса
    if let Some(custom_client_addr) = &query.custom_client_url{
        info!(%custom_client_addr, "Custom client url at login");
        // TODO: короткое время жизни куки
        let cookie = actix_web::http::Cookie::build("target_client_url", custom_client_addr)
            .finish();
        response.cookie(cookie);
    }else{
        // Либо удалим если уже были какие-то
        if let Some(target_client_url_cookie) = req.cookie("target_client_url"){
            response
                .del_cookie(&target_client_url_cookie);
        };
    }

    Ok(response.body(body))
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[instrument(err, skip(id))]
pub async fn logout(id: Identity) -> Result<web::HttpResponse, AppError> {
    id.forget();

    // Возвращаем код 302 и Location в заголовках для перехода
    return Ok(web::HttpResponse::Found()
                .header(actix_web::http::header::LOCATION, constants::LOGIN_PATH)
                .finish())
}