import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:twoc_client/models/rpc_message.dart';
import 'package:twoc_client/services/ws_client.dart';

Future<int> _serve({required Future<void> Function(WebSocket socket) handler}) async {
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
  // ignore: unawaited_futures
  server.listen((request) async {
    final socket = await WebSocketTransformer.upgrade(request);
    await handler(socket);
    await socket.close();
  });
  return server.port;
}

void main() {
  test('receives server_push and reports connected state', () async {
    final port = await _serve(
      handler: (socket) async {
        socket.add(jsonEncode({
          'type': 'server_push',
          'id': 'p1',
          'module': 'core',
          'event_type': 'object.created',
          'payload': {'sid': 'o:1'},
        }));
        await Future<void>.delayed(const Duration(milliseconds: 100));
      },
    );

    final client = WsClient(wsBaseUrl: 'ws://127.0.0.1:$port');
    final states = <WsConnectionState>[];
    final pushes = <RpcMessageServerPush>[];
    final stateSub = client.state.listen(states.add);
    final pushSub = client.messages.listen((m) {
      if (m is RpcMessageServerPush) {
        pushes.add(m);
      }
    });

    client.connect('test-token');
    await Future<void>.delayed(const Duration(milliseconds: 200));

    expect(states, contains(WsConnectionState.connected));
    expect(pushes, hasLength(1));
    expect(pushes.first.eventType, 'object.created');
    expect(pushes.first.payload['sid'], 'o:1');

    client.disconnect();
    await pushSub.cancel();
    await stateSub.cancel();
  });
}