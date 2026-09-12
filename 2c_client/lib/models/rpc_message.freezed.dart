// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'rpc_message.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

RpcMessage _$RpcMessageFromJson(Map<String, dynamic> json) {
  switch (json['type']) {
    case 'command':
      return RpcMessageCommand.fromJson(json);
    case 'query':
      return RpcMessageQuery.fromJson(json);
    case 'event_batch':
      return RpcMessageEventBatch.fromJson(json);
    case 'response':
      return RpcMessageResponse.fromJson(json);
    case 'error':
      return RpcMessageError.fromJson(json);
    case 'server_push':
      return RpcMessageServerPush.fromJson(json);

    default:
      throw CheckedFromJsonException(
        json,
        'type',
        'RpcMessage',
        'Invalid union type "${json['type']}"!',
      );
  }
}

/// @nodoc
mixin _$RpcMessage {
  String get id => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;

  /// Serializes this RpcMessage to a JSON map.
  Map<String, dynamic> toJson() => throw _privateConstructorUsedError;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  $RpcMessageCopyWith<RpcMessage> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $RpcMessageCopyWith<$Res> {
  factory $RpcMessageCopyWith(
    RpcMessage value,
    $Res Function(RpcMessage) then,
  ) = _$RpcMessageCopyWithImpl<$Res, RpcMessage>;
  @useResult
  $Res call({String id});
}

/// @nodoc
class _$RpcMessageCopyWithImpl<$Res, $Val extends RpcMessage>
    implements $RpcMessageCopyWith<$Res> {
  _$RpcMessageCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? id = null}) {
    return _then(
      _value.copyWith(
            id: null == id
                ? _value.id
                : id // ignore: cast_nullable_to_non_nullable
                      as String,
          )
          as $Val,
    );
  }
}

/// @nodoc
abstract class _$$RpcMessageCommandImplCopyWith<$Res>
    implements $RpcMessageCopyWith<$Res> {
  factory _$$RpcMessageCommandImplCopyWith(
    _$RpcMessageCommandImpl value,
    $Res Function(_$RpcMessageCommandImpl) then,
  ) = __$$RpcMessageCommandImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({
    String id,
    String module,
    String action,
    Map<String, dynamic> payload,
  });
}

