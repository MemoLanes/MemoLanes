import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    show DuoHardware, DuoHingeStatus, DuoRegion, DuoSizeClass;

export 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    show
        DuoBridge,
        DuoHardware,
        DuoHardwareScope,
        DuoHingeStatus,
        DuoRegion,
        DuoSizeClass;

/// Copies immutable hardware fixtures without adding a production data model.
extension DuoHardwareTestCopy on DuoHardware {
  DuoHardware copyWith({
    bool? supported,
    bool? hasHinge,
    DuoHingeStatus? status,
    double? angle,
    List<DuoRegion>? folds,
    List<DuoRegion>? cameras,
    DuoSizeClass? horizontalSizeClass,
    DuoSizeClass? verticalSizeClass,
  }) => DuoHardware(
    supported: supported ?? this.supported,
    hasHinge: hasHinge ?? this.hasHinge,
    status: status ?? this.status,
    angle: angle ?? this.angle,
    folds: folds ?? this.folds,
    cameras: cameras ?? this.cameras,
    horizontalSizeClass: horizontalSizeClass ?? this.horizontalSizeClass,
    verticalSizeClass: verticalSizeClass ?? this.verticalSizeClass,
    barEdge: barEdge,
  );
}
