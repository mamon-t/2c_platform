// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'event.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

ActorSnapshot _$ActorSnapshotFromJson(Map<String, dynamic> json) {
  return _ActorSnapshot.fromJson(json);
}

/// @nodoc
mixin _$ActorSnapshot {
  @JsonKey(name: 'user_id')
  String? get userId => throw _privateConstructorUsedError;
  String get login => throw _privateConstructorUsedError;
  @JsonKey(name: 'full_name')
  String get fullName => throw _privateConstructorUsedError;
  String? get position => throw _privateConstructorUsedError;
  @JsonKey(name: 'company_id')
  String? get companyId => throw _privateConstructorUsedError;
  @JsonKey(name: 'ip_address')
  String? get ipAddress => throw _privateConstructorUsedError;

  /// Serializes this ActorSnapshot to a JSON map.
  Map<String, dynamic> toJson() => throw _privateConstructorUsedError;

  /// Create a copy of ActorSnapshot
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  $ActorSnapshotCopyWith<ActorSnapshot> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $ActorSnapshotCopyWith<$Res> {
  factory $ActorSnapshotCopyWith(
    ActorSnapshot value,
    $Res Function(ActorSnapshot) then,
  ) = _$ActorSnapshotCopyWithImpl<$Res, ActorSnapshot>;
  @useResult
  $Res call({
    @JsonKey(name: 'user_id') String? userId,
    String login,
    @JsonKey(name: 'full_name') String fullName,
    String? position,
    @JsonKey(name: 'company_id') String? companyId,
    @JsonKey(name: 'ip_address') String? ipAddress,
  });
}

/// @nodoc
class _$ActorSnapshotCopyWithImpl<$Res, $Val extends ActorSnapshot>
    implements $ActorSnapshotCopyWith<$Res> {
  _$ActorSnapshotCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of ActorSnapshot
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? userId = freezed,
    Object? login = null,
    Object? fullName = null,
    Object? position = freezed,
    Object? companyId = freezed,
    Object? ipAddress = freezed,
  }) {
    return _then(
      _value.copyWith(
            userId: freezed == userId
                ? _value.userId
                : userId // ignore: cast_nullable_to_non_nullable
                      as String?,
            login: null == login
                ? _value.login
                : login // ignore: cast_nullable_to_non_nullable
                      as String,
            fullName: null == fullName
                ? _value.fullName
                : fullName // ignore: cast_nullable_to_non_nullable
                      as String,
            position: freezed == position
                ? _value.position
                : position // ignore: cast_nullable_to_non_nullable
                      as String?,
            companyId: freezed == companyId
                ? _value.companyId
                : companyId // ignore: cast_nullable_to_non_nullable
                      as String?,
            ipAddress: freezed == ipAddress
                ? _value.ipAddress
                : ipAddress // ignore: cast_nullable_to_non_nullable
                      as String?,
          )
          as $Val,
    );
  }
}

/// @nodoc
abstract class _$$ActorSnapshotImplCopyWith<$Res>
    implements $ActorSnapshotCopyWith<$Res> {
  factory _$$ActorSnapshotImplCopyWith(
    _$ActorSnapshotImpl value,
    $Res Function(_$ActorSnapshotImpl) then,
  ) = __$$ActorSnapshotImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({
    @JsonKey(name: 'user_id') String? userId,
    String login,
    @JsonKey(name: 'full_name') String fullName,
    String? position,
    @JsonKey(name: 'company_id') String? companyId,
    @JsonKey(name: 'ip_address') String? ipAddress,
  });
}

