import 'package:flutter_test/flutter_test.dart';
import 'package:json_annotation/json_annotation.dart';
import 'package:twoc_client/models/rpc_message.dart';

void main() {
  group('RpcMessage round-trip', () {
    test('command', () {
      const original = RpcMessageCommand(
        id: '1',
        module: 'core',
        action: 'company.create',
        payload: {'code': 'x', 'name': 'X'},
      );
      final json = original.toJson();
      expect(json['type'], 'command');
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageCommand>());
      final c = decoded as RpcMessageCommand;
      expect(c.id, '1');
      expect(c.module, 'core');
      expect(c.action, 'company.create');
      expect(c.payload['code'], 'x');
    });

    test('query', () {
      const original = RpcMessageQuery(
        id: '2',
        module: 'core',
        action: 'company.list',
        payload: {},
      );
      final json = original.toJson();
      expect(json['type'], 'query');
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageQuery>());
      expect((decoded as RpcMessageQuery).action, 'company.list');
    });

    test('event_batch', () {
      const original = RpcMessageEventBatch(
        id: '3',
        module: 'core',
        events: [
          {'id': 'e1'},
        ],
      );
      final json = original.toJson();
      expect(json['type'], 'event_batch');
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageEventBatch>());
      final e = decoded as RpcMessageEventBatch;
      expect(e.module, 'core');
      expect(e.events, hasLength(1));
      expect(e.events.first['id'], 'e1');
    });

    test('response', () {
      const original = RpcMessageResponse(id: '4', payload: {'ok': true});
      final json = original.toJson();
      expect(json['type'], 'response');
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageResponse>());
      expect(((decoded as RpcMessageResponse).payload as Map)['ok'], true);
    });

    test('response с массивным payload (list-команды)', () {
      const original = RpcMessageResponse(
        id: '4b',
        payload: <Object?>[
          {'id': 'o1'},
        ],
      );
      final json = original.toJson();
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageResponse>());
      final payload = (decoded as RpcMessageResponse).payload as List;
      expect((payload.first as Map)['id'], 'o1');
    });

    test('error', () {
      const original = RpcMessageError(
        id: '5',
        code: 'VALIDATION_ERROR',
        message: 'bad field',
        details: {'field': 'code'},
      );
      final json = original.toJson();
      expect(json['type'], 'error');
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageError>());
      final e = decoded as RpcMessageError;
      expect(e.code, 'VALIDATION_ERROR');
      expect(e.message, 'bad field');
      expect(e.details, {'field': 'code'});
    });

    test('server_push', () {
      const original = RpcMessageServerPush(
        id: '6',
        module: 'core',
        eventType: 'object.created',
        payload: {'sid': 'o:1'},
      );
      final json = original.toJson();
      expect(json['type'], 'server_push');
      expect(json['event_type'], 'object.created');
      final decoded = RpcMessage.fromJson(json);
      expect(decoded, isA<RpcMessageServerPush>());
      final p = decoded as RpcMessageServerPush;
      expect(p.eventType, 'object.created');
      expect(p.payload['sid'], 'o:1');
    });

    test('unknown type throws CheckedFromJsonException', () {
      expect(
        () => RpcMessage.fromJson(
          const {'type': 'bogus', 'id': '7'},
        ),
        throwsA(isA<CheckedFromJsonException>()),
      );
    });
  });
}