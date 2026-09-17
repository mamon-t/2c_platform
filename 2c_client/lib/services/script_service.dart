import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/script.dart';
import '../providers/app_providers.dart';
import '../services/rpc_client.dart';

/// Результат `script.validate`: контракт `{valid, errors:[{line, column,
/// message}]}` (15.1). Координаты опциональны (нарушение привязки к типу
/// сущности приходит без них; `<1` — метка «без позиции»).
class ScriptValidation {
  const ScriptValidation({required this.valid, required this.errors});

  final bool valid;
  final List<ScriptValidationError> errors;

  factory ScriptValidation.fromJson(Map<String, dynamic> json) {
    final rawErrors = json['errors'];
    return ScriptValidation(
      valid: json['valid'] as bool? ?? false,
      errors: rawErrors is List
          ? rawErrors
              .map((e) => ScriptValidationError.fromJson(
                  (e as Map).cast<String, dynamic>()))
              .toList()
          : const [],
    );
  }
}

/// Отдельная ошибка валидации исходника скрипта.
class ScriptValidationError {
  const ScriptValidationError({
    required this.message,
    this.line,
    this.column,
  });

  final String message;
  final int? line;
  final int? column;

  factory ScriptValidationError.fromJson(Map<String, dynamic> json) {
    final message = json['message'];
    return ScriptValidationError(
      message: message is String
          ? message
          : 'Неизвестная ошибка проверки скрипта',
      line: json['line'] is int ? json['line'] as int : null,
      column: json['column'] is int ? json['column'] as int : null,
    );
  }
}

/// Управление скриптами Rhai (`script.*`, ТЗ §15).
///
/// Wire-контракт проверен по `apps/platform-server/src/commands.rs`:
/// `list/get/create/update/delete` — стандартные CRUD; `validate` —
/// структурная проверка `${valid, errors}`; `test` — прогон без сохранения
/// (`${result, execution_time_ms}`, поля ввода `code/args`).
class ScriptService {
  ScriptService(this._rpc);

  final RpcClient _rpc;

  /// Список скриптов компании (`script.list`).
  Future<List<ScriptItem>> list({String? companyId}) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'script.list',
      payload: {'company_id': ?companyId},
    );
    if (payload is! List) {
      throw const FormatException('script.list: ожидался массив');
    }
    return payload
        .map((s) => ScriptItem.fromJson((s as Map).cast<String, dynamic>()))
        .toList();
  }

  /// Один скрипт по коду (`script.get`).
  Future<ScriptItem> get(String code, {String? companyId}) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'script.get',
      payload: {'code': code, 'company_id': ?companyId},
    );
    if (payload is! Map) {
      throw const FormatException('script.get: ожидался объект');
    }
    return ScriptItem.fromJson(payload.cast<String, dynamic>());
  }

  /// Создание скрипта (`script.create`).
  Future<ScriptItem> create({
    required String code,
    required String name,
    required String source,
    String scriptType = 'formula',
    String? companyId,
    String? entityType,
    bool isActive = true,
  }) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'script.create',
      payload: {
        'code': code,
        'name': name,
        'source': source,
        'script_type': scriptType,
        'is_active': isActive,
        'company_id': ?companyId,
        'entity_type': ?entityType,
      },
    );
    if (payload is! Map) {
      throw const FormatException('script.create: ожидался объект');
    }
    return ScriptItem.fromJson(payload.cast<String, dynamic>());
  }

  /// Обновление скрипта (`script.update`); идентификация по коду.
  Future<ScriptItem> update(
    String code, {
    String? companyId,
    String? name,
    String? source,
    String? scriptType,
    String? entityType,
    bool? isActive,
  }) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'script.update',
      payload: {
        'code': code,
        'company_id': ?companyId,
        'name': ?name,
        'source': ?source,
        'script_type': ?scriptType,
        'entity_type': ?entityType,
        'is_active': ?isActive,
      },
    );
    if (payload is! Map) {
      throw const FormatException('script.update: ожидался объект');
    }
    return ScriptItem.fromJson(payload.cast<String, dynamic>());
  }

  /// Удаление скрипта (`script.delete`).
  Future<void> delete(String code, {String? companyId}) async {
    await _rpc.queryAny(
      module: 'core',
      action: 'script.delete',
      payload: {'code': code, 'company_id': ?companyId},
    );
  }

  /// Структурная проверка исходника без сохранения (`script.validate`).
  Future<ScriptValidation> validate(String source) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'script.validate',
      payload: {'source': source},
    );
    if (payload is! Map) {
      throw const FormatException('script.validate: ожидался объект');
    }
    return ScriptValidation.fromJson(payload.cast<String, dynamic>());
  }

  /// Тестовый прогон сохранённого скрипта (`script.test`): результат
  /// выполнения и время (мс). Параметры `args` пробрасываются в `ctx.args`.
  Future<Map<String, dynamic>> testRun(
    String code, {
    Map<String, dynamic>? args,
  }) async {
    final payload = await _rpc.queryAny(
      module: 'core',
      action: 'script.test',
      payload: {'code': code, 'args': ?args},
    );
    if (payload is! Map) {
      throw const FormatException('script.test: ожидался объект');
    }
    return payload.cast<String, dynamic>();
  }
}

/// Провайдер сервиса скриптов (живёт весь сеанс).
final scriptServiceProvider = Provider<ScriptService>((ref) {
  final rpc = ref.watch(rpcClientProvider);
  return ScriptService(rpc);
});