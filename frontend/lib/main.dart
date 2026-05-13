import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';

const String socketPath = '/tmp/lumen.sock';

// Match SimState clamps in the backend. Redline is i30-petrol typical.
const double rpmMax = 7000;
const double rpmRedline = 6500;

void main() {
  runApp(const LumenApp());
}

class LumenApp extends StatelessWidget {
  const LumenApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'lumen',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark(useMaterial3: true).copyWith(
        scaffoldBackgroundColor: const Color(0xFF0A0A0A),
      ),
      home: const Hud(),
    );
  }
}

class Hud extends StatefulWidget {
  const Hud({super.key});

  @override
  State<Hud> createState() => _HudState();
}

class _HudState extends State<Hud> with SingleTickerProviderStateMixin {
  Socket? _socket;
  StreamSubscription<String>? _subscription;
  Timer? _retryTimer;
  late final AnimationController _flashController;
  double _rpm = 0;
  double _speedKmh = 0;
  String _status = 'connecting...';
  bool _connected = false;

  @override
  void initState() {
    super.initState();
    _flashController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 200),
    )..repeat();
    _connect();
  }

  @override
  void dispose() {
    _flashController.dispose();
    _retryTimer?.cancel();
    _subscription?.cancel();
    _socket?.destroy();
    super.dispose();
  }

  Future<void> _connect() async {
    try {
      final address = InternetAddress(
        socketPath,
        type: InternetAddressType.unix,
      );
      final socket = await Socket.connect(address, 0);
      if (!mounted) {
        socket.destroy();
        return;
      }
      _socket = socket;
      setState(() {
        _status = 'connected';
        _connected = true;
      });

      _subscription = socket
          .cast<List<int>>()
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(
        _handleLine,
        onDone: () => _onDisconnect('disconnected'),
        onError: (Object e) => _onDisconnect('error: $e'),
        cancelOnError: true,
      );
    } catch (e) {
      _onDisconnect('connect failed: $e');
    }
  }

  void _handleLine(String line) {
    try {
      final json = jsonDecode(line) as Map<String, dynamic>;
      final rpm = (json['rpm'] as num).toDouble();
      final speedKmh = (json['speed_kmh'] as num).toDouble();
      setState(() {
        _rpm = rpm;
        _speedKmh = speedKmh;
      });
    } catch (_) {
      // Ignore malformed lines — keep showing the last good values.
    }
  }

  void _onDisconnect(String reason) {
    _subscription?.cancel();
    _subscription = null;
    _socket?.destroy();
    _socket = null;
    if (!mounted) return;
    setState(() {
      _status = reason;
      _connected = false;
    });
    _retryTimer?.cancel();
    _retryTimer = Timer(const Duration(seconds: 1), _connect);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 48, vertical: 28),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _StatusChip(connected: _connected, label: _status),
              const Spacer(flex: 1),
              AnimatedBuilder(
                animation: _flashController,
                builder: (_, _) => RpmBar(
                  rpm: _rpm,
                  max: rpmMax,
                  redline: rpmRedline,
                  flashPhase: _flashController.value,
                ),
              ),
              const SizedBox(height: 12),
              _RpmReadout(rpm: _rpm),
              const Spacer(flex: 2),
              _SpeedReadout(speedKmh: _speedKmh),
              const Spacer(flex: 3),
            ],
          ),
        ),
      ),
    );
  }
}

class _StatusChip extends StatelessWidget {
  final bool connected;
  final String label;
  const _StatusChip({required this.connected, required this.label});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(
            color: connected ? const Color(0xFF22C55E) : const Color(0xFFEF4444),
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: 8),
        Text(
          label,
          style: const TextStyle(color: Colors.white38, fontSize: 12),
        ),
      ],
    );
  }
}

class _SpeedReadout extends StatelessWidget {
  final double speedKmh;
  const _SpeedReadout({required this.speedKmh});

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          speedKmh.toStringAsFixed(0),
          style: const TextStyle(
            fontSize: 220,
            fontWeight: FontWeight.w200,
            height: 0.9,
            letterSpacing: -6,
            fontFeatures: [FontFeature.tabularFigures()],
          ),
        ),
        const SizedBox(height: 4),
        const Text(
          'km/h',
          style: TextStyle(
            fontSize: 22,
            color: Colors.white54,
            letterSpacing: 6,
            fontWeight: FontWeight.w500,
          ),
        ),
      ],
    );
  }
}

