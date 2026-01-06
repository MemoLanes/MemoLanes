import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:provider/provider.dart';

// 假设 MultiProvider 中已经提供了 ValueNotifier<bool> screenshotMode
class ScreenshotButton extends StatelessWidget {

  const ScreenshotButton({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final screenshot = context.watch<ValueNotifier<bool>>();

    // 截图模式开启时隐藏按钮
    if (screenshot.value) return const SizedBox.shrink();

    return SafeArea(
      child: Align(
        alignment: Alignment.topRight,
        child: Container(
          margin: const EdgeInsets.only(top: 8, bottom: 8),
          width: 48,
          height: 48,
          decoration: const BoxDecoration(
            color: Colors.black,
            shape: BoxShape.circle,
          ),
          child: IconButton(
            onPressed: () {
              screenshot.value = true; // 点击开启截图模式
            },
            icon: const Icon(
              Icons.camera_alt,
              color: StyleConstants.defaultColor,
            ),
            tooltip: 'Enable screenshot mode',
          ),
        ),
      ),
    );
  }
}