/// @nodoc
class __$$RpcMessageCommandImplCopyWithImpl<$Res>
    extends _$RpcMessageCopyWithImpl<$Res, _$RpcMessageCommandImpl>
    implements _$$RpcMessageCommandImplCopyWith<$Res> {
  __$$RpcMessageCommandImplCopyWithImpl(
    _$RpcMessageCommandImpl _value,
    $Res Function(_$RpcMessageCommandImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? module = null,
    Object? action = null,
    Object? payload = null,
  }) {
    return _then(
      _$RpcMessageCommandImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        module: null == module
            ? _value.module
            : module // ignore: cast_nullable_to_non_nullable
                  as String,
        action: null == action
            ? _value.action
            : action // ignore: cast_nullable_to_non_nullable
                  as String,
        payload: null == payload
            ? _value._payload
            : payload // ignore: cast_nullable_to_non_nullable
                  as Map<String, dynamic>,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$RpcMessageCommandImpl implements RpcMessageCommand {
  const _$RpcMessageCommandImpl({
    required this.id,
    required this.module,
    required this.action,
    required final Map<String, dynamic> payload,
    final String? $type,
  }) : _payload = payload,
       $type = $type ?? 'command';

  factory _$RpcMessageCommandImpl.fromJson(Map<String, dynamic> json) =>
      _$$RpcMessageCommandImplFromJson(json);

  @override
  final String id;
  @override
  final String module;
  @override
  final String action;
  final Map<String, dynamic> _payload;
  @override
  Map<String, dynamic> get payload {
    if (_payload is EqualUnmodifiableMapView) return _payload;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_payload);
  }

  @JsonKey(name: 'type')
  final String $type;

  @override
  String toString() {
    return 'RpcMessage.command(id: $id, module: $module, action: $action, payload: $payload)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$RpcMessageCommandImpl &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.module, module) || other.module == module) &&
            (identical(other.action, action) || other.action == action) &&
            const DeepCollectionEquality().equals(other._payload, _payload));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    module,
    action,
    const DeepCollectionEquality().hash(_payload),
  );

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$RpcMessageCommandImplCopyWith<_$RpcMessageCommandImpl> get copyWith =>
      __$$RpcMessageCommandImplCopyWithImpl<_$RpcMessageCommandImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) {
    return command(id, module, action, payload);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) {
    return command?.call(id, module, action, payload);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) {
    if (command != null) {
      return command(id, module, action, payload);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) {
    return command(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) {
    return command?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) {
    if (command != null) {
      return command(this);
    }
    return orElse();
  }

  @override
  Map<String, dynamic> toJson() {
    return _$$RpcMessageCommandImplToJson(this);
  }
}

abstract class RpcMessageCommand implements RpcMessage {
  const factory RpcMessageCommand({
    required final String id,
    required final String module,
    required final String action,
    required final Map<String, dynamic> payload,
  }) = _$RpcMessageCommandImpl;

  factory RpcMessageCommand.fromJson(Map<String, dynamic> json) =
      _$RpcMessageCommandImpl.fromJson;

  @override
  String get id;
  String get module;
  String get action;
  Map<String, dynamic> get payload;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$RpcMessageCommandImplCopyWith<_$RpcMessageCommandImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$RpcMessageQueryImplCopyWith<$Res>
    implements $RpcMessageCopyWith<$Res> {
  factory _$$RpcMessageQueryImplCopyWith(
    _$RpcMessageQueryImpl value,
    $Res Function(_$RpcMessageQueryImpl) then,
  ) = __$$RpcMessageQueryImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({
    String id,
    String module,
    String action,
    Map<String, dynamic> payload,
  });
}

/// @nodoc
class __$$RpcMessageQueryImplCopyWithImpl<$Res>
    extends _$RpcMessageCopyWithImpl<$Res, _$RpcMessageQueryImpl>
    implements _$$RpcMessageQueryImplCopyWith<$Res> {
  __$$RpcMessageQueryImplCopyWithImpl(
    _$RpcMessageQueryImpl _value,
    $Res Function(_$RpcMessageQueryImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? module = null,
    Object? action = null,
    Object? payload = null,
  }) {
    return _then(
      _$RpcMessageQueryImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        module: null == module
            ? _value.module
            : module // ignore: cast_nullable_to_non_nullable
                  as String,
        action: null == action
            ? _value.action
            : action // ignore: cast_nullable_to_non_nullable
                  as String,
        payload: null == payload
            ? _value._payload
            : payload // ignore: cast_nullable_to_non_nullable
                  as Map<String, dynamic>,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$RpcMessageQueryImpl implements RpcMessageQuery {
  const _$RpcMessageQueryImpl({
    required this.id,
    required this.module,
    required this.action,
    required final Map<String, dynamic> payload,
    final String? $type,
  }) : _payload = payload,
       $type = $type ?? 'query';

  factory _$RpcMessageQueryImpl.fromJson(Map<String, dynamic> json) =>
      _$$RpcMessageQueryImplFromJson(json);

  @override
  final String id;
  @override
  final String module;
  @override
  final String action;
  final Map<String, dynamic> _payload;
  @override
  Map<String, dynamic> get payload {
    if (_payload is EqualUnmodifiableMapView) return _payload;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_payload);
  }

  @JsonKey(name: 'type')
  final String $type;

  @override
  String toString() {
    return 'RpcMessage.query(id: $id, module: $module, action: $action, payload: $payload)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$RpcMessageQueryImpl &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.module, module) || other.module == module) &&
            (identical(other.action, action) || other.action == action) &&
            const DeepCollectionEquality().equals(other._payload, _payload));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    module,
    action,
    const DeepCollectionEquality().hash(_payload),
  );

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$RpcMessageQueryImplCopyWith<_$RpcMessageQueryImpl> get copyWith =>
      __$$RpcMessageQueryImplCopyWithImpl<_$RpcMessageQueryImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) {
    return query(id, module, action, payload);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) {
    return query?.call(id, module, action, payload);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) {
    if (query != null) {
      return query(id, module, action, payload);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) {
    return query(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) {
    return query?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) {
    if (query != null) {
      return query(this);
    }
    return orElse();
  }

  @override
  Map<String, dynamic> toJson() {
    return _$$RpcMessageQueryImplToJson(this);
  }
}

abstract class RpcMessageQuery implements RpcMessage {
  const factory RpcMessageQuery({
    required final String id,
    required final String module,
    required final String action,
    required final Map<String, dynamic> payload,
  }) = _$RpcMessageQueryImpl;

  factory RpcMessageQuery.fromJson(Map<String, dynamic> json) =
      _$RpcMessageQueryImpl.fromJson;

  @override
  String get id;
  String get module;
  String get action;
  Map<String, dynamic> get payload;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$RpcMessageQueryImplCopyWith<_$RpcMessageQueryImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$RpcMessageEventBatchImplCopyWith<$Res>
    implements $RpcMessageCopyWith<$Res> {
  factory _$$RpcMessageEventBatchImplCopyWith(
    _$RpcMessageEventBatchImpl value,
    $Res Function(_$RpcMessageEventBatchImpl) then,
  ) = __$$RpcMessageEventBatchImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String id, String module, List<Map<String, dynamic>> events});
}

/// @nodoc
class __$$RpcMessageEventBatchImplCopyWithImpl<$Res>
    extends _$RpcMessageCopyWithImpl<$Res, _$RpcMessageEventBatchImpl>
    implements _$$RpcMessageEventBatchImplCopyWith<$Res> {
  __$$RpcMessageEventBatchImplCopyWithImpl(
    _$RpcMessageEventBatchImpl _value,
    $Res Function(_$RpcMessageEventBatchImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? id = null, Object? module = null, Object? events = null}) {
    return _then(
      _$RpcMessageEventBatchImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        module: null == module
            ? _value.module
            : module // ignore: cast_nullable_to_non_nullable
                  as String,
        events: null == events
            ? _value._events
            : events // ignore: cast_nullable_to_non_nullable
                  as List<Map<String, dynamic>>,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$RpcMessageEventBatchImpl implements RpcMessageEventBatch {
  const _$RpcMessageEventBatchImpl({
    required this.id,
    required this.module,
    required final List<Map<String, dynamic>> events,
    final String? $type,
  }) : _events = events,
       $type = $type ?? 'event_batch';

  factory _$RpcMessageEventBatchImpl.fromJson(Map<String, dynamic> json) =>
      _$$RpcMessageEventBatchImplFromJson(json);

  @override
  final String id;
  @override
  final String module;
  final List<Map<String, dynamic>> _events;
  @override
  List<Map<String, dynamic>> get events {
    if (_events is EqualUnmodifiableListView) return _events;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_events);
  }

  @JsonKey(name: 'type')
  final String $type;

  @override
  String toString() {
    return 'RpcMessage.eventBatch(id: $id, module: $module, events: $events)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$RpcMessageEventBatchImpl &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.module, module) || other.module == module) &&
            const DeepCollectionEquality().equals(other._events, _events));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    module,
    const DeepCollectionEquality().hash(_events),
  );

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$RpcMessageEventBatchImplCopyWith<_$RpcMessageEventBatchImpl>
  get copyWith =>
      __$$RpcMessageEventBatchImplCopyWithImpl<_$RpcMessageEventBatchImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) {
    return eventBatch(id, module, events);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) {
    return eventBatch?.call(id, module, events);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) {
    if (eventBatch != null) {
      return eventBatch(id, module, events);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) {
    return eventBatch(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) {
    return eventBatch?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) {
    if (eventBatch != null) {
      return eventBatch(this);
    }
    return orElse();
  }

  @override
  Map<String, dynamic> toJson() {
    return _$$RpcMessageEventBatchImplToJson(this);
  }
}

abstract class RpcMessageEventBatch implements RpcMessage {
  const factory RpcMessageEventBatch({
    required final String id,
    required final String module,
    required final List<Map<String, dynamic>> events,
  }) = _$RpcMessageEventBatchImpl;

  factory RpcMessageEventBatch.fromJson(Map<String, dynamic> json) =
      _$RpcMessageEventBatchImpl.fromJson;

  @override
  String get id;
  String get module;
  List<Map<String, dynamic>> get events;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$RpcMessageEventBatchImplCopyWith<_$RpcMessageEventBatchImpl>
  get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$RpcMessageResponseImplCopyWith<$Res>
    implements $RpcMessageCopyWith<$Res> {
  factory _$$RpcMessageResponseImplCopyWith(
    _$RpcMessageResponseImpl value,
    $Res Function(_$RpcMessageResponseImpl) then,
  ) = __$$RpcMessageResponseImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({String id, Map<String, dynamic> payload});
}

/// @nodoc
class __$$RpcMessageResponseImplCopyWithImpl<$Res>
    extends _$RpcMessageCopyWithImpl<$Res, _$RpcMessageResponseImpl>
    implements _$$RpcMessageResponseImplCopyWith<$Res> {
  __$$RpcMessageResponseImplCopyWithImpl(
    _$RpcMessageResponseImpl _value,
    $Res Function(_$RpcMessageResponseImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? id = null, Object? payload = null}) {
    return _then(
      _$RpcMessageResponseImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        payload: null == payload
            ? _value._payload
            : payload // ignore: cast_nullable_to_non_nullable
                  as Map<String, dynamic>,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$RpcMessageResponseImpl implements RpcMessageResponse {
  const _$RpcMessageResponseImpl({
    required this.id,
    required final Map<String, dynamic> payload,
    final String? $type,
  }) : _payload = payload,
       $type = $type ?? 'response';

  factory _$RpcMessageResponseImpl.fromJson(Map<String, dynamic> json) =>
      _$$RpcMessageResponseImplFromJson(json);

  @override
  final String id;
  final Map<String, dynamic> _payload;
  @override
  Map<String, dynamic> get payload {
    if (_payload is EqualUnmodifiableMapView) return _payload;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_payload);
  }

  @JsonKey(name: 'type')
  final String $type;

  @override
  String toString() {
    return 'RpcMessage.response(id: $id, payload: $payload)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$RpcMessageResponseImpl &&
            (identical(other.id, id) || other.id == id) &&
            const DeepCollectionEquality().equals(other._payload, _payload));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    const DeepCollectionEquality().hash(_payload),
  );

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$RpcMessageResponseImplCopyWith<_$RpcMessageResponseImpl> get copyWith =>
      __$$RpcMessageResponseImplCopyWithImpl<_$RpcMessageResponseImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) {
    return response(id, payload);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) {
    return response?.call(id, payload);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) {
    if (response != null) {
      return response(id, payload);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) {
    return response(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) {
    return response?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) {
    if (response != null) {
      return response(this);
    }
    return orElse();
  }

  @override
  Map<String, dynamic> toJson() {
    return _$$RpcMessageResponseImplToJson(this);
  }
}

abstract class RpcMessageResponse implements RpcMessage {
  const factory RpcMessageResponse({
    required final String id,
    required final Map<String, dynamic> payload,
  }) = _$RpcMessageResponseImpl;

  factory RpcMessageResponse.fromJson(Map<String, dynamic> json) =
      _$RpcMessageResponseImpl.fromJson;

  @override
  String get id;
  Map<String, dynamic> get payload;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$RpcMessageResponseImplCopyWith<_$RpcMessageResponseImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$RpcMessageErrorImplCopyWith<$Res>
    implements $RpcMessageCopyWith<$Res> {
  factory _$$RpcMessageErrorImplCopyWith(
    _$RpcMessageErrorImpl value,
    $Res Function(_$RpcMessageErrorImpl) then,
  ) = __$$RpcMessageErrorImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({
    String id,
    String code,
    String message,
    Map<String, dynamic>? details,
  });
}

/// @nodoc
class __$$RpcMessageErrorImplCopyWithImpl<$Res>
    extends _$RpcMessageCopyWithImpl<$Res, _$RpcMessageErrorImpl>
    implements _$$RpcMessageErrorImplCopyWith<$Res> {
  __$$RpcMessageErrorImplCopyWithImpl(
    _$RpcMessageErrorImpl _value,
    $Res Function(_$RpcMessageErrorImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? code = null,
    Object? message = null,
    Object? details = freezed,
  }) {
    return _then(
      _$RpcMessageErrorImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        code: null == code
            ? _value.code
            : code // ignore: cast_nullable_to_non_nullable
                  as String,
        message: null == message
            ? _value.message
            : message // ignore: cast_nullable_to_non_nullable
                  as String,
        details: freezed == details
            ? _value._details
            : details // ignore: cast_nullable_to_non_nullable
                  as Map<String, dynamic>?,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$RpcMessageErrorImpl implements RpcMessageError {
  const _$RpcMessageErrorImpl({
    required this.id,
    required this.code,
    required this.message,
    final Map<String, dynamic>? details,
    final String? $type,
  }) : _details = details,
       $type = $type ?? 'error';

  factory _$RpcMessageErrorImpl.fromJson(Map<String, dynamic> json) =>
      _$$RpcMessageErrorImplFromJson(json);

  @override
  final String id;
  @override
  final String code;
  @override
  final String message;
  final Map<String, dynamic>? _details;
  @override
  Map<String, dynamic>? get details {
    final value = _details;
    if (value == null) return null;
    if (_details is EqualUnmodifiableMapView) return _details;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(value);
  }

  @JsonKey(name: 'type')
  final String $type;

  @override
  String toString() {
    return 'RpcMessage.error(id: $id, code: $code, message: $message, details: $details)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$RpcMessageErrorImpl &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.code, code) || other.code == code) &&
            (identical(other.message, message) || other.message == message) &&
            const DeepCollectionEquality().equals(other._details, _details));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    code,
    message,
    const DeepCollectionEquality().hash(_details),
  );

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$RpcMessageErrorImplCopyWith<_$RpcMessageErrorImpl> get copyWith =>
      __$$RpcMessageErrorImplCopyWithImpl<_$RpcMessageErrorImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) {
    return error(id, code, message, details);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) {
    return error?.call(id, code, message, details);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(id, code, message, details);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) {
    return error(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) {
    return error?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(this);
    }
    return orElse();
  }

  @override
  Map<String, dynamic> toJson() {
    return _$$RpcMessageErrorImplToJson(this);
  }
}

abstract class RpcMessageError implements RpcMessage {
  const factory RpcMessageError({
    required final String id,
    required final String code,
    required final String message,
    final Map<String, dynamic>? details,
  }) = _$RpcMessageErrorImpl;

  factory RpcMessageError.fromJson(Map<String, dynamic> json) =
      _$RpcMessageErrorImpl.fromJson;

  @override
  String get id;
  String get code;
  String get message;
  Map<String, dynamic>? get details;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$RpcMessageErrorImplCopyWith<_$RpcMessageErrorImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$RpcMessageServerPushImplCopyWith<$Res>
    implements $RpcMessageCopyWith<$Res> {
  factory _$$RpcMessageServerPushImplCopyWith(
    _$RpcMessageServerPushImpl value,
    $Res Function(_$RpcMessageServerPushImpl) then,
  ) = __$$RpcMessageServerPushImplCopyWithImpl<$Res>;
  @override
  @useResult
  $Res call({
    String id,
    String module,
    @JsonKey(name: 'event_type') String eventType,
    Map<String, dynamic> payload,
  });
}

/// @nodoc
class __$$RpcMessageServerPushImplCopyWithImpl<$Res>
    extends _$RpcMessageCopyWithImpl<$Res, _$RpcMessageServerPushImpl>
    implements _$$RpcMessageServerPushImplCopyWith<$Res> {
  __$$RpcMessageServerPushImplCopyWithImpl(
    _$RpcMessageServerPushImpl _value,
    $Res Function(_$RpcMessageServerPushImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? id = null,
    Object? module = null,
    Object? eventType = null,
    Object? payload = null,
  }) {
    return _then(
      _$RpcMessageServerPushImpl(
        id: null == id
            ? _value.id
            : id // ignore: cast_nullable_to_non_nullable
                  as String,
        module: null == module
            ? _value.module
            : module // ignore: cast_nullable_to_non_nullable
                  as String,
        eventType: null == eventType
            ? _value.eventType
            : eventType // ignore: cast_nullable_to_non_nullable
                  as String,
        payload: null == payload
            ? _value._payload
            : payload // ignore: cast_nullable_to_non_nullable
                  as Map<String, dynamic>,
      ),
    );
  }
}

/// @nodoc
@JsonSerializable()
class _$RpcMessageServerPushImpl implements RpcMessageServerPush {
  const _$RpcMessageServerPushImpl({
    required this.id,
    required this.module,
    @JsonKey(name: 'event_type') required this.eventType,
    required final Map<String, dynamic> payload,
    final String? $type,
  }) : _payload = payload,
       $type = $type ?? 'server_push';

  factory _$RpcMessageServerPushImpl.fromJson(Map<String, dynamic> json) =>
      _$$RpcMessageServerPushImplFromJson(json);

  @override
  final String id;
  @override
  final String module;
  @override
  @JsonKey(name: 'event_type')
  final String eventType;
  final Map<String, dynamic> _payload;
  @override
  Map<String, dynamic> get payload {
    if (_payload is EqualUnmodifiableMapView) return _payload;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableMapView(_payload);
  }

  @JsonKey(name: 'type')
  final String $type;

  @override
  String toString() {
    return 'RpcMessage.serverPush(id: $id, module: $module, eventType: $eventType, payload: $payload)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$RpcMessageServerPushImpl &&
            (identical(other.id, id) || other.id == id) &&
            (identical(other.module, module) || other.module == module) &&
            (identical(other.eventType, eventType) ||
                other.eventType == eventType) &&
            const DeepCollectionEquality().equals(other._payload, _payload));
  }

  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  int get hashCode => Object.hash(
    runtimeType,
    id,
    module,
    eventType,
    const DeepCollectionEquality().hash(_payload),
  );

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$RpcMessageServerPushImplCopyWith<_$RpcMessageServerPushImpl>
  get copyWith =>
      __$$RpcMessageServerPushImplCopyWithImpl<_$RpcMessageServerPushImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    command,
    required TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )
    query,
    required TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )
    eventBatch,
    required TResult Function(String id, Map<String, dynamic> payload) response,
    required TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )
    error,
    required TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )
    serverPush,
  }) {
    return serverPush(id, module, eventType, payload);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult? Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult? Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult? Function(String id, Map<String, dynamic> payload)? response,
    TResult? Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult? Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
  }) {
    return serverPush?.call(id, module, eventType, payload);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    command,
    TResult Function(
      String id,
      String module,
      String action,
      Map<String, dynamic> payload,
    )?
    query,
    TResult Function(
      String id,
      String module,
      List<Map<String, dynamic>> events,
    )?
    eventBatch,
    TResult Function(String id, Map<String, dynamic> payload)? response,
    TResult Function(
      String id,
      String code,
      String message,
      Map<String, dynamic>? details,
    )?
    error,
    TResult Function(
      String id,
      String module,
      @JsonKey(name: 'event_type') String eventType,
      Map<String, dynamic> payload,
    )?
    serverPush,
    required TResult orElse(),
  }) {
    if (serverPush != null) {
      return serverPush(id, module, eventType, payload);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RpcMessageCommand value) command,
    required TResult Function(RpcMessageQuery value) query,
    required TResult Function(RpcMessageEventBatch value) eventBatch,
    required TResult Function(RpcMessageResponse value) response,
    required TResult Function(RpcMessageError value) error,
    required TResult Function(RpcMessageServerPush value) serverPush,
  }) {
    return serverPush(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RpcMessageCommand value)? command,
    TResult? Function(RpcMessageQuery value)? query,
    TResult? Function(RpcMessageEventBatch value)? eventBatch,
    TResult? Function(RpcMessageResponse value)? response,
    TResult? Function(RpcMessageError value)? error,
    TResult? Function(RpcMessageServerPush value)? serverPush,
  }) {
    return serverPush?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RpcMessageCommand value)? command,
    TResult Function(RpcMessageQuery value)? query,
    TResult Function(RpcMessageEventBatch value)? eventBatch,
    TResult Function(RpcMessageResponse value)? response,
    TResult Function(RpcMessageError value)? error,
    TResult Function(RpcMessageServerPush value)? serverPush,
    required TResult orElse(),
  }) {
    if (serverPush != null) {
      return serverPush(this);
    }
    return orElse();
  }

  @override
  Map<String, dynamic> toJson() {
    return _$$RpcMessageServerPushImplToJson(this);
  }
}

abstract class RpcMessageServerPush implements RpcMessage {
  const factory RpcMessageServerPush({
    required final String id,
    required final String module,
    @JsonKey(name: 'event_type') required final String eventType,
    required final Map<String, dynamic> payload,
  }) = _$RpcMessageServerPushImpl;

  factory RpcMessageServerPush.fromJson(Map<String, dynamic> json) =
      _$RpcMessageServerPushImpl.fromJson;

  @override
  String get id;
  String get module;
  @JsonKey(name: 'event_type')
  String get eventType;
  Map<String, dynamic> get payload;

  /// Create a copy of RpcMessage
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$RpcMessageServerPushImplCopyWith<_$RpcMessageServerPushImpl>
  get copyWith => throw _privateConstructorUsedError;
}
