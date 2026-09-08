//! Хост WASM-модулей на базе Extism 1.30 (раздел 9 ТЗ): загрузка и валидация
//! манифеста через `get_info()`, исполнение экспортируемых функций с
//! ресурсными лимитами (топливо, память, таймауты) и host-функции подфазы 8a
//! (контекст/сервис и KV-хранилище) в namespace `ExtismHost`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use core_application::ports::WasmHost;
use core_domain::error::DomainError;
use core_domain::event::ActorSnapshot;
use core_domain::wasm_manifest::ModuleManifest;
use extism::{CurrentPlugin, Function, Plugin, PluginBuilder, UserData, Val, ValType, PTR};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use tokio::runtime::Handle;
use tokio::sync::RwLock;

use crate::module_kv::ModuleKv;

/// Namespace импортов host-функций, заявленный в ТЗ (Приложение №4).
const NS_HOST: &str = "ExtismHost";

/// Лимит топлива на исполнение плагина (раздел 9 ТЗ v3.1).
const FUEL_LIMIT: u64 = 10_000_000;
/// Максимум страниц памяти плагина (256 страниц ≈ 16 МБ).
const MAX_PAGES: u32 = 256;
/// Таймаут внутри плагина (прерывание через fuel/эпоху wasmtime).
const PLUGIN_TIMEOUT: Duration = Duration::from_secs(10);
/// Таймаут на весь вызов экспортируемой функции (включая host-функции).
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Контекст единичного вызова модуля: кто вызывает, для какой компании,
/// какие capabilities выданы модулю и какие настройки установлены.
/// Передаётся в `call_with_host_context` и доступен каждой host-функции.
#[derive(Debug, Clone, Default)]
pub struct HostCallCtx {
    /// Код вызываемого модуля.
    pub module_code: String,
    /// Компания, в контексте которой исполняется вызов.
    pub company_id: String,
    /// Исполнитель действия; `None` для системных операций.
    pub actor: Option<ActorSnapshot>,
    /// Гранты модуля из манифеста.
    pub capabilities: HashSet<String>,
    /// Настройки модуля для текущей компании (`module_settings`).
    pub settings: Value,
}

/// Общие для всех плагинов данные: актуальный контекст вызова, KV-хранилище
/// и handle текущего tokio-runtime для выполнения асинхронных операций
/// SurrealDB из синхронных host-функций.
struct HostShared {
    ctx: RwLock<HostCallCtx>,
    kv: ModuleKv,
    runtime: Handle,
}

impl HostShared {
    /// Клон обеспечивает тот же клиент SurrealDB, что и исходный.
    fn kv_module(&self) -> ModuleKv {
        self.kv.clone()
    }
}

/// Загруженный в память модуль вместе со своим манифестом.
struct LoadedModule {
    manifest: ModuleManifest,
    /// Извлекается из модуля на время вызова и возвращается обратно,
    /// поэтому хранится в `Option`.
    plugin: Option<Plugin>,
}

/// Хост WASM-модулей с host-функциями подфазы 8a.
pub struct ExtismWasmHost {
    modules: RwLock<HashMap<String, LoadedModule>>,
    shared: Arc<HostShared>,
    cache_dir: PathBuf,
}

/// Итог host-функции: строка результата или строка-конверт ошибки.
type HostFnResult = Result<String, String>;

impl ExtismWasmHost {
    /// Возвращает манифест загруженного модуля (используется при
    /// декларативной регистрации в подфазе 9b).
    pub async fn manifest(&self, code: &str) -> Option<ModuleManifest> {
        self.modules
            .read()
            .await
            .get(code)
            .map(|m| m.manifest.clone())
    }
    /// Создаёт хост. `db` используется для KV-хранилища (`module_kv`).
    /// Должен вызываться внутри tokio-runtime (берётся `Handle::current()`).
    ///
    /// # Errors
    ///
    /// Возвращает `DomainError::Storage`, если хост создан вне tokio-runtime.
    pub fn new(db: Surreal<Any>, cache_dir: PathBuf) -> Result<Self, DomainError> {
        let runtime = Handle::try_current()
            .map_err(|_| DomainError::Storage("WasmHost требует tokio-runtime".to_string()))?;
        Ok(Self {
            shared: Arc::new(HostShared {
                ctx: RwLock::new(HostCallCtx::default()),
                kv: ModuleKv::new(db),
                runtime,
            }),
            modules: RwLock::new(HashMap::new()),
            cache_dir,
        })
    }

