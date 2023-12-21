use actix_web::HttpResponse;

// Canned responses for proposed and deprecated endpoints

pub async fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().body("")
}

pub async fn unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized().body("")
}
