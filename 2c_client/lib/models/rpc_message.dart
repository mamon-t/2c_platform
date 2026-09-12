import 'package:freezed_annotation/freezed_annotation.dart';

part 'rpc_message.freezed.dart';
part 'rpc_message.g.dart';

@Freezed(unionKey: 'type')
sealed class RpcMessage with _$RpcMessage {
  const factory RpcMessage.command({
    required String id,
    required String module,
    required String action,
    required Map<String, dynamic> payload,
  }) = RpcMessageCommand;

  const factory RpcMessage.query({
    required String id,
    required String module,
    required String action,
    required Map<String, dynamic> payload,
  }) = RpcMessageQuery;

  @FreezedUnionValue('event_batch')
  const factory RpcMessage.eventBatch({
    required String id,
    required String module,
    required List<Map<String, dynamic>> events,
  }) = RpcMessageEventBatch;

  const factory RpcMessage.response({
    required String id,
    required Map<String, dynamic> payload,
  }) = RpcMessageResponse;

  const factory RpcMessage.error({
    required String id,
    required String code,
    required String message,
    Map<String, dynamic>? details,
  }) = RpcMessageError;

  @FreezedUnionValue('server_push')
  const factory RpcMessage.serverPush({
    required String id,
    required String module,
    @JsonKey(name: 'event_type') required String eventType,
    required Map<String, dynamic> payload,
  }) = RpcMessageServerPush;

  factory RpcMessage.fromJson(Map<String, dynamic> json) =>
      _$RpcMessageFromJson(json);
}
