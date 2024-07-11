// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'map.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

MapState _$MapStateFromJson(Map<String, dynamic> json) => MapState(
      $enumDecode(_$TrackingModeEnumMap, json['trackingMode']),
      (json['zoom'] as num).toDouble(),
      (json['lng'] as num).toDouble(),
      (json['lat'] as num).toDouble(),
      (json['bearing'] as num).toDouble(),
    );

Map<String, dynamic> _$MapStateToJson(MapState instance) => <String, dynamic>{
      'trackingMode': _$TrackingModeEnumMap[instance.trackingMode]!,
      'zoom': instance.zoom,
      'lng': instance.lng,
      'lat': instance.lat,
      'bearing': instance.bearing,
    };

const _$TrackingModeEnumMap = {
  TrackingMode.displayAndTracking: 'displayAndTracking',
  TrackingMode.displayOnly: 'displayOnly',
  TrackingMode.off: 'off',
};
