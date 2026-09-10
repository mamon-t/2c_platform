//! Декларативная регистрация WASM-модулей (подфаза 9b): применяет манифест
//! без запуска кода плагина. Согласно разделу 9 ТЗ манифест — единственный
//! источник правды: политики, схемы, команды и ресурсы регистрируются
//! идемпотентно, а код плагина исполняется только при вызове команды
//! `plugin.{code}.{name}`.

use core_domain::audit::{AuditEntry, AuditResult, AuditTarget};
use core_domain::error::DomainError;
use core_domain::event::{ActorSnapshot, Event, StreamType};
use core_domain::metadata::{EntityField, EntityType, FieldType};
use core_domain::module::{
    CompanyModulePayload, ModuleLifecyclePayload, ModuleRecord, ModuleState, PluginCallContext,
};
use core_domain::permission::PermissionPolicy;
use core_domain::wasm_manifest::{ManifestPermission, ModuleManifest};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::app_registry::AppRegistry;
use crate::command_registry::CommandMetadata;
use crate::ports::{
    AuditRepository, EntitySchema, MetadataRepository, ModuleRepository, PermissionPolicyRepository,
    WasmHost,
};

/// Итог предзагрузки при старте сервера: сколько модулей обработано, для
/// скольких компаний применены декларативные ресурсы и какие ошибки возникли
/// по отдельным модулям (не фатальны — сервер продолжает старт).
#[derive(Debug, Clone, Default)]
pub struct PreloadReport {
    /// Число установленных модулей, загруженных и зарегистрированных успешно.
    pub modules_loaded: usize,
    /// Суммарное число включений (пар «модуль—компания»), обработанных успешно.
    pub companies_affected: usize,
    /// Описания ошибок отдельных модулей/компаний; пусто при полностью
    /// успешном старте.
    pub errors: Vec<String>,
}

/// Применяет манифест на стороне платформы без выполнения кода плагина.
pub struct ModuleManager {
    host: Arc<dyn WasmHost>,
    modules: Arc<dyn ModuleRepository>,
    app: Arc<AppRegistry>,
    policies: Arc<dyn PermissionPolicyRepository>,
    metadata: Arc<dyn MetadataRepository>,
    audit: Arc<dyn AuditRepository>,
    cache_dir: PathBuf,
}

impl ModuleManager {
    pub fn new(
        host: Arc<dyn WasmHost>,
        modules: Arc<dyn ModuleRepository>,
        app: Arc<AppRegistry>,
        policies: Arc<dyn PermissionPolicyRepository>,
        metadata: Arc<dyn MetadataRepository>,
        audit: Arc<dyn AuditRepository>,
        cache_dir: PathBuf,
    ) -> Self {
        Self {
            host,
            modules,
            app,
            policies,
            metadata,
            audit,
            cache_dir,
        }
    }

    /// Декларативно применяет манифест для компании `company_id`:
    /// политики — upsert, схемы объектов — ensure `create_entity_type`,
    /// команды — `plugin.{code}.{name}`, печатные формы и скрипты — ensure,
    /// аудит `seed_metadata`. Вызов идемпотентен.
    pub async fn register(
        &self,
        manifest: &ModuleManifest,
        company_id: &str,
    ) -> Result<(), DomainError> {
        self.app.register_module(&manifest.code).await?;

        for permission in &manifest.permissions {
            let policy = build_policy(&manifest.code, permission);
            self.policies.upsert(&policy).await?;
        }

        for schema in &manifest.object_schemas {
            let entity_schema = build_schema(manifest, schema, company_id);
            let event = schema_event(&entity_schema, "metadata.entity_type.registered");
            self.metadata.create_entity_type(&entity_schema, &[event]).await?;
        }

        for resource in &manifest.print_templates {
            self.app.print_templates.ensure(&resource.code).await;
        }
        for script in &manifest.scripts {
            self.app.scripts.ensure(&script.code).await;
        }

        self.register_commands(manifest).await?;

        self.audit
            .log(AuditEntry {
                id: Uuid::new_v4(),
                action: "seed_metadata".to_string(),
                actor: core_domain::event::ActorSnapshot::system(),
                target: Some(AuditTarget {
                    entity_type: None,
                    entity_id: None,
                    entity_code: Some(manifest.code.clone()),
                    company_id: Some(
                        Uuid::parse_str(company_id).unwrap_or_else(|_| Uuid::nil()),
                    ),
                }),
                result: AuditResult::Success,
                details: Some(json!({
                    "module": manifest.code,
                    "company_id": company_id,
                    "display_name": manifest.display_name,
                    "version": manifest.version,
                })),
                ip_address: None,
                user_agent: None,
                company_id: Uuid::parse_str(company_id).ok(),
                timestamp: chrono::Utc::now(),
            })
            .await?;

        Ok(())
    }

