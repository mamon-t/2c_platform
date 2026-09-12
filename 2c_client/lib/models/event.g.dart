// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'event.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

_$ActorSnapshotImpl _$$ActorSnapshotImplFromJson(Map<String, dynamic> json) =>
    _$ActorSnapshotImpl(
      userId: json['user_id'] as String?,
      login: json['login'] as String,
      fullName: json['full_name'] as String,
      position: json['position'] as String?,
      companyId: json['company_id'] as String?,
      ipAddress: json['ip_address'] as String?,
    );

Map<String, dynamic> _$$ActorSnapshotImplToJson(_$ActorSnapshotImpl instance) =>
    <String, dynamic>{
      'user_id': instance.userId,
      'login': instance.login,
      'full_name': instance.fullName,
      'position': instance.position,
      'company_id': instance.companyId,
      'ip_address': instance.ipAddress,
    };

_$KcEventImpl _$$KcEventImplFromJson(Map<String, dynamic> json) =>
    _$KcEventImpl(
      id: json['id'] as String,
      streamType: $enumDecode(_$StreamTypeEnumMap, json['stream_type']),
      streamId: json['stream_id'] as String,
      eventType: json['event_type'] as String,
      version: (json['version'] as num).toInt(),
      payload: json['payload'] as Map<String, dynamic>,
      metadata: ActorSnapshot.fromJson(
        json['metadata'] as Map<String, dynamic>,
      ),
      companyId: json['company_id'] as String,
      correlationId: json['correlation_id'] as String,
      causationId: json['causation_id'] as String?,
      occurredAt: DateTime.parse(json['occurred_at'] as String),
    );

Map<String, dynamic> _$$KcEventImplToJson(_$KcEventImpl instance) =>
    <String, dynamic>{
      'id': instance.id,
      'stream_type': _$StreamTypeEnumMap[instance.streamType]!,
      'stream_id': instance.streamId,
      'event_type': instance.eventType,
      'version': instance.version,
      'payload': instance.payload,
      'metadata': instance.metadata,
      'company_id': instance.companyId,
      'correlation_id': instance.correlationId,
      'causation_id': instance.causationId,
      'occurred_at': instance.occurredAt.toIso8601String(),
    };

const _$StreamTypeEnumMap = {
  StreamType.object: 'object',
  StreamType.user: 'user',
  StreamType.person: 'person',
  StreamType.userContact: 'userContact',
  StreamType.userProfile: 'userProfile',
  StreamType.userCert: 'userCert',
  StreamType.company: 'company',
  StreamType.role: 'role',
  StreamType.module: 'module',
  StreamType.metadata: 'metadata',
};
