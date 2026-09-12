import 'package:freezed_annotation/freezed_annotation.dart';

part 'event.freezed.dart';
part 'event.g.dart';

/// Тип потока (stream), к которому принадлежит событие
/// (`crates/core-domain/src/event.rs`).
enum StreamType {
  object('object'),
  user('user'),
  person('person'),
  userContact('user_contact'),
  userProfile('user_profile'),
  userCert('user_cert'),
  company('company'),
  role('role'),
  module('module'),
  metadata('metadata');

  const StreamType(this.wireValue);

  final String wireValue;

  static StreamType fromWire(String value) {
    for (final st in StreamType.values) {
      if (st.wireValue == value) {
        return st;
      }
    }
    return StreamType.object;
  }
}

/// Исполнитель, создавший событие; читаемый снимок для аудита
/// (`ActorSnapshot`). `user_id`/`company_id` — None для системного исполнителя.
@freezed
class ActorSnapshot with _$ActorSnapshot {
  const factory ActorSnapshot({
    @JsonKey(name: 'user_id') String? userId,
    required String login,
    @JsonKey(name: 'full_name') required String fullName,
    String? position,
    @JsonKey(name: 'company_id') String? companyId,
    @JsonKey(name: 'ip_address') String? ipAddress,
  }) = _ActorSnapshot;

  factory ActorSnapshot.fromJson(Map<String, dynamic> json) =>
      _$ActorSnapshotFromJson(json);
}

/// Факт, записанный в хранилище событий (коллекция `events`).
/// Используется клиентом в `event_batch` (фаза 12) и при чтении истории.
@freezed
class KcEvent with _$KcEvent {
  const factory KcEvent({
    required String id,
    @JsonKey(name: 'stream_type') required StreamType streamType,
    @JsonKey(name: 'stream_id') required String streamId,
    @JsonKey(name: 'event_type') required String eventType,
    required int version,
    required Map<String, dynamic> payload,
    required ActorSnapshot metadata,
    @JsonKey(name: 'company_id') required String companyId,
    @JsonKey(name: 'correlation_id') required String correlationId,
    @JsonKey(name: 'causation_id') String? causationId,
    @JsonKey(name: 'occurred_at') required DateTime occurredAt,
  }) = _KcEvent;

  factory KcEvent.fromJson(Map<String, dynamic> json) =>
      _$KcEventFromJson(json);
}