/// @nodoc
class __$$ActorSnapshotImplCopyWithImpl<$Res>
    extends _$ActorSnapshotCopyWithImpl<$Res, _$ActorSnapshotImpl>
    implements _$$ActorSnapshotImplCopyWith<$Res> {
  __$$ActorSnapshotImplCopyWithImpl(
    _$ActorSnapshotImpl _value,
    $Res Function(_$ActorSnapshotImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of ActorSnapshot
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? userId = freezed,
    Object? login = null,
    Object? fullName = null,
    Object? position = freezed,
    Object? companyId = freezed,
    Object? ipAddress = freezed,
  }) {
    return _then(
      _$ActorSnapshotImpl(
        userId: freezed == userId
            ? _value.userId
            : userId // ignore: cast_nullable_to_non_nullable
                  as String?,
        login: null == login
            ? _value.login
            : login // ignore: cast_nullable_to_non_nullable
                  as String,
        fullName: null == fullName
            ? _value.fullName
            : fullName // ignore: cast_nullable_to_non_nullable
                  as String,
        position: freezed == position
            ? _value.position
            : position // ignore: cast_nullable_to_non_nullable
                  as String?,
        companyId: freezed == companyId
            ? _value.companyId
            : companyId // ignore: cast_nullable_to_non_nullable
                  as String?,
        ipAddress: freezed == ipAddress
            ? _value.ipAddress
            : ipAddress // ignore: cast_nullable_to_non_nullable
                  as String?,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$ActorSnapshotImpl implements _ActorSnapshot {
  const _$ActorSnapshotImpl({
    @JsonKey(name: 'user_id') this.userId,
    required this.login,
    @JsonKey(name: 'full_name') required this.fullName,
    this.position,
    @JsonKey(name: 'company_id') this.companyId,
    @JsonKey(name: 'ip_address') this.ipAddress,
  });

  factory _$ActorSnapshotImpl.fromJson(Map<String, dynamic> json) =>
      _$$ActorSnapshotImplFromJson(json);

  @override
  @JsonKey(name: 'user_id')
  final String? userId;
  @override
  final String login;
  @override
  @JsonKey(name: 'full_name')
  final String fullName;
  @override
  final String? position;
  @override
  @JsonKey(name: 'company_id')
  final String? companyId;
  @override
  @JsonKey(name: 'ip_address')
  final String? ipAddress;

  @override
  String toString() {
    return 'ActorSnapshot(userId: $userId, login: $login, fullName: $fullName, position: $position, companyId: $companyId, ipAddress: $ipAddress)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$ActorSnapshotImpl &&
            (identical(other.userId, userId) || other.userId == userId) &&
            (identical(other.login, login) || other.login == login) &&
            (identical(other.fullName, fullName) ||
                other.fullName == fullName) &&
            (identical(other.position, position) ||
                other.position == position) &&
            (identical(other.companyId, companyId) ||
                other.companyId == companyId) &&
            (identical(other.ipAddress, ipAddress) ||
                other.ipAddress == ipAddress));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    userId,
    login,
    fullName,
    position,
    companyId,
    ipAddress,
  );

  /// Create a copy of ActorSnapshot
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$ActorSnapshotImplCopyWith<_$ActorSnapshotImpl> get copyWith =>
      __$$ActorSnapshotImplCopyWithImpl<_$ActorSnapshotImpl>(this, _$identity);

  @override
  Map<String, dynamic> toJson() {
    return _$$ActorSnapshotImplToJson(this);
  }
}

abstract class _ActorSnapshot implements ActorSnapshot {
  const factory _ActorSnapshot({
    @JsonKey(name: 'user_id') final String? userId,
    required final String login,
    @JsonKey(name: 'full_name') required final String fullName,
    final String? position,
    @JsonKey(name: 'company_id') final String? companyId,
    @JsonKey(name: 'ip_address') final String? ipAddress,
  }) = _$ActorSnapshotImpl;

  factory _ActorSnapshot.fromJson(Map<String, dynamic> json) =
      _$ActorSnapshotImpl.fromJson;

  @override
  @JsonKey(name: 'user_id')
  String? get userId;
  @override
  String get login;
  @override
  @JsonKey(name: 'full_name')
  String get fullName;
  @override
  String? get position;
  @override
  @JsonKey(name: 'company_id')
  String? get companyId;
  @override
  @JsonKey(name: 'ip_address')
  String? get ipAddress;

  /// Create a copy of ActorSnapshot
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$ActorSnapshotImplCopyWith<_$ActorSnapshotImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

KcEvent _$KcEventFromJson(Map<String, dynamic> json) {
  return _KcEvent.fromJson(json);
}

/// @nodoc
mixin _$KcEvent {
  String get id => throw _privateConstructorUsedError;
  @JsonKey(name: 'stream_type')
  StreamType get streamType => throw _privateConstructorUsedError;
  @JsonKey(name: 'stream_id')
  String get streamId => throw _privateConstructorUsedError;
  @JsonKey(name: 'event_type')
  String get eventType => throw _privateConstructorUsedError;
  int get version => throw _privateConstructorUsedError;
  Map<String, dynamic> get payload => throw _privateConstructorUsedError;
  ActorSnapshot get metadata => throw _privateConstructorUsedError;
  @JsonKey(name: 'company_id')
  String get companyId => throw _privateConstructorUsedError;
  @JsonKey(name: 'correlation_id')
  String get correlationId => throw _privateConstructorUsedError;
  @JsonKey(name: 'causation_id')
  String? get causationId => throw _privateConstructorUsedError;
  @JsonKey(name: 'occurred_at')
  DateTime get occurredAt => throw _privateConstructorUsedError;

  /// Serializes this KcEvent to a JSON map.
  Map<String, dynamic> toJson() => throw _privateConstructorUsedError;

  /// Create a copy of KcEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  $KcEventCopyWith<KcEvent> get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $KcEventCopyWith<$Res> {
  factory $KcEventCopyWith(KcEvent value, $Res Function(KcEvent) then) =
      _$KcEventCopyWithImpl<$Res, KcEvent>;
  @useResult
  $Res call({
    String id,
    @JsonKey(name: 'stream_type') StreamType streamType,
    @JsonKey(name: 'stream_id') String streamId,
    @JsonKey(name: 'event_type') String eventType,
    int version,
    Map<String, dynamic> payload,
    ActorSnapshot metadata,
    @JsonKey(name: 'company_id') String companyId,
    @JsonKey(name: 'correlation_id') String correlationId,
    @JsonKey(name: 'causation_id') String? causationId,
    @JsonKey(name: 'occurred_at') DateTime occurredAt,
  });

  $ActorSnapshotCopyWith<$Res> get metadata;
}

/// @nodoc
class _$KcEventCopyWithImpl<$Res, $Val extends KcEvent>
    implements $KcEventCopyWith<$Res> {
  _$KcEventCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of KcEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? streamType = null,
    Object? streamId = null,
    Object? eventType = null,
    Object? version = null,
    Object? payload = null,
    Object? metadata = null,
    Object? companyId = null,
    Object? correlationId = null,
    Object? causationId = freezed,
    Object? occurredAt = null,
  }) {
    return _then(
      _value.copyWith(
            id: null == id
                ? _value.id
                : id // ignore: cast_nullable_to_non_nullable
                      as String,
            streamType: null == streamType
                ? _value.streamType
                : streamType // ignore: cast_nullable_to_non_nullable
                      as StreamType,
            streamId: null == streamId
                ? _value.streamId
                : streamId // ignore: cast_nullable_to_non_nullable
                      as String,
            eventType: null == eventType
                ? _value.eventType
                : eventType // ignore: cast_nullable_to_non_nullable
                      as String,
            version: null == version
                ? _value.version
                : version // ignore: cast_nullable_to_non_nullable
                      as int,
            payload: null == payload
                ? _value.payload
                : payload // ignore: cast_nullable_to_non_nullable
                      as Map<String, dynamic>,
            metadata: null == metadata
                ? _value.metadata
                : metadata // ignore: cast_nullable_to_non_nullable
                      as ActorSnapshot,
            companyId: null == companyId
                ? _value.companyId
                : companyId // ignore: cast_nullable_to_non_nullable
                      as String,
            correlationId: null == correlationId
                ? _value.correlationId
                : correlationId // ignore: cast_nullable_to_non_nullable
                      as String,
            causationId: freezed == causationId
                ? _value.causationId
                : causationId // ignore: cast_nullable_to_non_nullable
                      as String?,
            occurredAt: null == occurredAt
                ? _value.occurredAt
                : occurredAt // ignore: cast_nullable_to_non_nullable
                      as DateTime,
          )
          as $Val,
    );
  }

  /// Create a copy of KcEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $ActorSnapshotCopyWith<$Res> get metadata {
    return $ActorSnapshotCopyWith<$Res>(_value.metadata, (value) {
      return _then(_value.copyWith(metadata: value) as $Val);
    });
  }
}

/// @nodoc
abstract class _$$KcEventImplCopyWith<$Res> implements $KcEventCopyWith<$Res> {
  factory _$$KcEventImplCopyWith(
    _$KcEventImpl value,
    $Res Function(_$KcEventImpl) then,
  ) = __$$KcEventImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({
    String id,
    @JsonKey(name: 'stream_type') StreamType streamType,
    @JsonKey(name: 'stream_id') String streamId,
    @JsonKey(name: 'event_type') String eventType,
    int version,
    Map<String, dynamic> payload,
    ActorSnapshot metadata,
    @JsonKey(name: 'company_id') String companyId,
    @JsonKey(name: 'correlation_id') String correlationId,
    @JsonKey(name: 'causation_id') String? causationId,
    @JsonKey(name: 'occurred_at') DateTime occurredAt,
  });

  @override
  $ActorSnapshotCopyWith<$Res> get metadata;
}

/// @nodoc
class __$$KcEventImplCopyWithImpl<$Res>
    extends _$KcEventCopyWithImpl<$Res, _$KcEventImpl>
    implements _$$KcEventImplCopyWith<$Res> {
  __$$KcEventImplCopyWithImpl(
    _$KcEventImpl _value,
    $Res Function(_$KcEventImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of KcEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? streamType = null,
    Object? streamId = null,
    Object? eventType = null,
    Object? version = null,
    Object? payload = null,
    Object? metadata = null,
    Object? companyId = null,
    Object? correlationId = null,
    Object? causationId = freezed,
    Object? occurredAt = null,
  }) {
    return _then(
      _$KcEventImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        streamType: null == streamType
            ? _value.streamType
            : streamType // ignore: cast_nullable_to_non_nullable
                  as StreamType,
        streamId: null == streamId
            ? _value.streamId
            : streamId // ignore: cast_nullable_to_non_nullable
                  as String,
        eventType: null == eventType
            ? _value.eventType
            : eventType // ignore: cast_nullable_to_non_nullable
                  as String,
        version: null == version
            ? _value.version
            : version // ignore: cast_nullable_to_non_nullable
                  as int,
        payload: null == payload
            ? _value._payload
            : payload // ignore: cast_nullable_to_non_nullable
                  as Map<String, dynamic>,
        metadata: null == metadata
            ? _value.metadata
            : metadata // ignore: cast_nullable_to_non_nullable
                  as ActorSnapshot,
        companyId: null == companyId
            ? _value.companyId
            : companyId // ignore: cast_nullable_to_non_nullable
                  as String,
        correlationId: null == correlationId
            ? _value.correlationId
            : correlationId // ignore: cast_nullable_to_non_nullable
                  as String,
        causationId: freezed == causationId
            ? _value.causationId
            : causationId // ignore: cast_nullable_to_non_nullable
                  as String?,
        occurredAt: null == occurredAt
            ? _value.occurredAt
            : occurredAt // ignore: cast_nullable_to_non_nullable
                  as DateTime,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$KcEventImpl implements _KcEvent {
  const _$KcEventImpl({
    required this.id,
    @JsonKey(name: 'stream_type') required this.streamType,
    @JsonKey(name: 'stream_id') required this.streamId,
    @JsonKey(name: 'event_type') required this.eventType,
    required this.version,
    required final Map<String, dynamic> payload,
    required this.metadata,
    @JsonKey(name: 'company_id') required this.companyId,
    @JsonKey(name: 'correlation_id') required this.correlationId,
    @JsonKey(name: 'causation_id') this.causationId,
    @JsonKey(name: 'occurred_at') required this.occurredAt,
  }) : _payload = payload;

  factory _$KcEventImpl.fromJson(Map<String, dynamic> json) =>
      _$$KcEventImplFromJson(json);

  @override
  final String id;
  @override
  @JsonKey(name: 'stream_type')
  final StreamType streamType;
  @override
  @JsonKey(name: 'stream_id')
  final String streamId;
  @override
  @JsonKey(name: 'event_type')
  final String eventType;
  @override
  final int version;
  final Map<String, dynamic> _payload;
  @override
  Map<String, dynamic> get payload {
    if (_payload is EqualUnmodifiableMapView) return _payload;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_payload);
  }

  @override
  final ActorSnapshot metadata;
  @override
  @JsonKey(name: 'company_id')
  final String companyId;
  @override
  @JsonKey(name: 'correlation_id')
  final String correlationId;
  @override
  @JsonKey(name: 'causation_id')
  final String? causationId;
  @override
  @JsonKey(name: 'occurred_at')
  final DateTime occurredAt;

  @override
  String toString() {
    return 'KcEvent(id: $id, streamType: $streamType, streamId: $streamId, eventType: $eventType, version: $version, payload: $payload, metadata: $metadata, companyId: $companyId, correlationId: $correlationId, causationId: $causationId, occurredAt: $occurredAt)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$KcEventImpl &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.streamType, streamType) ||
                other.streamType == streamType) &&
            (identical(other.streamId, streamId) ||
                other.streamId == streamId) &&
            (identical(other.eventType, eventType) ||
                other.eventType == eventType) &&
            (identical(other.version, version) || other.version == version) &&
            const DeepCollectionEquality().equals(other._payload, _payload) &&
            (identical(other.metadata, metadata) ||
                other.metadata == metadata) &&
            (identical(other.companyId, companyId) ||
                other.companyId == companyId) &&
            (identical(other.correlationId, correlationId) ||
                other.correlationId == correlationId) &&
            (identical(other.causationId, causationId) ||
                other.causationId == causationId) &&
            (identical(other.occurredAt, occurredAt) ||
                other.occurredAt == occurredAt));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    streamType,
    streamId,
    eventType,
    version,
    const DeepCollectionEquality().hash(_payload),
    metadata,
    companyId,
    correlationId,
    causationId,
    occurredAt,
  );

  /// Create a copy of KcEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$KcEventImplCopyWith<_$KcEventImpl> get copyWith =>
      __$$KcEventImplCopyWithImpl<_$KcEventImpl>(this, _$identity);

  @override
  Map<String, dynamic> toJson() {
    return _$$KcEventImplToJson(this);
  }
}

abstract class _KcEvent implements KcEvent {
  const factory _KcEvent({
    required final String id,
    @JsonKey(name: 'stream_type') required final StreamType streamType,
    @JsonKey(name: 'stream_id') required final String streamId,
    @JsonKey(name: 'event_type') required final String eventType,
    required final int version,
    required final Map<String, dynamic> payload,
    required final ActorSnapshot metadata,
    @JsonKey(name: 'company_id') required final String companyId,
    @JsonKey(name: 'correlation_id') required final String correlationId,
    @JsonKey(name: 'causation_id') final String? causationId,
    @JsonKey(name: 'occurred_at') required final DateTime occurredAt,
  }) = _$KcEventImpl;

  factory _KcEvent.fromJson(Map<String, dynamic> json) = _$KcEventImpl.fromJson;

  @override
  String get id;
  @override
  @JsonKey(name: 'stream_type')
  StreamType get streamType;
  @override
  @JsonKey(name: 'stream_id')
  String get streamId;
  @override
  @JsonKey(name: 'event_type')
  String get eventType;
  @override
  int get version;
  @override
  Map<String, dynamic> get payload;
  @override
  ActorSnapshot get metadata;
  @override
  @JsonKey(name: 'company_id')
  String get companyId;
  @override
  @JsonKey(name: 'correlation_id')
  String get correlationId;
  @override
  @JsonKey(name: 'causation_id')
  String? get causationId;
  @override
  @JsonKey(name: 'occurred_at')
  DateTime get occurredAt;

  /// Create a copy of KcEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$KcEventImplCopyWith<_$KcEventImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
