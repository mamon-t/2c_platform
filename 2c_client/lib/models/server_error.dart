/// Клиентский код ошибки: совпадает с серверным `RpcMessage::Error.code`
/// либо `AUTH_REQUIRED` — локальная концепция (см. `ServerError`).
enum ErrorCode {
  notFound('NOT_FOUND_ERROR'),
  conflict('CONFLICT_ERROR'),
  validation('VALIDATION_ERROR'),
  permission('PERMISSION_ERROR'),
  storage('STORAGE_ERROR'),
  network('NETWORK_ERROR'),
  authRequired('AUTH_REQUIRED'),
  unknown('UNKNOWN_ERROR');

  const ErrorCode(this.wireValue);

  final String wireValue;

  static ErrorCode fromWire(String value) {
    for (final code in ErrorCode.values) {
      if (code.wireValue == value) {
        return code;
      }
    }
    return ErrorCode.unknown;
  }
}

/// Маппинг серверных кодов (`crates/core-domain/src/error.rs`) на русские
/// сообщения для UI. Транспортные/клиентские ошибки — `NETWORK_ERROR`,
/// концепция «сессия истекла» — `AUTH_REQUIRED`.
class ServerError implements Exception {
  const ServerError(this.code, this.message, {this.details});

  final ErrorCode code;

  /// Сообщение `RpcMessage::Error.message` (с сервера, уже русское) либо
  /// локальный текст для клиентских кодов.
  final String message;

  final Map<String, dynamic>? details;

  /// `AUTH_REQUIRED` — верный признак «уйти на /login».
  bool get isAuthRequired => code == ErrorCode.authRequired;

  /// Человекочитаемое сообщение для UI (уже русское — с сервера либо локальное).
  String get localizedMessage {
    if (message.isNotEmpty) {
      return message;
    }
    switch (code) {
      case ErrorCode.notFound:
        return 'Объект не найден';
      case ErrorCode.conflict:
        return 'Конфликт версий';
      case ErrorCode.validation:
        return 'Невалидные данные';
      case ErrorCode.permission:
        return 'Недостаточно прав';
      case ErrorCode.storage:
        return 'Ошибка хранилища';
      case ErrorCode.network:
        return 'Нет соединения с сервером';
      case ErrorCode.authRequired:
        return 'Требуется вход в систему';
      case ErrorCode.unknown:
        return 'Неизвестная ошибка';
    }
  }

  @override
  String toString() => 'ServerError(${code.wireValue}): $message';
}