    /// Регистрирует команды `plugin.{code}.{name}` с handler'ом на вызов
    /// функции модуля (`WasmHost::invoke_with_context`). Перед вызовом
    /// проверяется включение модуля для компании: отключённый модуль
    /// возвращает ошибку (INVALID_ACTION «module disabled»).
    async fn register_commands(&self, manifest: &ModuleManifest) -> Result<(), DomainError> {
        let code = manifest.code.clone();
        let module_capabilities = manifest.capabilities.clone();
        for command in &manifest.commands {
            let fname = command.code.clone();
            let name = format!("plugin.{code}.{fname}");
            let required_permission = command.required_permission.clone();
            let modules = self.modules.clone();
            let host = self.host.clone();
            let code_for_handler = code.clone();
            let caps = module_capabilities.clone();

            self.app
                .commands
                .register_with_metadata(
                    &name,
                    CommandMetadata::requires(
                        required_permission.as_deref().unwrap_or("module.execute"),
                    ),
                    move |params: Value| {
                        let modules = modules.clone();
                        let host = host.clone();
                        let code = code_for_handler.clone();
                        let fname = fname.clone();
                        let caps = caps.clone();
                        async move {
                            let company_id = params
                                .get("company_id")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    DomainError::ValidationError(
                                        "отсутствует обязательный параметр 'company_id'".to_string(),
                                    )
                                })?
                                .to_string();
                            let enabled = modules
                                .is_enabled_for_company(&company_id, &code)
                                .await?;
                            if !enabled {
                                return Err(DomainError::ValidationError(format!(
                                    "INVALID_ACTION: модуль {code} отключён для компании"
                                )));
                            }
                            let input = params
                                .get("input")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    DomainError::ValidationError(
                                        "отсутствует обязательный параметр 'input'".to_string(),
                                    )
                                })?
                                .to_string();
                            let actor = params
                                .get("actor")
                                .cloned()
                                .map(serde_json::from_value::<core_domain::event::ActorSnapshot>)
                                .transpose()
                                .map_err(|e| {
                                    DomainError::ValidationError(format!("некорректный 'actor': {e}"))
                                })?;
                            let settings = params
                                .get("settings")
                                .cloned()
                                .unwrap_or(Value::Null);
                            let ctx = PluginCallContext {
                                module_code: code.clone(),
                                company_id: company_id.clone(),
                                actor,
                                capabilities: caps,
                                settings,
                            };
                            let out = host
                                .invoke_with_context(&code, &fname, input.as_bytes(), ctx)
                                .await?;
                            let output = String::from_utf8_lossy(&out).to_string();
                            Ok(json!({ "output": output }))
                        }
                    },
                )
                .await;
        }
        Ok(())
    }

    /// Убирает модуль из реестров платформы: команды удаляются по префиксу,
    /// четыре реестра кодов очищаются. Метаданные и политики (Доска и Труба)
    /// не удаляются — при повторной установке ensure-семантика восстановит их.
    pub async fn unregister(&self, module_code: &str) -> Result<(), DomainError> {
        self.app.unregister_module(module_code).await
    }

    /// Доступ к хранилищу модулей для команд `module.*` (списки, детализация).
    pub fn modules(&self) -> Arc<dyn ModuleRepository> {
        self.modules.clone()
    }

    /// Устанавливает WASM-модуль: загружает в хост для чтения манифеста,
    /// пишет запись в каталог (Труба + Доска), декларативно регистрирует
    /// ресурсы и включает модуль для компании `company_id`. Повторная
    /// установка удалённого модуля (reinstall) разрешена и фиксируется
    /// отдельным событием/аудитом `module.reinstalled`; установка активного
    /// модуля отклоняется как дубликат.
    pub async fn install(
        &self,
        code: &str,
        wasm_bytes: &[u8],
        company_id: &str,
    ) -> Result<ModuleRecord, DomainError> {
        // Активный модуль нельзя установить повторно; удалённый — можно (reinstall).
        let reinstalling = match self.modules.get(code).await {
            Ok(record) => {
                if record.state == ModuleState::Installed {
                    return Err(DomainError::ValidationError(format!(
                        "Модуль {code} уже установлен"
                    )));
                }
                true
            }
            Err(DomainError::NotFound(_)) => false,
            Err(e) => return Err(e),
        };
        let event_type = if reinstalling {
            "module.reinstalled"
        } else {
            "module.installed"
        };
        let reason = if reinstalling { "reinstall" } else { "install" };

        let manifest = self.host.load_module(code, wasm_bytes).await?;

        let wasm_sha256 = short_sha256(wasm_bytes);
        let record = ModuleRecord {
            code: manifest.code.clone(),
            name: manifest.display_name.clone(),
            description: manifest.description.clone(),
            version: manifest.version.clone(),
            api_version: manifest.api_version.clone(),
            capabilities: manifest.capabilities.clone(),
            metadata_version: manifest.metadata_version,
            state: ModuleState::Installed,
            wasm_sha256,
            manifest: manifest.clone(),
            installed_at: chrono::Utc::now(),
        };

        let events = self.lifecycle_events(&record.code, event_type, reason);
        if let Err(e) = self.modules.install(&record, &events).await {
            self.host.unload_module(&record.code).await?;
            return Err(e);
        }

        if let Err(e) = self.register(&manifest, company_id).await {
            self.host.unload_module(&record.code).await?;
            return Err(e);
        }

        let events = self.company_events(&record.code, company_id, "module.enabled");
        // Включение — проекционная операция; если не удалось, регистрация уже выполнена.
        self.modules
            .enable_for_company(company_id, &record.code, &events)
            .await?;

        self.audit
            .log(self.audit_entry(
                event_type,
                &record.code,
                company_id,
                json!({
                    "name": record.name,
                    "version": record.version,
                    "display_name": manifest.display_name,
                    "reinstall": reinstalling,
                }),
            ))
            .await?;
        Ok(record)
    }

    /// Удаляет модуль: выгружает из хоста, помечает запись как `Uninstalled`
    /// и убирает команды модуля из реестра.
    pub async fn uninstall(&self, code: &str) -> Result<ModuleRecord, DomainError> {
        let events = self.lifecycle_events(code, "module.uninstalled", "uninstall");
        let record = self.modules.uninstall(code, &events).await?;
        self.unregister(code).await?;
        let _ = self.host.unload_module(code).await;
        self.audit
            .log(self.audit_entry("module.uninstalled", code, "", json!({})))
            .await?;
        Ok(record)
    }

    /// Включает модуль для компании: повторно применяет декларативные ресурсы
    /// (политики, схемы, команды) по сохранённому манифесту и создаёт проекцию
    /// `company_modules`.
    pub async fn enable(&self, code: &str, company_id: &str) -> Result<ModuleRecord, DomainError> {
        let record = self.modules.get(code).await?;
        if record.state != ModuleState::Installed {
            return Err(DomainError::ValidationError(format!(
                "Модуль {code} не установлен"
            )));
        }
        self.register(&record.manifest, company_id).await?;
        let events = self.company_events(code, company_id, "module.enabled");
        self.modules
            .enable_for_company(company_id, code, &events)
            .await?;
        self.audit
            .log(self.audit_entry(
                "module.enabled",
                code,
                company_id,
                json!({ "company_id": company_id }),
            ))
            .await?;
        Ok(record)
    }

    /// Отключает модуль для компании (проекция `company_modules`). Команды
    /// остаются в реестре, но выполняются только для включивших модуль компаний.
    pub async fn disable(&self, code: &str, company_id: &str) -> Result<(), DomainError> {
        let events = self.company_events(code, company_id, "module.disabled");
        self.modules
            .disable_for_company(company_id, code, &events)
            .await?;
        self.audit
            .log(self.audit_entry(
                "module.disabled",
                code,
                company_id,
                json!({ "company_id": company_id }),
            ))
            .await?;
        Ok(())
    }

    /// Предзагружает установленные модули при старте сервера (ТЗ v3.1,
    /// раздел 9, `preload_company_modules`): загружает из кэша
    /// `{cache_dir}/{code}-{wasm_sha256}.wasm` модули в состоянии `Installed`
    /// и декларативно регистрирует их ресурсы для всех включивших модуль
    /// компаний. Повторные вызовы идемпотентны (загрузка и регистрация —
    /// ensure-семантики). Ошибки отдельных модулей не фатальны: они
    /// собираются в `PreloadReport.errors`, сервер продолжает старт.
    ///
    /// # Errors
    ///
    /// Возвращает `DomainError::Storage` при отказе чтения каталога манифестов.
    pub async fn preload_all(&self) -> Result<PreloadReport, DomainError> {
        let mut report = PreloadReport::default();
        let mut records: Vec<ModuleRecord> = self
            .modules
            .list()
            .await?
            .into_iter()
            .filter(|r| r.state == ModuleState::Installed)
            .collect();
        records.sort_by_key(|r| r.installed_at);

        for record in records {
            let companies = match self.modules.list_enabled_companies(&record.code).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(module = %record.code, "preload: список компаний: {e}");
                    report.errors.push(format!("{}: список компаний: {e}", record.code));
                    continue;
                }
            };
            if companies.is_empty() {
                continue;
            }

            if !self.host.is_loaded(&record.code).await {
                let path = self
                    .cache_dir
                    .join(format!("{}-{}.wasm", record.code, record.wasm_sha256));
                let bytes = match tokio::fs::read(&path).await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(module = %record.code, path = %path.display(), "preload: кэш недоступен: {e}");
                        report
                            .errors
                            .push(format!("{}: кэш недоступен: {e}", record.code));
                        continue;
                    }
                };
                if let Err(e) = self.host.load_module(&record.code, &bytes).await {
                    tracing::warn!(module = %record.code, "preload: загрузка модуля: {e}");
                    report
                        .errors
                        .push(format!("{}: загрузка: {e}", record.code));
                    continue;
                }
            }

            let mut module_ok = true;
            for company_id in &companies {
                if let Err(e) = self.register(&record.manifest, company_id).await {
                    tracing::warn!(
                        module = %record.code,
                        company = %company_id,
                        "preload: регистрация: {e}"
                    );
                    report.errors.push(format!(
                        "{}: регистрация для {company_id}: {e}",
                        record.code
                    ));
                    module_ok = false;
                }
            }
            if module_ok {
                report.modules_loaded += 1;
            }
            report.companies_affected += companies.len();
        }

        Ok(report)
    }

    fn lifecycle_events(&self, code: &str, event_type: &str, reason: &str) -> Vec<Event> {
        vec![Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Module,
            stream_id: code.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload: serde_json::to_value(ModuleLifecyclePayload {
                reason: reason.to_string(),
            })
            .unwrap_or_else(|_| json!({})),
            metadata: ActorSnapshot::system(),
            company_id: String::new(),
            correlation_id: Uuid::new_v4().to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }]
    }

    fn company_events(&self, code: &str, company_id: &str, event_type: &str) -> Vec<Event> {
        vec![Event {
            id: Uuid::new_v4(),
            stream_type: StreamType::Module,
            stream_id: code.to_string(),
            event_type: event_type.to_string(),
            version: 0,
            payload: serde_json::to_value(CompanyModulePayload {
                company_id: company_id.to_string(),
                module_code: code.to_string(),
            })
            .unwrap_or_else(|_| json!({})),
            metadata: ActorSnapshot::system(),
            company_id: company_id.to_string(),
            correlation_id: Uuid::new_v4().to_string(),
            causation_id: None,
            occurred_at: chrono::Utc::now(),
        }]
    }

    fn audit_entry(
        &self,
        action: &str,
        code: &str,
        company_id: &str,
        details: Value,
    ) -> AuditEntry {
        AuditEntry {
            id: Uuid::new_v4(),
            action: action.to_string(),
            actor: ActorSnapshot::system(),
            target: Some(AuditTarget {
                entity_type: None,
                entity_id: None,
                entity_code: Some(code.to_string()),
                company_id: None,
            }),
            result: AuditResult::Success,
            details: Some(details),
            ip_address: None,
            user_agent: None,
            company_id: if company_id.is_empty() {
                None
            } else {
                Uuid::parse_str(company_id).ok()
            },
            timestamp: chrono::Utc::now(),
        }
    }
}

