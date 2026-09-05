import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/async_load_token.dart';

void main() {
  test('only the latest token is active', () {
    final guard = AsyncLoadToken();
    final first = guard.begin();
    final second = guard.begin();

    expect(guard.isActive(first), isFalse);
    expect(guard.isActive(second), isTrue);
  });

  test('invalidate and clear make existing tokens inactive', () {
    final guard = AsyncLoadToken();
    final token = guard.begin();

    guard.invalidate();
    expect(guard.isActive(token), isFalse);

    final nextToken = guard.begin();
    guard.clear();
    expect(guard.isActive(nextToken), isFalse);
  });
}
