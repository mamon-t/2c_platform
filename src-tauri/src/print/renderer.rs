use handlebars::Handlebars;
use handlebars::{HelperDef, HelperResult, Helper, RenderContext, Output, Context};
use super::{PrintContext, PrintTemplate};

fn fmt_money(h: &Helper<'_>, _: &Handlebars<'_>, _: &Context, _: &mut RenderContext<'_, '_>, out: &mut dyn Output) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
    let abs = param.abs();
    let whole = abs as i64;
    let frac = ((abs - whole as f64) * 100.0).round() as i64;
    let sign = if param < 0.0 { "-" } else { "" };
    let formatted = format!("{}{} \u{20BD}", sign, format_with_delimiters(whole, frac));
    out.write(&formatted)?;
    Ok(())
}

fn fmt_date(h: &Helper<'_>, _: &Handlebars<'_>, _: &Context, _: &mut RenderContext<'_, '_>, out: &mut dyn Output) -> HelperResult {
    let val = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let formatted = if val.is_empty() {
        "\u{2014}".into()
    } else if let Ok(dt) = chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d") {
        dt.format("%d.%m.%Y").to_string()
    } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(val) {
        dt.format("%d.%m.%Y").to_string()
    } else {
        val.to_string()
    };
    out.write(&formatted)?;
    Ok(())
}

fn fmt_datetime(h: &Helper<'_>, _: &Handlebars<'_>, _: &Context, _: &mut RenderContext<'_, '_>, out: &mut dyn Output) -> HelperResult {
    let val = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let formatted = if val.is_empty() {
        "\u{2014}".into()
    } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(val) {
        dt.format("%d.%m.%Y %H:%M").to_string()
    } else {
        val.to_string()
    };
    out.write(&formatted)?;
    Ok(())
}

fn fmt_number(h: &Helper<'_>, _: &Handlebars<'_>, _: &Context, _: &mut RenderContext<'_, '_>, out: &mut dyn Output) -> HelperResult {
    let val = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
    let abs = val.abs();
    let whole = abs as i64;
    let frac = ((abs - whole as f64) * 100.0).round() as i64;
    let sign = if val < 0.0 { "-" } else { "" };
    let formatted = if frac > 0 {
        format!("{}{}.{}", sign, format_with_delimiters(whole, 0), frac)
    } else {
        format!("{}{}", sign, format_with_delimiters(whole, 0))
    };
    out.write(&formatted)?;
    Ok(())
}

pub fn render_html(template: &PrintTemplate, context: &PrintContext) -> Result<String, String> {
    let mut hbs = Handlebars::new();
    hbs.register_escape_fn(handlebars::no_escape);

    hbs.register_helper("format_money", Box::new(fmt_money));
    hbs.register_helper("format_date", Box::new(fmt_date));
    hbs.register_helper("format_datetime", Box::new(fmt_datetime));
    hbs.register_helper("format_number", Box::new(fmt_number));

    hbs.register_template_string("print_layout", generate_layout(template))
        .map_err(|e| format!("Layout register error: {}", e))?;
    hbs.register_template_string("print_body", &template.template_body)
        .map_err(|e| format!("Template register error: {}", e))?;

    let data = serde_json::json!({
        "object": context.object,
        "entity_type": context.entity_type,
        "company": context.company,
        "parent": context.parent,
        "computed": context.computed,
        "print_info": context.print_info,
    });

    hbs.render("print_layout", &data).map_err(|e| format!("Render error: {}", e))
}

fn generate_layout(template: &PrintTemplate) -> String {
    let margins = &template.margins;
    let page_size = template.paper_format.css();
    let page_orient = match template.orientation {
        super::Orientation::Landscape => "landscape",
        super::Orientation::Portrait => "portrait",
    };
    let user_css = &template.css_styles;

    format!(
        r#"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="utf-8">
<title>Печатная форма</title>
<style>
@page {{ {page_size}; {orient}; margin: {mt}mm {mr}mm {mb}mm {ml}mm; }}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: "Times New Roman", Times, serif; font-size: 11pt; line-height: 1.4; color: #000; background: #fff; padding: 15mm; }}
table {{ width: 100%; border-collapse: collapse; margin: 8pt 0; font-size: 10pt; }}
th, td {{ border: 1px solid #333; padding: 4pt 6pt; text-align: left; vertical-align: top; }}
th {{ background: #f0f0f0; font-weight: bold; }}
.text-right {{ text-align: right; }}
.text-center {{ text-align: center; }}
.text-bold {{ font-weight: bold; }}
.font-lg {{ font-size: 14pt; }}
.font-md {{ font-size: 12pt; }}
.mt-1 {{ margin-top: 8pt; }}
.mt-2 {{ margin-top: 16pt; }}
.mb-1 {{ margin-bottom: 8pt; }}
.mb-2 {{ margin-bottom: 16pt; }}
@media print {{
  body {{ padding: 0; }}
  .no-print {{ display: none !important; }}
}}
{user_css}
</style>
</head>
<body>
{{{{> print_body}}}}
</body>
</html>"#,
        page_size = page_size,
        orient = page_orient,
        mt = margins.top,
        mr = margins.right,
        mb = margins.bottom,
        ml = margins.left,
        user_css = user_css,
    )
}

fn format_with_delimiters(whole: i64, frac: i64) -> String {
    let sign = if whole < 0 { "-" } else { "" };
    let abs = whole.unsigned_abs();
    let s = abs.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ' ');
        }
        result.insert(0, c);
    }
    if frac > 0 {
        format!("{}{},{:02}", sign, result, frac)
    } else {
        format!("{}{}", sign, result)
    }
}
