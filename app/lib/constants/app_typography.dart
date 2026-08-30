import 'package:flutter/material.dart';

/// Semantic typography used throughout MemoLanes.
///
/// The scale intentionally uses a small number of roles so Chinese and English
/// copy keep the same hierarchy. Generic bilingual styles avoid letter spacing;
/// tracking is reserved for short, Latin-only status badges.
abstract final class AppTypography {
  static const TextStyle pageTitle = TextStyle(
    fontSize: 28,
    fontWeight: FontWeight.w800,
    height: 1.1,
  );

  static const TextStyle compactPageTitle = TextStyle(
    fontSize: 24,
    fontWeight: FontWeight.w800,
    height: 1.1,
  );

  static const TextStyle metricTitle = TextStyle(
    fontSize: 22,
    fontWeight: FontWeight.w800,
    height: 1.08,
  );

  static const TextStyle dataValue = TextStyle(
    fontSize: 32,
    fontWeight: FontWeight.w500,
    height: 1.1,
  );

  static const TextStyle pickerValue = TextStyle(
    fontSize: 20,
    fontWeight: FontWeight.w600,
    height: 1.2,
  );

  static const TextStyle surfaceTitle = TextStyle(
    fontSize: 17,
    fontWeight: FontWeight.w700,
    height: 1.2,
  );

  static const TextStyle subpageTitle = TextStyle(
    fontSize: 16,
    fontWeight: FontWeight.w600,
    height: 1.25,
  );

  static const TextStyle cardTitle = TextStyle(
    fontSize: 15,
    fontWeight: FontWeight.w600,
    height: 1.3,
  );

  static const TextStyle itemTitle = TextStyle(
    fontSize: 14,
    fontWeight: FontWeight.w600,
    height: 1.35,
  );

  static const TextStyle bodyLarge = TextStyle(
    fontSize: 16,
    fontWeight: FontWeight.w400,
    height: 1.4,
  );

  static const TextStyle body = TextStyle(
    fontSize: 14,
    fontWeight: FontWeight.w400,
    height: 1.42,
  );

  static const TextStyle supporting = TextStyle(
    fontSize: 13,
    fontWeight: FontWeight.w400,
    height: 1.4,
  );

  static const TextStyle caption = TextStyle(
    fontSize: 12,
    fontWeight: FontWeight.w500,
    height: 1.3,
  );

  static const TextStyle label = TextStyle(
    fontSize: 12,
    fontWeight: FontWeight.w600,
    height: 1.2,
  );

  static const TextStyle micro = TextStyle(
    fontSize: 11,
    fontWeight: FontWeight.w600,
    height: 1.2,
  );

  static const TextStyle sectionLabel = TextStyle(
    fontSize: 13,
    fontWeight: FontWeight.w600,
    height: 1.2,
  );

  static const TextStyle button = TextStyle(
    fontSize: 14,
    fontWeight: FontWeight.w700,
    height: 1.15,
  );

  static const TextStyle compactButton = TextStyle(
    fontSize: 12,
    fontWeight: FontWeight.w700,
    height: 1.15,
  );

  static const TextStyle largeButton = TextStyle(
    fontSize: 16,
    fontWeight: FontWeight.w700,
    height: 1.15,
  );

  static const TextStyle badge = TextStyle(
    fontSize: 11,
    fontWeight: FontWeight.w700,
    height: 1.1,
    letterSpacing: 0.2,
  );

  static const TextTheme textTheme = TextTheme(
    displaySmall: pageTitle,
    headlineSmall: metricTitle,
    titleLarge: surfaceTitle,
    titleMedium: cardTitle,
    titleSmall: itemTitle,
    bodyLarge: bodyLarge,
    bodyMedium: body,
    bodySmall: supporting,
    labelLarge: button,
    labelMedium: label,
    labelSmall: micro,
  );
}