class _RpmReadout extends StatelessWidget {
  final double rpm;
  const _RpmReadout({required this.rpm});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(
          '${rpm.toStringAsFixed(0)} rpm',
          style: const TextStyle(
            fontSize: 18,
            color: Colors.white70,
            letterSpacing: 2,
            fontFeatures: [FontFeature.tabularFigures()],
          ),
        ),
        const Text(
          'redline 6500',
          style: TextStyle(
            fontSize: 12,
            color: Colors.white24,
            letterSpacing: 2,
          ),
        ),
      ],
    );
  }
}

class RpmBar extends StatelessWidget {
  final double rpm;
  final double max;
  final double redline;
  final double flashPhase;

  const RpmBar({
    super.key,
    required this.rpm,
    required this.max,
    required this.redline,
    required this.flashPhase,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 56,
      child: CustomPaint(
        painter: _RpmBarPainter(
          rpm: rpm,
          max: max,
          redline: redline,
          flashPhase: flashPhase,
        ),
        size: Size.infinite,
      ),
    );
  }
}

class _RpmBarPainter extends CustomPainter {
  static const int segments = 40;
  static const double segmentGap = 4;
  static const Color redBright = Color(0xFFEF4444);
  static const Color redDim = Color(0xFF5C1010);

  final double rpm;
  final double max;
  final double redline;
  final double flashPhase;

  _RpmBarPainter({
    required this.rpm,
    required this.max,
    required this.redline,
    required this.flashPhase,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final segWidth = (size.width - segmentGap * (segments - 1)) / segments;
    final fraction = (rpm / max).clamp(0.0, 1.0);
    final litCount = (fraction * segments).round();

    // Color thresholds, as fraction of the bar.
    final yellowAt = ((redline - 1000) / max).clamp(0.0, 1.0);
    final redAt = (redline / max).clamp(0.0, 1.0);

    final bool overRedline = rpm >= redline;
    final bool flashBright = flashPhase < 0.5;

    const radius = Radius.circular(3);

    for (int i = 0; i < segments; i++) {
      final position = (i + 0.5) / segments;
      final isLit = i < litCount;
      final rect = Rect.fromLTWH(
        i * (segWidth + segmentGap),
        0,
        segWidth,
        size.height,
      );

      Color segmentColor;
      if (overRedline) {
        // Whole-bar shift-light flash. Every segment is red, alternating
        // bright/dim — bar position is preserved (lit vs. unlit still maps
        // to current rpm) but the colour says "you are past the line."
        segmentColor = flashBright
            ? (isLit ? redBright : redDim)
            : (isLit ? redDim : const Color(0xFF1A0A0A));
      } else {
        segmentColor = isLit
            ? _zoneColor(position, yellowAt, redAt)
            : const Color(0xFF1A1A1A);
      }

      final paint = Paint()..color = segmentColor;
      canvas.drawRRect(RRect.fromRectAndRadius(rect, radius), paint);

      // Soft inner highlight on lit segments for a touch of dimension.
      if (isLit && !overRedline) {
        final highlight = Paint()
          ..color = Colors.white.withValues(alpha: 0.10)
          ..blendMode = BlendMode.plus;
        final highlightRect = Rect.fromLTWH(
          rect.left,
          rect.top,
          rect.width,
          rect.height * 0.35,
        );
        canvas.drawRRect(
          RRect.fromRectAndCorners(
            highlightRect,
            topLeft: radius,
            topRight: radius,
          ),
          highlight,
        );
      }
    }
  }

  Color _zoneColor(double position, double yellowAt, double redAt) {
    if (position >= redAt) return redBright;
    if (position >= yellowAt) return const Color(0xFFEAB308); // yellow
    return const Color(0xFF22C55E); // green
  }

  @override
  bool shouldRepaint(_RpmBarPainter old) =>
      old.rpm != rpm ||
      old.max != max ||
      old.redline != redline ||
      // Only repaint for flash changes when we're actually over redline.
      (rpm >= redline && old.flashPhase != flashPhase);
}