/// SHA-256 байтов WASM-модуля, первые 16 символов hex — имя файла в кэше.
fn short_sha256(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()[..16]
        .to_string()
}

/// Преобразует политику манифеста в `PermissionPolicy`. Действия объединяются;
/// `entity_type` заполняется, только если все действия ссылаются на один тип.
fn build_policy(module_code: &str, perm: &ManifestPermission) -> PermissionPolicy {
    let mut actions: HashSet<String> = HashSet::new();
    let mut entity_types: HashSet<String> = HashSet::new();
    for action in &perm.actions {
        entity_types.insert(action.entity_type.clone());
        actions.extend(action.actions.iter().cloned());
    }
    let mut actions: Vec<String> = actions.into_iter().collect();
    actions.sort();

    let entity_type = if entity_types.len() == 1 {
        entity_types.iter().next().cloned()
    } else {
        None
    };
    let now = chrono::Utc::now();
    PermissionPolicy {
        id: Uuid::new_v4(),
        code: perm.code.clone(),
        name: perm.code.clone(),
        description: Some(perm.description.clone()),
        scope_type: perm.scope_type.clone(),
        entity_type,
        actions,
        record_access: perm.record_access.clone(),
        deny: false,
        priority: 0,
        module_code: Some(module_code.to_string()),
        is_system: false,
        created_at: now,
        updated_at: now,
    }
}

