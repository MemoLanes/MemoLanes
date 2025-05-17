import 'package:flutter/material.dart';

class EdgePulseGlowEffect extends StatefulWidget {
  final double glowIntensity;
  final Duration duration;
  final Color glowColor;

  const EdgePulseGlowEffect({
    Key? key,
    this.glowIntensity = 30.0,
    this.duration = const Duration(seconds: 5),
    this.glowColor = const Color(0xFFB4EC51),
  }) : super(key: key);

  @override
  State<EdgePulseGlowEffect> createState() => _EdgePulseGlowEffectState();
}

class _EdgePulseGlowEffectState extends State<EdgePulseGlowEffect>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _glow;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: widget.duration,
    )..repeat(reverse: true); // 在动画结束后反转

    _glow = Tween<double>(begin: 4.0, end: widget.glowIntensity).animate(
      CurvedAnimation(parent: _controller, curve: Curves.easeInOut),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Widget _buildEdgeGlow({required Alignment alignment}) {
    return RepaintBoundary(
      child: Align(
        alignment: alignment,
        child: AnimatedBuilder(
          animation: _controller,
          builder: (context, child) {
            final color = widget.glowColor;
            return Container(
              width: alignment == Alignment.topCenter || alignment == Alignment.bottomCenter
                  ? double.infinity
                  : _glow.value,
              height: alignment == Alignment.topCenter || alignment == Alignment.bottomCenter
                  ? _glow.value
                  : double.infinity,
              decoration: BoxDecoration(
                boxShadow: [
                  BoxShadow(
                    color: color.withOpacity(0.4),
                    // blurRadius: _glow.value * 1.5,
                    // spreadRadius: _glow.value * 0.6,
                  )
                ],
              ),
            );
          },
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: Positioned.fill(
        child: Stack(
          children: [
            _buildEdgeGlow(alignment: Alignment.topCenter),
            _buildEdgeGlow(alignment: Alignment.bottomCenter),
            _buildEdgeGlow(alignment: Alignment.centerLeft),
            _buildEdgeGlow(alignment: Alignment.centerRight),
          ],
        ),
      ),
    );
  }
}
