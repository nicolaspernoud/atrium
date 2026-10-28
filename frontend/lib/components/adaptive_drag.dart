import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Grey highlight for drop targets, readable in both light and dark modes.
Color dropHighlightColor(BuildContext context, {bool strong = false}) {
  final bool dark = Theme.of(context).brightness == Brightness.dark;
  return dark
      ? Colors.grey[strong ? 700 : 800]!
      : Colors.grey[strong ? 300 : 200]!;
}

/// The scroll offset, in pixels per frame (positive to scroll down), that a
/// list should apply for a smooth edge auto-scroll while [location] is
/// dragged near the top or bottom edge of its [rect] viewport.
///
/// Returns `null` when [location] is not close enough to any edge. The speed
/// ramps from zero at [edge] distance to [maxSpeed] at (or beyond) the edge.
double? dragAutoScrollOffset(
  Offset location,
  Rect rect, {
  double edge = 64,
  double maxSpeed = 4,
}) {
  final double topDist = location.dy - rect.top;
  final double bottomDist = rect.bottom - location.dy;
  if (topDist < edge) {
    return -maxSpeed * math.min(1.0, math.max(0.0, 1.0 - topDist / edge));
  }
  if (bottomDist < edge) {
    return maxSpeed * math.min(1.0, math.max(0.0, 1.0 - bottomDist / edge));
  }
  return null;
}

/// A [LongPressDraggable] that starts the drag immediately on press and
/// move for precise pointers (mouse), like a common file manager, while
/// still requiring a long press for touch pointers so that the list can
/// be scrolled.
class AdaptiveDraggable<T extends Object> extends LongPressDraggable<T> {
  const AdaptiveDraggable({
    super.key,
    required super.child,
    required super.data,
    required super.feedback,
    super.onDragStarted,
    super.onDragUpdate,
    super.onDragEnd,
    super.onDraggableCanceled,
    super.hitTestBehavior,
    super.rootOverlay,
  });

  @override
  DelayedMultiDragGestureRecognizer createRecognizer(
      GestureMultiDragStartCallback onStart) {
    return _AdaptiveDragGestureRecognizer(delay: delay)
      ..onStart = (Offset position) {
        final Drag? result = onStart(position);
        if (result != null && hapticFeedbackOnStart) {
          HapticFeedback.selectionClick();
        }
        return result;
      };
  }
}

class _AdaptiveDragGestureRecognizer extends DelayedMultiDragGestureRecognizer {
  _AdaptiveDragGestureRecognizer({super.delay});

  @override
  MultiDragPointerState createNewPointerState(PointerDownEvent event) {
    if (event.kind == PointerDeviceKind.mouse) {
      return _ImmediateMultiDragPointerState(
        event.position,
        event.kind,
        gestureSettings,
      );
    }
    return _LongPressMultiDragPointerState(
      event.position,
      delay,
      event.kind,
      gestureSettings,
    );
  }
}

/// Starts the drag as soon as the pointer moves past the hit slop, so a
/// mouse press followed by any movement drags right away.
class _ImmediateMultiDragPointerState extends MultiDragPointerState {
  _ImmediateMultiDragPointerState(
    super.initialPosition,
    super.kind,
    super.gestureSettings,
  );

  @override
  void checkForResolutionAfterMove() {
    if (pendingDelta!.distance > computeHitSlop(kind, gestureSettings)) {
      resolve(GestureDisposition.accepted);
    }
  }

  @override
  void accepted(GestureMultiDragStartCallback starter) {
    starter(initialPosition);
  }
}

/// Requires the pointer to be held still for the delay before the drag
/// starts, so that touch drags keep working as scroll gestures.
class _LongPressMultiDragPointerState extends MultiDragPointerState {
  _LongPressMultiDragPointerState(
    super.initialPosition,
    this.delay,
    super.kind,
    super.gestureSettings,
  ) {
    _timer = Timer(delay, _delayPassed);
  }

  final Duration delay;
  Timer? _timer;
  GestureMultiDragStartCallback? _starter;

  void _delayPassed() {
    _timer = null;
    if (_starter != null) {
      _starter!(initialPosition);
      _starter = null;
    } else {
      resolve(GestureDisposition.accepted);
    }
  }

  void _ensureTimerStopped() {
    _timer?.cancel();
    _timer = null;
  }

  @override
  void accepted(GestureMultiDragStartCallback starter) {
    if (_timer == null) {
      starter(initialPosition);
    } else {
      _starter = starter;
    }
  }

  @override
  void checkForResolutionAfterMove() {
    if (_timer == null) {
      // Accepted by the arena but the drag has not started yet
      return;
    }
    if (pendingDelta!.distance > computeHitSlop(kind, gestureSettings)) {
      resolve(GestureDisposition.rejected);
      _ensureTimerStopped();
    }
  }

  @override
  void rejected() {
    _ensureTimerStopped();
    super.rejected();
  }

  @override
  void dispose() {
    _ensureTimerStopped();
    super.dispose();
  }
}
