import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:logging/logging.dart';
import 'package:uuid/uuid.dart';

import '../models/rpc_message.dart';
import '../models/server_error.dart';

/// HTTP-клиент конверта `RpcMessage` над `POST /rpc` (ТЗ §10).
///
/// Код ошибки берётся из тела `RpcMessage::Error` (первично), HTTP-статус —
/// вторично (клиенты, не прошедшие RBAC, получают `403 PERMISSION_ERROR`).
/// Транспортные сбои и некорректные ответы — `NETWORK_ERROR`.
class RpcClient {
  RpcClient({
    required this.baseUrl,
    http.Client? httpClient,
    Duration? requestTimeout,
    Logger? logger,
  })  : _http = httpClient ?? http.Client(),
        _log = logger ?? Logger('RpcClient'),
        _timeout = requestTimeout;

  final String baseUrl;
  final http.Client _http;
  final Logger _log;
  final Uuid _uuid = const Uuid();
  final Duration? _timeout;

  /// Актуальный JWT, выставляется `AuthService` после логина/восстановления
  /// (None → запрос без `Authorization`).
  String? token;

  static const Duration _defaultTimeout = Duration(seconds: 30);

  /// Отправляет Command и возвращает `payload` ответа.
  Future<Map<String, dynamic>> command({
    required String module,
    required String action,
    Map<String, dynamic> payload = const {},
  }) async {
    final id = _uuid.v4();
    final msg = RpcMessage.command(
      id: id,
      module: module,
      action: action,
      payload: payload,
    );
    final response = await _roundTrip(msg);
    return response.when(
      response: (id, payload) => payload,
      error: (id, code, message, details) =>
          throw _serverError(code, message, details),
      command: (_, _, _, _) => throw _invalid(id),
      query: (_, _, _, _) => throw _invalid(id),
      eventBatch: (_, _, _) => throw _invalid(id),
      serverPush: (_, _, _, _) => throw _invalid(id),
    );
  }

  /// Отправляет Query и возвращает `payload` ответа.
  Future<Map<String, dynamic>> query({
    required String module,
    required String action,
    Map<String, dynamic> payload = const {},
  }) async {
    final id = _uuid.v4();
    final msg = RpcMessage.query(
      id: id,
      module: module,
      action: action,
      payload: payload,
    );
    final response = await _roundTrip(msg);
    return response.when(
      response: (id, payload) => payload,
      error: (id, code, message, details) =>
          throw _serverError(code, message, details),
      command: (_, _, _, _) => throw _invalid(id),
      query: (_, _, _, _) => throw _invalid(id),
      eventBatch: (_, _, _) => throw _invalid(id),
      serverPush: (_, _, _, _) => throw _invalid(id),
    );
  }

  Never _invalid(String id) =>
      throw ServerError(ErrorCode.unknown, 'Сервер вернул неожиданный ответ на запрос $id');

  ServerError _serverError(
    String code,
    String message,
    Map<String, dynamic>? details,
  ) =>
      ServerError(ErrorCode.fromWire(code), message, details: details);

  /// Полный цикл запроса-ответа с авторизацией и таймаутом.
  Future<RpcMessage> _roundTrip(RpcMessage request) async {
    final headers = <String, String>{'Content-Type': 'application/json'};
    final token = this.token;
    if (token != null && token.isNotEmpty) {
      headers['Authorization'] = 'Bearer $token';
    }

    final uri = Uri.parse('$baseUrl/rpc');
    final encoded = jsonEncode(request.toJson());

    final http.Response httpResponse;
    try {
      _log.fine('→ $uri ${request.toJson()}');
      httpResponse = await _http
          .post(uri, headers: headers, body: encoded)
          .timeout(_timeout ?? _defaultTimeout);
    } on TimeoutException {
      throw ServerError(
        ErrorCode.network,
        'Сервер $baseUrl не ответил в течение ${(_timeout ?? _defaultTimeout).inSeconds} с',
      );
    } on Exception catch (e) {
      throw ServerError(
        ErrorCode.network,
        'Не удалось подключиться к серверу $baseUrl: $e',
      );
    }

    Object? decoded;
    try {
      decoded = jsonDecode(httpResponse.body);
    } on FormatException {
      throw ServerError(
        ErrorCode.network,
        'Не удалось распознать ответ сервера (HTTP ${httpResponse.statusCode})',
      );
    }
    if (decoded is! Map<String, dynamic>) {
      throw const ServerError(
        ErrorCode.network,
        'Некорректный формат ответа сервера',
      );
    }

    final msg = RpcMessage.fromJson(decoded);
    _log.fine('← $msg');
    return msg;
  }

  void dispose() => _http.close();
}