    /// Настраивает хранилище KV идемпотентно.
    pub async fn ensure_schema(&self) -> Result<(), DomainError> {
        self.shared.kv.ensure_schema().await
    }

    /// Устанавливает контекст для последующих вызовов модулей.
    pub async fn set_call_context(&self, ctx: HostCallCtx) {
        *self.shared.ctx.write().await = ctx;
    }

    /// Возвращает текущий контекст вызова (для тестов и отладки).
    pub async fn call_context(&self) -> HostCallCtx {
        self.shared.ctx.read().await.clone()
    }

    /// Набор host-функций подфазы 8a. Каждая функция проверяет capability
    /// модуля из контекста перед выполнением.
    fn host_functions(&self) -> Vec<Function> {
        let mut funcs = Vec::new();

        funcs.push(Function::new(
            "whoami",
            Vec::<ValType>::new(),
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "whoami", None, |ctx, _args| {
                    let actor = ctx.actor.as_ref().map(|a| {
                        json!({
                            "user_id": a.user_id.map(|id| id.to_string()),
                            "login": a.login,
                            "display_name": a.full_name,
                            "position": a.position,
                            "company_id": a.company_id.map(|id| id.to_string()),
                            "role_id": Value::Null,
                            "role_ids": Value::Array(vec![]),
                        })
                    });
                    Ok(envelope_ok(actor.unwrap_or(Value::Null)))
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        funcs.push(Function::new(
            "now_ms",
            Vec::<ValType>::new(),
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "now_ms", None, |_ctx, _args| {
                    Ok(now_ms().to_string())
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        funcs.push(Function::new(
            "module_settings",
            Vec::<ValType>::new(),
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "module_settings", None, |ctx, _args| {
                    Ok(envelope_ok(ctx.settings.clone()))
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        funcs.push(Function::new(
            "log_message",
            vec![PTR],
            vec![],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(
                    plugin,
                    inputs,
                    outputs,
                    "log_message",
                    Some("logging"),
                    |ctx, args| {
                        let msg = args.first().cloned().unwrap_or_default();
                        tracing::info!(module = %ctx.module_code, company = %ctx.company_id, "Module: {}", msg);
                        Ok(String::new())
                    },
                );
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        let shared = self.shared.clone();
        funcs.push(Function::new(
            "kv_put",
            vec![PTR, PTR],
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "kv_put", Some("storage"), |ctx, args| {
                    let key = args.first().cloned().unwrap_or_default();
                    let value = parse_arg(&args.get(1).cloned().unwrap_or_default())?;
                    if key.is_empty() {
                        return Err(envelope_err("INVALID_JSON", "kv_put: пустой ключ"));
                    }
                    let kv = shared.kv_module();
                    let company = ctx.company_id.clone();
                    let module = ctx.module_code.clone();
                    block_on_kv(
                        &shared,
                        ctx,
                        format!("kv_put:{key}"),
                        async move {
                            kv.put(&company, &module, &key, value).await?;
                            Ok(envelope_ok(Value::Null))
                        },
                    )
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        let shared = self.shared.clone();
        funcs.push(Function::new(
            "kv_put_if_absent",
            vec![PTR, PTR],
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "kv_put_if_absent", Some("storage"), |ctx, args| {
                    let key = args.first().cloned().unwrap_or_default();
                    let value = parse_arg(&args.get(1).cloned().unwrap_or_default())?;
                    if key.is_empty() {
                        return Err(envelope_err("INVALID_JSON", "kv_put_if_absent: пустой ключ"));
                    }
                    let kv = shared.kv_module();
                    let company = ctx.company_id.clone();
                    let module = ctx.module_code.clone();
                    block_on_kv(
                        &shared,
                        ctx,
                        format!("kv_put_if_absent:{key}"),
                        async move {
                            let inserted = kv
                                .put_if_absent(&company, &module, &key, value)
                                .await?;
                            Ok(envelope_ok(Value::Bool(inserted)))
                        },
                    )
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        let shared = self.shared.clone();
        funcs.push(Function::new(
            "kv_get",
            vec![PTR],
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "kv_get", Some("storage"), |ctx, args| {
                    let key = args.first().cloned().unwrap_or_default();
                    let kv = shared.kv_module();
                    let company = ctx.company_id.clone();
                    let module = ctx.module_code.clone();
                    block_on_kv(
                        &shared,
                        ctx,
                        format!("kv_get:{key}"),
                        async move {
                            let value = kv.get(&company, &module, &key).await?;
                            Ok(envelope_ok(value.unwrap_or(Value::Null)))
                        },
                    )
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        let shared = self.shared.clone();
        funcs.push(Function::new(
            "kv_list",
            vec![PTR],
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "kv_list", Some("storage"), |ctx, args| {
                    let prefix = args.first().cloned().unwrap_or_default();
                    let kv = shared.kv_module();
                    let company = ctx.company_id.clone();
                    let module = ctx.module_code.clone();
                    block_on_kv(
                        &shared,
                        ctx,
                        format!("kv_list:{prefix}"),
                        async move {
                            let rows = kv.list(&company, &module, &prefix).await?;
                            let arr = rows
                                .into_iter()
                                .map(|(k, v)| json!({ "key": k, "value": v }))
                                .collect::<Vec<_>>();
                            Ok(envelope_ok(Value::Array(arr)))
                        },
                    )
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        let shared = self.shared.clone();
        funcs.push(Function::new(
            "kv_delete",
            vec![PTR],
            vec![PTR],
            UserData::new(()),
            move |plugin, inputs, outputs, _ud| {
                host_fn_dispatch(plugin, inputs, outputs, "kv_delete", Some("storage"), |ctx, args| {
                    let key = args.first().cloned().unwrap_or_default();
                    let kv = shared.kv_module();
                    let company = ctx.company_id.clone();
                    let module = ctx.module_code.clone();
                    block_on_kv(
                        &shared,
                        ctx,
                        format!("kv_delete:{key}"),
                        async move {
                            let deleted = kv.delete(&company, &module, &key).await?;
                            Ok(envelope_ok(Value::Bool(deleted)))
                        },
                    )
                });
                Ok(())
            },
        )
        .with_namespace(NS_HOST));

        funcs
    }

    /// Путь к кэш-копии модуля: `{dir}/{code}-{sha256:16}.wasm`.
    fn cache_path(&self, code: &str, wasm: &[u8]) -> PathBuf {
        let digest = Sha256::digest(wasm);
        self.cache_dir.join(format!(
            "{code}-{}.wasm",
            digest[..8]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ))
    }

    /// Пишет бинарник модуля в кэш (сбой записи не критичен).
    fn write_cache(&self, code: &str, wasm: &[u8]) {
        if self.cache_dir.as_os_str().is_empty() {
            return;
        }
        let path = self.cache_path(code, wasm);
        if std::fs::create_dir_all(&self.cache_dir).is_ok() {
            let _ = std::fs::write(path, wasm);
        }
    }
}

impl WasmHost for ExtismWasmHost {
    async fn load_module(
        &self,
        code: &str,
        wasm_bytes: &[u8],
    ) -> Result<ModuleManifest, DomainError> {
        if code.is_empty() {
            return Err(DomainError::ValidationError(
                "load_module: пустой код модуля".to_string(),
            ));
        }
        let fallback_ctx = HostCallCtx {
            module_code: code.to_string(),
            ..HostCallCtx::default()
        };

        let extism_manifest = extism::Manifest::new([extism::Wasm::data(wasm_bytes.to_vec())])
            .with_memory_max(MAX_PAGES)
            .with_timeout(PLUGIN_TIMEOUT);

        let mut plugin = PluginBuilder::new(extism_manifest)
            .with_functions(self.host_functions())
            .with_fuel_limit(FUEL_LIMIT)
            .with_wasi(false)
            .build()
            .map_err(|e| DomainError::ValidationError(format!("компиляция плагина: {e}")))?;

        let raw = plugin
            .call_with_host_context::<Vec<u8>, Vec<u8>, HostCallCtx>(
                "get_info",
                Vec::new(),
                fallback_ctx,
            )
            .map_err(|e| DomainError::ValidationError(format!("get_info: {e}")))?;
        let manifest: ModuleManifest = serde_json::from_slice(&raw).map_err(|e| {
            DomainError::ValidationError(format!("get_info вернула невалидный манифест: {e}"))
        })?;
        manifest.validate()?;
        if manifest.code != code {
            return Err(DomainError::ValidationError(format!(
                "код манифеста {} не совпадает с ожидаемым {code}",
                manifest.code
            )));
        }

        self.write_cache(code, wasm_bytes);
        self.modules.write().await.insert(
            code.to_string(),
            LoadedModule {
                manifest: manifest.clone(),
                plugin: Some(plugin),
            },
        );
        Ok(manifest)
    }

    async fn call_function(
        &self,
        code: &str,
        function: &str,
        input: &[u8],
    ) -> Result<Vec<u8>, DomainError> {
        let ctx = self.shared.ctx.read().await.clone();
        let mut guard = self.modules.write().await;
        let loaded = guard
            .get_mut(code)
            .ok_or_else(|| DomainError::NotFound(format!("модуль не загружен: {code}")))?;
        let mut plugin = loaded
            .plugin
            .take()
            .ok_or_else(|| {
                DomainError::ValidationError(format!("модуль занят другим вызовом: {code}"))
            })?;

        let input_bytes = input.to_vec();
        let function_owned = function.to_string();
        let handle = tokio::task::spawn_blocking(move || {
            let out = plugin
                .call_with_host_context::<Vec<u8>, Vec<u8>, HostCallCtx>(
                    &function_owned,
                    input_bytes,
                    ctx,
                )
                .map_err(|e| e.to_string());
            (plugin, out)
        });

        let outcome = tokio::time::timeout(CALL_TIMEOUT, handle).await;
        match outcome {
            Ok(Ok((plugin_back, Ok(output)))) => {
                if let Some(loaded) = guard.get_mut(code) {
                    loaded.plugin = Some(plugin_back);
                }
                Ok(output)
            }
            Ok(Ok((plugin_back, Err(e)))) => {
                if let Some(loaded) = guard.get_mut(code) {
                    loaded.plugin = Some(plugin_back);
                }
                Err(DomainError::ValidationError(format!(
                    "вызов {function} модуля {code}: {e}"
                )))
            }
            Ok(Err(je)) => {
                guard.remove(code);
                Err(DomainError::ValidationError(format!(
                    "вызов {function} модуля {code} прерван: {je}"
                )))
            }
            Err(elapsed) => {
                guard.remove(code);
                Err(DomainError::ValidationError(format!(
                    "вызов {function} модуля {code}: таймаут {elapsed}"
                )))
            }
        }
    }

    async fn unload_module(&self, code: &str) -> Result<(), DomainError> {
        if self.modules.write().await.remove(code).is_some() {
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("модуль не загружен: {code}")))
        }
    }

    async fn is_loaded(&self, code: &str) -> bool {
        self.modules.read().await.contains_key(code)
    }
}

/// Общая развёртка host-функции: чтение аргументов, проверка capability,
/// вызов обработчика и запись результата в память плагина. Ошибки
/// возвращаются плагину строкой-конвертом (код — внутри текста результата).
fn host_fn_dispatch(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    name: &str,
    capability: Option<&str>,
    f: impl Fn(&HostCallCtx, Vec<String>) -> HostFnResult,
) {
    let result: HostFnResult = (|| {
        let mut args = Vec::with_capacity(inputs.len());
        for input in inputs {
            let s: String = plugin.memory_get_val(input).map_err(|e| {
                envelope_err("HOST_ERROR", &format!("чтение аргумента {name}: {e}"))
            })?;
            args.push(s);
        }
        let ctx = plugin
            .host_context::<HostCallCtx>()
            .map_err(|e| envelope_err("HOST_ERROR", &format!("отсутствует контекст {name}: {e}")))?
            .clone();
        if let Some(cap) = capability {
            if !ctx.capabilities.contains(cap) {
                return Err(envelope_err(
                    "CAPABILITY_DENIED",
                    &format!("модуль {} не имеет capability {cap}", ctx.module_code),
                ));
            }
        }
        f(&ctx, args)
    })();

    let output = match result {
        Ok(s) => s,
        Err(e) => e,
    };
    if let Ok(handle) = plugin.memory_new(output.as_bytes().to_vec()) {
        if !outputs.is_empty() {
            outputs[0] = plugin.memory_to_val(handle);
        }
    }
}

/// Текущее время хоста в unix-миллисекундах.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Разбирает JSON-строку аргумента host-функции.
fn parse_arg(raw: &str) -> Result<Value, String> {
    serde_json::from_str(raw)
        .map_err(|e| envelope_err("INVALID_JSON", &format!("невалидный JSON аргумента: {e}")))
}

/// Конверт успеха host-функции (Приложение №6 ТЗ).
fn envelope_ok(data: Value) -> String {
    json!({ "ok": true, "data": data }).to_string()
}

/// Конверт ошибки host-функции (Приложение №6 ТЗ).
fn envelope_err(code: &str, message: &str) -> String {
    json!({ "ok": false, "error": { "code": code, "message": message } }).to_string()
}

/// Разворачивает конверт host-функции (Приложение №6 ТЗ).
pub fn unwrap_host(raw: &str) -> Result<Value, DomainError> {
    let v: Value = serde_json::from_str(raw)
        .map_err(|e| DomainError::ValidationError(format!("невалидный конверт хоста: {e}")))?;
    if v.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(v.get("data").cloned().unwrap_or(Value::Null))
    } else {
        let code = v["error"]["code"].as_str().unwrap_or("UNKNOWN");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        Err(DomainError::ValidationError(format!("{code}: {msg}")))
    }
}

/// Исполняет асинхронную KV-операцию SurrealDB из синхронного контекста
/// host-функции: задача ставится в текущий tokio-runtime, результат
/// ожидается блокирующе через канал. Итоговое значение — строка-конверт
/// (успех или ошибка), которую host-функция вернёт плагину.
fn block_on_kv(
    shared: &Arc<HostShared>,
    ctx: &HostCallCtx,
    op: String,
    fut: impl std::future::Future<Output = Result<String, DomainError>> + Send + 'static,
) -> HostFnResult {
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, DomainError>>();
    std::mem::drop(shared.runtime.spawn(async move {
        let _ = tx.send(fut.await);
    }));
    rx.recv()
        .map_err(|_| envelope_err("DB_ERROR", &format!("KV-операция {op} не выполнена")))?
        .map_err(|e| {
            tracing::warn!(
                module = %ctx.module_code,
                company = %ctx.company_id,
                "{op}: {e}"
            );
            envelope_err("DB_ERROR", &format!("{op}: {e}"))
        })
}