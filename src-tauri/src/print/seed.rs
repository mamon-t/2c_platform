use chrono::Utc;
use mongodb::bson::{doc, Document};
use tracing::info;

use crate::core::PlatformResult;
use crate::db::MongoClient;
use super::{PrintTemplate, PaperFormat, Orientation, PrintMargins};

const COLLECTION: &str = "print_templates";

pub async fn seed_templates(db: &MongoClient) -> PlatformResult<()> {
    let col = db.collection::<Document>(COLLECTION);

    let templates = vec![
        seed_document(),
        seed_osv(),
        seed_journal(),
    ];

    for tmpl in templates {
        let doc = serialize(&tmpl);
        col.insert_one(doc).await.ok();
        info!("Seeded print template: {}", tmpl.code);
    }

    info!("Print templates seeded: 3 templates");
    Ok(())
}

fn seed_document() -> PrintTemplate {
    PrintTemplate {
        _id: uuid::Uuid::new_v4(),
        code: "document_standard".into(),
        name: "Документ операции".into(),
        entity_type: "document".into(),
        form_code: "document_print".into(),
        template_body: DOCUMENT_TEMPLATE.into(),
        css_styles: String::new(),
        paper_format: PaperFormat::A4,
        orientation: Orientation::Portrait,
        margins: PrintMargins::default(),
        is_default: true,
        is_active: true,
        version: 1,
        valid_from: None,
        valid_to: None,
        company_id: None,
        before_print_script: None,
        created_by: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn seed_osv() -> PrintTemplate {
    PrintTemplate {
        _id: uuid::Uuid::new_v4(),
        code: "osv_standard".into(),
        name: "Оборотно-сальдовая ведомость".into(),
        entity_type: "report".into(),
        form_code: "osv_print".into(),
        template_body: OSV_TEMPLATE.into(),
        css_styles: String::new(),
        paper_format: PaperFormat::A4,
        orientation: Orientation::Landscape,
        margins: PrintMargins::default(),
        is_default: true,
        is_active: true,
        version: 1,
        valid_from: None,
        valid_to: None,
        company_id: None,
        before_print_script: None,
        created_by: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn seed_journal() -> PrintTemplate {
    PrintTemplate {
        _id: uuid::Uuid::new_v4(),
        code: "journal_standard".into(),
        name: "Журнал проводок".into(),
        entity_type: "report".into(),
        form_code: "journal_print".into(),
        template_body: JOURNAL_TEMPLATE.into(),
        css_styles: String::new(),
        paper_format: PaperFormat::A4,
        orientation: Orientation::Landscape,
        margins: PrintMargins::default(),
        is_default: true,
        is_active: true,
        version: 1,
        valid_from: None,
        valid_to: None,
        company_id: None,
        before_print_script: None,
        created_by: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

const DOCUMENT_TEMPLATE: &str = r#"
<div class="header">
  <div class="company-name text-bold font-lg">{{company.name}}</div>
  <div class="doc-title font-md mt-1 text-bold">{{entity_type.name}} №{{object.number}}</div>
  <div class="doc-date mt-1">от {{format_date object.date}}</div>
</div>

<hr class="mt-2 mb-2">

<table>
  <tbody>
    <tr>
      <td class="text-bold" style="width:30%">Статус</td>
      <td>{{object.state}}</td>
    </tr>
    {{#each object.data}}
    <tr>
      <td class="text-bold">{{@key}}</td>
      <td>{{this}}</td>
    </tr>
    {{/each}}
  </tbody>
</table>

<div class="mt-2" style="font-size:10pt; color:#666;">
  Дата печати: {{print_info.print_date}} | Версия: {{object.version}}
</div>
"#;

const OSV_TEMPLATE: &str = r#"
<div class="header text-center">
  <div class="font-lg text-bold">ОБОРОТНО-САЛЬДОВАЯ ВЕДОМОСТЬ</div>
  <div class="mt-1">{{company.name}}</div>
  <div class="mt-1" style="font-size:10pt;">на {{print_info.print_date}}</div>
</div>

<table class="mt-2">
  <thead>
    <tr>
      <th style="width:8%">Счёт</th>
      <th style="width:25%">Наименование</th>
      <th class="text-right" style="width:12%">Нач. дебет</th>
      <th class="text-right" style="width:12%">Нач. кредит</th>
      <th class="text-right" style="width:12%">Оборот дебет</th>
      <th class="text-right" style="width:12%">Оборот кредит</th>
      <th class="text-right" style="width:10%">Кон. дебет</th>
      <th class="text-right" style="width:10%">Кон. кредит</th>
    </tr>
  </thead>
  <tbody>
    {{#each object.data.lines}}
    <tr>
      <td class="text-center">{{this.account_code}}</td>
      <td>{{this.account_name}}</td>
      <td class="text-right">{{format_money this.opening_debit}}</td>
      <td class="text-right">{{format_money this.opening_credit}}</td>
      <td class="text-right">{{format_money this.turnover_debit}}</td>
      <td class="text-right">{{format_money this.turnover_credit}}</td>
      <td class="text-right">{{format_money this.closing_debit}}</td>
      <td class="text-right">{{format_money this.closing_credit}}</td>
    </tr>
    {{/each}}
  </tbody>
</table>

<div class="mt-2" style="font-size:10pt; color:#666;">
  Дата печати: {{print_info.print_date}}
</div>
"#;

const JOURNAL_TEMPLATE: &str = r#"
<div class="header text-center">
  <div class="font-lg text-bold">ЖУРНАЛ ПРОВОДОК</div>
  <div class="mt-1">{{company.name}}</div>
  <div class="mt-1" style="font-size:10pt;">на {{print_info.print_date}}</div>
</div>

<table class="mt-2">
  <thead>
    <tr>
      <th style="width:8%">Дата</th>
      <th style="width:8%">Номер</th>
      <th style="width:10%">Дебет</th>
      <th style="width:10%">Кредит</th>
      <th class="text-right" style="width:12%">Сумма</th>
      <th>Описание</th>
    </tr>
  </thead>
  <tbody>
    {{#each object.data.entries}}
    <tr>
      <td class="text-center">{{format_date this.date}}</td>
      <td class="text-center">{{this.number}}</td>
      <td class="text-center">{{this.account_debit}}</td>
      <td class="text-center">{{this.account_credit}}</td>
      <td class="text-right">{{format_money this.amount}}</td>
      <td>{{this.description}}</td>
    </tr>
    {{/each}}
  </tbody>
</table>

<div class="mt-2" style="font-size:10pt; color:#666;">
  Дата печати: {{print_info.print_date}}
</div>
"#;

fn serialize(t: &PrintTemplate) -> Document {
    let mut doc = Document::new();
    doc.insert("_id", t._id.to_string());
    doc.insert("code", &t.code);
    doc.insert("name", &t.name);
    doc.insert("entity_type", &t.entity_type);
    doc.insert("form_code", &t.form_code);
    doc.insert("template_body", &t.template_body);
    doc.insert("css_styles", &t.css_styles);
    doc.insert("paper_format", serde_json::to_string(&t.paper_format).unwrap_or_default().trim_matches('"'));
    doc.insert("orientation", serde_json::to_string(&t.orientation).unwrap_or_default().trim_matches('"'));
    if let Ok(bson) = mongodb::bson::to_bson(&t.margins) { doc.insert("margins", bson); }
    doc.insert("is_default", t.is_default);
    doc.insert("is_active", t.is_active);
    doc.insert("version", t.version);
    if let Some(ref s) = t.before_print_script { doc.insert("before_print_script", s); }
    doc.insert("created_at", mongodb::bson::to_bson(&t.created_at).unwrap());
    doc.insert("updated_at", mongodb::bson::to_bson(&t.updated_at).unwrap());
    doc
}
