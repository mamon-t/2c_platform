import 'dart:async';
import 'dart:convert';

import 'package:logging/logging.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

import '../models/rpc_message.dart';

/// Состояние WebSocket-соединения.
enum WsConnectionState {
  disconnected,
  connecting,
  connected,
}

/// Клиент `GET /ws?token=<JWT>` (ТЗ §10c): получает `ServerPush`,
/// автоматически переподключается с экспоненциальным backoff (1 с → 30 с).
///
/// Отдельный инстанс на сессию: `connect` создаёт канал и фоновые таски,
/// `dispose` закрывает всё при логауте.
class WsClient {
  WsClient({required this.wsBaseUrl, Logger? logger})
      : _log = logger ?? Logger('WsClient');

  final String wsBaseUrl;
  final Logger _log;

  static const _maxBackoff = Duration(seconds: 30);
  static const _baseBackoff = Duration(seconds: 1);

  final StreamController<RpcMessage> _messages =
      StreamController<RpcMessage>.broadcast();
  final StreamController<WsConnectionState> _state =
      StreamController<WsConnectionState>.broadcast();

  WebSocketChannel? _channel;
  StreamSubscription<String>? _subscription;
  Timer? _reconnectTimer;
  String? _token;
  int _reconnectAttempt = 0;
  bool _stoppedByUser = false;
  bool _disposed = false;

  /// Входящие `ServerPush` (прочие сообщения игнорируются).
  Stream<RpcMessage> get messages => _messages.stream;

  /// Текущее состояние соединения (для индикатора в UI).
  Stream<WsConnectionState> get state => _state.stream;

  WsConnectionState get currentState => _state.hasListener
      ? _lastState
      : WsConnectionState.disconnected;
  WsConnectionState _lastState = WsConnectionState.disconnected;

  /// Устанавливает соединение от имени авторизованного пользователя
  /// и запускает авто-переподключение при обрывах.
  void connect(String token) {
    if (_disposed) return;
    _stoppedByUser = false;
    _token = token;
    _reconnectAttempt = 0;
    _open();
  }

  /// Сознательно останавливает переподключение и закрывает канал.
  void disconnect() {
    if (_disposed) return;
    _stoppedByUser = true;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _teardown();
    _emitState(WsConnectionState.disconnected);
  }

  /// Закрывает клиент полностью. После вызова объект непригоден.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _stoppedByUser = true;
    _reconnectTimer?.cancel();
    _teardown();
    await _messages.close();
    await _state.close();
  }

  void _open() {
    _emitState(WsConnectionState.connecting);
    final uri =
        Uri.parse('$wsBaseUrl/ws').replace(queryParameters: {'token': _token});
    _log.fine('WS connect $uri');
    try {
      _channel = WebSocketChannel.connect(uri);
      _subscription = _channel!.stream
          .where((data) => data is String)
          .cast<String>()
          .listen(_onData, onError: _onError, onDone: _onDone);
      _emitState(WsConnectionState.connected);
    } on Exception catch (e) {
      _log.warning('WS connect failed: $e');
      _teardown();
      _scheduleReconnect();
    }
  }

  void _onData(String data) {
    final Object? decoded;
    try {
      decoded = jsonDecode(data);
    } on FormatException {
      _log.warning('WS нераспознаваемое сообщение: $data');
      return;
    }
    if (decoded is! Map<String, dynamic>) return;
    final msg = RpcMessage.fromJson(decoded);
    if (msg is RpcMessageServerPush) {
      _log.fine('WS push ${msg.eventType}');
      _messages.add(msg);
    }
  }

  void _onError(Object e) {
    _log.warning('WS error: $e');
  }

  void _onDone() {
    _log.fine('WS канал закрыт');
    _teardown();
    if (_stoppedByUser || _disposed) {
      _emitState(WsConnectionState.disconnected);
    } else {
      _scheduleReconnect();
    }
  }

  void _scheduleReconnect() {
    if (_stoppedByUser || _disposed || _reconnectTimer != null) return;
    final attempt = _reconnectAttempt;
    final exponent = 1 << attempt;
    final raw = _baseBackoff * exponent;
    final delay = raw > _maxBackoff ? _maxBackoff : raw;
    _log.info('WS переподключение через ${delay.inSeconds} с');
    _emitState(WsConnectionState.disconnected);
    _reconnectTimer = Timer(delay, () {
      _reconnectTimer = null;
      _reconnectAttempt++;
      if (!_stoppedByUser && !_disposed) {
        _open();
      }
    });
  }

  void _teardown() {
    _subscription?.cancel();
    _subscription = null;
    _channel?.sink.close();
    _channel = null;
  }

  void _emitState(WsConnectionState next) {
    _lastState = next;
    _state.add(next);
  }
}