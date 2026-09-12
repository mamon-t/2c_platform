// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'rpc_message.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

_$RpcMessageCommandImpl _$$RpcMessageCommandImplFromJson(
  Map<String, dynamic> json,
) => _$RpcMessageCommandImpl(
  id: json['id'] as String,
  module: json['module'] as String,
  action: json['action'] as String,
  payload: json['payload'] as Map<String, dynamic>,
  $type: json['type'] as String?,
);

Map<String, dynamic> _$$RpcMessageCommandImplToJson(
  _$RpcMessageCommandImpl instance,
) => <String, dynamic>{
  'id': instance.id,
  'module': instance.module,
  'action': instance.action,
  'payload': instance.payload,
  'type': instance.$type,
};

_$RpcMessageQueryImpl _$$RpcMessageQueryImplFromJson(
  Map<String, dynamic> json,
) => _$RpcMessageQueryImpl(
  id: json['id'] as String,
  module: json['module'] as String,
  action: json['action'] as String,
  payload: json['payload'] as Map<String, dynamic>,
  $type: json['type'] as String?,
);

Map<String, dynamic> _$$RpcMessageQueryImplToJson(
  _$RpcMessageQueryImpl instance,
) => <String, dynamic>{
  'id': instance.id,
  'module': instance.module,
  'action': instance.action,
  'payload': instance.payload,
  'type': instance.$type,
};

_$RpcMessageEventBatchImpl _$$RpcMessageEventBatchImplFromJson(
  Map<String, dynamic> json,
) => _$RpcMessageEventBatchImpl(
  id: json['id'] as String,
  module: json['module'] as String,
  events: (json['events'] as List<dynamic>)
      .map((e) => e as Map<String, dynamic>)
      .toList(),
  $type: json['type'] as String?,
);

Map<String, dynamic> _$$RpcMessageEventBatchImplToJson(
  _$RpcMessageEventBatchImpl instance,
) => <String, dynamic>{
  'id': instance.id,
  'module': instance.module,
  'events': instance.events,
  'type': instance.$type,
};

_$RpcMessageResponseImpl _$$RpcMessageResponseImplFromJson(
  Map<String, dynamic> json,
) => _$RpcMessageResponseImpl(
  id: json['id'] as String,
  payload: json['payload'] as Map<String, dynamic>,
  $type: json['type'] as String?,
);

Map<String, dynamic> _$$RpcMessageResponseImplToJson(
  _$RpcMessageResponseImpl instance,
) => <String, dynamic>{
  'id': instance.id,
  'payload': instance.payload,
  'type': instance.$type,
};

_$RpcMessageErrorImpl _$$RpcMessageErrorImplFromJson(
  Map<String, dynamic> json,
) => _$RpcMessageErrorImpl(
  id: json['id'] as String,
  code: json['code'] as String,
  message: json['message'] as String,
  details: json['details'] as Map<String, dynamic>?,
  $type: json['type'] as String?,
);

Map<String, dynamic> _$$RpcMessageErrorImplToJson(
  _$RpcMessageErrorImpl instance,
) => <String, dynamic>{
  'id': instance.id,
  'code': instance.code,
  'message': instance.message,
  'details': instance.details,
  'type': instance.$type,
};

_$RpcMessageServerPushImpl _$$RpcMessageServerPushImplFromJson(
  Map<String, dynamic> json,
) => _$RpcMessageServerPushImpl(
  id: json['id'] as String,
  module: json['module'] as String,
  eventType: json['event_type'] as String,
  payload: json['payload'] as Map<String, dynamic>,
  $type: json['type'] as String?,
);

Map<String, dynamic> _$$RpcMessageServerPushImplToJson(
  _$RpcMessageServerPushImpl instance,
) => <String, dynamic>{
  'id': instance.id,
  'module': instance.module,
  'event_type': instance.eventType,
  'payload': instance.payload,
  'type': instance.$type,
};
