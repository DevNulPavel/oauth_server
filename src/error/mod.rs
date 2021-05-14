use quick_error::{
    quick_error
};
use actix_web::{
    http::{
        StatusCode
    },
    dev::{
        HttpResponseBuilder
    },
    HttpResponse,
    ResponseError
};
use tracing_error::{
    SpanTrace
};
use serde_json::{
    json
};
use crate::{
    responses::{
        FacebookErrorResponse,
        GoogleErrorResponse
    }
};

quick_error!{
    #[derive(Debug)]
    pub enum AppError{
        /// Не смогли отрендерить шаблон
        TemplateRenderError(trace: SpanTrace, err: handlebars::RenderError){
            from(err: handlebars::RenderError) -> (SpanTrace::capture(), err)
        }

        /// Не смогли отрендерить шаблон
        ActixError(trace: SpanTrace, err: actix_web::Error){
            from(err: actix_web::Error) -> (SpanTrace::capture(), err)
        }

        /// Ошибка парсинга адреса
        URLParseError(trace: SpanTrace, err: url::ParseError){
            from(err: url::ParseError) -> (SpanTrace::capture(), err)
        }

        /// Ошибка у внутреннего запроса с сервера на какое-то API
        InternalReqwestLibraryError(trace: SpanTrace, context: &'static str, err: reqwest::Error){
            context(context: &'static str, err: reqwest::Error) -> (SpanTrace::capture(), context, err)
        }

        /// Сервер Facebook ответил ошибкой какой-то
        FacebookApiError(trace: SpanTrace, err: FacebookErrorResponse){
            from(err: FacebookErrorResponse) -> (SpanTrace::capture(), err)
        }

        /// Сервер Google ответил ошибкой какой-то
        GoogleApiError(trace: SpanTrace, err: GoogleErrorResponse){
            from(err: GoogleErrorResponse) -> (SpanTrace::capture(), err)
        }

        /// Произошла ошибка работы с базой данных
        DatabaseError(trace: SpanTrace, err: sqlx::Error){
            from(err: sqlx::Error) -> (SpanTrace::capture(), err)
        }

        /// Ошибка с произвольным описанием
        Custom(trace: SpanTrace, err: String){
        }
    }
}

// Для нашего enum ошибки реализуем конвертацию в ResponseError,
// но делаем это так, чтобы ответ был в виде json
impl ResponseError for AppError {
    // Код ошибки
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    // Создаем ответ в виде json
    fn error_response(&self) -> HttpResponse {
        let data = json!({
            "code": self.status_code().as_u16(),
            "message": self.to_string()
        });
        HttpResponseBuilder::new(self.status_code())
            .json(data)
    }    
}