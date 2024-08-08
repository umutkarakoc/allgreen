use axum::response::Html;
use maud::{html, Markup, DOCTYPE};

pub fn page(headers: Markup, page: Markup) -> Html<String> {
    let t = html! {
        (DOCTYPE)
        head {
            meta charset="UTF-8";
            meta http-equiv="X-UA-Compatible" content="IE=edge";
            meta name="viewport" content="width=device-width, initial-scale=1.0";
            title {"AllGreen"}
            script src="https://kit.fontawesome.com/c018f1996d.js" crossorigin="anonymous" {}
            link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css";
            link rel="stylesheet" href="/style.css";
            link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/cooltipz-css/cooltipz.min.css" ;
            link rel="icon" href="/logo.png" type="image/x-icon";
            // script[src="https://unpkg.com/htmx.org@1.8.4"] {}
            script src="https://unpkg.com/htmx.org@1.8.6/dist/htmx.js" {}
            script src="https://unpkg.com/htmx.org/dist/ext/multi-swap.js" {}
            script src="https://cdnjs.cloudflare.com/ajax/libs/cash/8.1.2/cash.min.js" {}
            script src="https://cdn.jsdelivr.net/npm/chart.js" {}

            (headers)
        }
        body hx-ext="multi-swap" {
            (page)
        }
    };

    Html(t.into_string())
}