/// Собирает `EntitySchema` типа сущности модуля для компании `company_id`.
fn build_schema(
    manifest: &ModuleManifest,
    schema: &core_domain::wasm_manifest::ManifestObjectSchema,
    company_id: &str,
) -> EntitySchema {
    let now = chrono::Utc::now();
    let entity_type = EntityType {
        id: Uuid::new_v4(),
        code: schema.code.clone(),
        name: schema.name.clone(),
        kind: schema.kind,
        company_id: company_id.to_string(),
        metadata_version: manifest.metadata_version,
        is_system: true,
        created_at: now,
        updated_at: now,
    };
    let fields = schema
        .fields
        .iter()
        .map(|f| EntityField {
            id: Uuid::new_v4(),
            entity_type: schema.code.clone(),
            code: f.code.clone(),
            label: f.name.clone(),
            data_type: field_type_from_kind(&f.kind),
            required: f.required,
            is_unique: false,
            is_indexed: false,
            options: json!({}),
            is_system: true,
            order: 0,
        })
        .collect();
    EntitySchema {
        entity_type,
        fields,
        states: Vec::new(),
        transitions: Vec::new(),
        forms: Vec::new(),
        actions: Vec::new(),
        relations: Vec::new(),
    }
}

/// Маппинг строковых типов манифеста на `FieldType`.
fn field_type_from_kind(kind: &str) -> FieldType {
    match kind {
        "string" => FieldType::String,
        "text" => FieldType::Text,
        "integer" => FieldType::Integer,
        "money" => FieldType::Money,
        "date" => FieldType::Date,
        "datetime" => FieldType::Datetime,
        "boolean" => FieldType::Boolean,
        "enum" => FieldType::Enum,
        "reference" => FieldType::Reference,
        "array" => FieldType::Array,
        "table" => FieldType::Table,
        "json" => FieldType::Json,
        "file" => FieldType::File,
        "user" => FieldType::User,
        "company" => FieldType::Company,
        _ => FieldType::String,
    }
}

/// Событие регистрации схемы типа сущности модуля.
fn schema_event(schema: &EntitySchema, event_type: &str) -> Event {
    Event {
        id: Uuid::new_v4(),
        stream_type: StreamType::Metadata,
        stream_id: schema.entity_type.id.to_string(),
        event_type: event_type.to_string(),
        version: 0,
        payload: serde_json::to_value(schema).unwrap_or_else(|_| json!({})),
        metadata: core_domain::event::ActorSnapshot::system(),
        company_id: schema.entity_type.company_id.clone(),
        correlation_id: Uuid::new_v4().to_string(),
        causation_id: None,
        occurred_at: chrono::Utc::now(),
    }
}