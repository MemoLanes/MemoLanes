import 'package:flutter/material.dart';
import 'package:memolanes/common/loading_manager.dart';

/// Unified navigation helpers.
///
/// All helpers here wrap the destination page with [GlobalPopScope] so that
/// back button / back gesture is blocked while global loading is active.
Future<T?> pushNoPop<T>(
  BuildContext context, {
  required Widget page,
  RouteSettings? settings,
  bool fullscreenDialog = false,
  bool maintainState = true,
  bool rootNavigator = false,
}) {
  return Navigator.of(context, rootNavigator: rootNavigator).push<T>(
    MaterialPageRoute<T>(
      builder: (_) => GlobalPopScope(child: page),
      settings: settings,
      fullscreenDialog: fullscreenDialog,
      maintainState: maintainState,
    ),
  );
}

Future<T?> pushReplacementPage<T, TO>(
  BuildContext context, {
  required Widget page,
  RouteSettings? settings,
  bool fullscreenDialog = false,
  bool maintainState = true,
  bool rootNavigator = false,
  TO? result,
}) {
  return Navigator.of(context, rootNavigator: rootNavigator)
      .pushReplacement<T, TO>(
    MaterialPageRoute<T>(
      builder: (_) => GlobalPopScope(child: page),
      settings: settings,
      fullscreenDialog: fullscreenDialog,
      maintainState: maintainState,
    ),
    result: result,
  );
}

Future<T?> pushAndRemoveUntilPage<T>(
  BuildContext context, {
  required Widget page,
  required RoutePredicate predicate,
  RouteSettings? settings,
  bool fullscreenDialog = false,
  bool maintainState = true,
  bool rootNavigator = false,
}) {
  return Navigator.of(context, rootNavigator: rootNavigator)
      .pushAndRemoveUntil<T>(
    MaterialPageRoute<T>(
      builder: (_) => GlobalPopScope(child: page),
      settings: settings,
      fullscreenDialog: fullscreenDialog,
      maintainState: maintainState,
    ),
    predicate,
  );
}
