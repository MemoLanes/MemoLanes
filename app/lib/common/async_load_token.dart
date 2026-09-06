/// Tracks which asynchronous load is currently allowed to publish results.
///
/// Starting or invalidating a load changes the active token. Older async
/// operations can still finish, but [isActive] lets callers ignore their
/// results safely.
class AsyncLoadToken {
  Object? _activeToken;

  Object begin() {
    final token = Object();
    _activeToken = token;
    return token;
  }

  void invalidate() {
    _activeToken = null;
  }

  bool isActive(Object token) => identical(token, _activeToken);

  void clear() {
    _activeToken = null;
  }
}
