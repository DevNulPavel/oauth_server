use actix_web::{
    web::{
        self
    },
    HttpMessage
};
use tracing::{
    debug,
    error,
    instrument
};
use actix_identity::{
    Identity
};
use serde::{
    Deserialize
};
use quick_error::{
    ResultExt
};
use tap::{
    prelude::{
        *
    }
};
use crate::{
    error::{
        AppError
    },
    app_params::{
        GoogleEnvParams,
        AppEnvParams
    },
    responses::{
        DataOrErrorResponse,
        GoogleErrorResponse,
        GoogleTokenResponse,
        GoogleUserInfoResponse
    },
    database::{
        Database
    },
    constants::{
        self
    }
};

/*fn get_callback_address(req: &actix_web::HttpRequest) -> String {
    let conn_info = req.connection_info();
    format!("{scheme}://{host}{api}{login}", 
                scheme = conn_info.scheme(),
                host = conn_info.host(),
                api = constants::GOOGLE_SCOPE_PATH,
                login = constants::AUTH_CALLBACK_PATH)
}*/

fn get_callback_address(base_url: &str) -> String {
    format!("{base_url}{api}{login}", 
                base_url = base_url.trim_end_matches("/"),
                api = constants::GOOGLE_SCOPE_PATH,
                login = constants::AUTH_CALLBACK_PATH)
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Данный метод вызывается при нажатии на кнопку логина в Facebook
#[instrument(err, skip(app_params, google_params, fb_params))]
pub async fn login_with_google(app_params: web::Data<AppEnvParams>,
                               google_params: web::Data<GoogleEnvParams>,
                               fb_params: web::Data<GoogleEnvParams>) -> Result<web::HttpResponse, AppError> {

    // Адрес нашего сайта + адрес коллбека
    let callback_site_address = get_callback_address(app_params.app_base_url.as_str());
    
    // Создаем урл, на который надо будет идти для логина
    // https://developers.google.com/identity/protocols/oauth2/web-server#httprest
    let mut auth_url = google_params.auth_uri.clone();
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &fb_params.client_id)
        .append_pair("redirect_uri", &callback_site_address)
        .append_pair("response_type", "code")
        .append_pair("access_type", "online")
        .append_pair("include_granted_scopes", "true")
        .append_pair("scope", "https://www.googleapis.com/auth/userinfo.email")
        .finish();

    debug!("Google url value: {}", auth_url);

    // Возвращаем код 302 и Location в заголовках для перехода
    Ok(web::HttpResponse::Found()
        .header(actix_web::http::header::LOCATION, auth_url.as_str())
        .finish())
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Данный метод является адресом-коллбеком который вызывается после логина на facebook
#[derive(Debug, Deserialize)]
pub struct GoogleAuthParams{
    code: String
}
#[instrument(err, skip(identity, google_params, query_params, app_params, http_client, db))]
pub async fn google_auth_callback(req: actix_web::HttpRequest,
                                  app_params: web::Data<AppEnvParams>,
                                  query_params: web::Query<GoogleAuthParams>, 
                                  identity: Identity,
                                  google_params: web::Data<GoogleEnvParams>,
                                  http_client: web::Data<reqwest::Client>,
                                  db: web::Data<Database>) -> Result<web::HttpResponse, AppError> {
    // debug!("Request object: {:?}", req);
    // debug!("Google auth callback query params: {:?}", query_params);

    // Адрес нашего сайта + адрес коллбека
    let callback_site_address = get_callback_address(app_params.app_base_url.as_str());

    // Выполняем запрос для получения токена на основании кода у редиректа
    let response = http_client
        .post(google_params.token_uri.as_ref())
        .form(&serde_json::json!({
            "client_id": google_params.client_id.as_str(),
            "client_secret": google_params.client_secret.as_str(),
            "redirect_uri": callback_site_address.as_str(),   // TODO: Для чего он нужен?
            "code": query_params.code.as_str(),
            "grant_type": "authorization_code"
        }))
        .send()
        .await
        // .context("Google token reqwest send error")? // Может выдать секреты наружу
        // .error_for_status()
        .context("Google token reqwest status error")?
        .json::<DataOrErrorResponse<GoogleTokenResponse, GoogleErrorResponse>>()
        .await
        .context("Google token reqwest response parse error")?
        .into_result()
        .map_err(AppError::from)
        .tap_err(|err|{
            error!("Google user token request failed: {}", err);
        })?;

    debug!(?response, "Token response");

    // Выполняем запрос информации о пользователе
    let user_info_data = http_client
        .get("https://www.googleapis.com/oauth2/v1/userinfo")
        .bearer_auth(&response.access_token)
        .send()
        .await
        // .context("Google token reqwest send error")? // Может выдать секреты наружу
        // .error_for_status()
        .context("Google user data reqwest status error")?
        .json::<DataOrErrorResponse<GoogleUserInfoResponse, GoogleErrorResponse>>()
        .await
        .context("Google user data reqwest response parse error")?
        .into_result()
        .map_err(AppError::from)
        .tap_err(|err|{
            error!("Google user info request failed: {}", err);
        })?;

    debug!(?user_info_data, "Google user info");

    // Получили айдишник пользователя на FB, делаем запрос к базе данных, чтобы проверить наличие нашего пользователя
    let db_res = db
        .try_to_find_user_uuid_with_google_id(&user_info_data.id)
        .await?;

    debug!(?db_res, "Google database search");
    
    match db_res {
        Some(user_uuid) => {
            debug!(%user_uuid, "User exists");

            // Сохраняем идентификатор в куках
            identity.remember(user_uuid);
        },
        None => {
            // Если мы залогинились, но у нас есть валидный пользователь в куках, джойним к нему GoogleId
            let uuid = match identity.identity() {
                Some(uuid) if db.does_user_uuid_exist(&uuid).await? => {
                    debug!(uuid = %uuid, "User with identity exists");

                    // Добавляем в базу идентификатор нашего пользователя
                    db
                        .append_google_user_for_uuid(&uuid, &user_info_data.id)
                        .await?;

                    uuid
                },
                _ => {
                    // Сбрасываем если был раньше
                    identity.forget();
                    
                    // TODO: вынести в общую функцию
                    // Выполняем генерацию нового UUID
                    let uuid = format!("{}", uuid::Uuid::new_v4());

                    // Сохраняем в базу идентификатор нашего пользователя
                    db
                        .insert_google_user_with_uuid(&uuid, &user_info_data.id)
                        .await?;

                    uuid
                }
            };

            // Сохраняем идентификатор в куках
            identity.remember(uuid);
        }
    }

    let mut response = web::HttpResponse::Found();

    // Если в куках у нас сохранены значения кастомного урла, добавляем его к пути
    if let Some(target_client_url_cookie) = req.cookie("target_client_url"){
        let path = format!("{}?custom_client_url={}", constants::INDEX_PATH, target_client_url_cookie.value());
        response
            .header(actix_web::http::header::LOCATION, path)
            .del_cookie(&target_client_url_cookie);
    }else{
        response.header(actix_web::http::header::LOCATION, constants::INDEX_PATH);
    };

    // Возвращаем код 302 и Location в заголовках для перехода
    Ok(response.finish())
}