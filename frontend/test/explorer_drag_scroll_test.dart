import 'package:atrium/components/adaptive_drag.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:scrollable_positioned_list/scrollable_positioned_list.dart';

Widget buildListWithDrag({required ScrollOffsetController scrollController}) {
  return MaterialApp(
    home: Scaffold(
      body: Column(
        children: [
          const SizedBox(
            height: 56,
            child: Align(
              alignment: Alignment.centerLeft,
              child: ListTile(leading: Icon(Icons.reply), title: Text("..")),
            ),
          ),
          Expanded(
            child: DragTarget<List<String>>(
              onWillAcceptWithDetails: (d) => true,
              onAcceptWithDetails: (d) {},
              builder: (context, candidateData, acceptedData) => ColoredBox(
                // The highlight must not toggle the widget type around the
                // list (e.g. Container(color: null <-> color) inserts a
                // ColoredBox) or the list state, with its scroll position,
                // would be destroyed when a drag starts.
                color: candidateData.isNotEmpty
                    ? Colors.grey.shade200
                    : Colors.transparent,
                child: ScrollablePositionedList.builder(
                  itemCount: 30,
                  scrollOffsetController: scrollController,
                  itemBuilder: (context, index) {
                    final item = ListTile(
                      leading: const Icon(Icons.file_present_rounded),
                      title: Text('file $index'),
                      subtitle: Text('2024-01-01 - 1 KB'),
                    );
                    return AdaptiveDraggable<List<String>>(
                      data: ['/file $index'],
                      feedback: Material(
                        color: Colors.white,
                        child: Container(
                          padding: const EdgeInsets.all(8),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              const Icon(Icons.file_present_rounded, size: 20),
                              const SizedBox(width: 8),
                              Text('file $index'),
                            ],
                          ),
                        ),
                      ),
                      child: item,
                    );
                  },
                ),
              ),
            ),
          ),
          const SizedBox(
            height: 40,
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text('30 items'),
            ),
          ),
        ],
      ),
    ),
  );
}

double scrollPixels(WidgetTester tester) =>
    tester.state<ScrollableState>(find.byType(Scrollable)).position.pixels;

void main() {
  group('dragAutoScrollOffset', () {
    const rect = Rect.fromLTWH(0, 100, 800, 600);

    test('no scroll in the middle', () {
      expect(dragAutoScrollOffset(const Offset(400, 400), rect), null);
      expect(dragAutoScrollOffset(const Offset(400, 180), rect), null);
      expect(dragAutoScrollOffset(const Offset(400, 620), rect), null);
    });

    test('scrolls up near the top edge, faster the closer it gets', () {
      expect(dragAutoScrollOffset(const Offset(400, 164), rect), null);
      expect(
        dragAutoScrollOffset(const Offset(400, 160), rect),
        closeTo(-0.25, 0.001),
      );
      expect(
        dragAutoScrollOffset(const Offset(400, 132), rect),
        closeTo(-2.0, 0.001),
      );
      expect(dragAutoScrollOffset(const Offset(400, 100), rect), -4.0);
      // Above the list: clamped to max speed.
      expect(dragAutoScrollOffset(const Offset(400, 10), rect), -4.0);
    });

    test('scrolls down near the bottom edge', () {
      expect(dragAutoScrollOffset(const Offset(400, 636), rect), null);
      expect(
        dragAutoScrollOffset(const Offset(400, 640), rect),
        closeTo(0.25, 0.001),
      );
      expect(
        dragAutoScrollOffset(const Offset(400, 668), rect),
        closeTo(2.0, 0.001),
      );
      expect(dragAutoScrollOffset(const Offset(400, 700), rect), 4.0);
      // Below the list: clamped to max speed.
      expect(dragAutoScrollOffset(const Offset(400, 900), rect), 4.0);
    });
  });

  testWidgets('starting a drag over the list keeps the scroll position', (
    tester,
  ) async {
    final scrollController = ScrollOffsetController();
    await tester.pumpWidget(
      buildListWithDrag(scrollController: scrollController),
    );
    tester.state<ScrollableState>(find.byType(Scrollable)).position.jumpTo(400);
    await tester.pumpAndSettle();
    expect(scrollPixels(tester), 400.0);

    final gesture = await tester.startGesture(
      tester.getCenter(find.text('file 12')),
      kind: PointerDeviceKind.touch,
    );
    // Long press: the drag starts over the list, showing the drop highlight.
    await tester.pump(kLongPressTimeout + const Duration(milliseconds: 100));
    await tester.pump();

    // The list must not have been reset to the top.
    expect(scrollPixels(tester), 400.0);

    // Drop the file: the drop target accepts it.
    await gesture.up();
    await tester.pumpAndSettle();
  });

  testWidgets('scrolls smoothly while the drag is held near an edge', (
    tester,
  ) async {
    final scrollController = ScrollOffsetController();
    await tester.pumpWidget(
      buildListWithDrag(scrollController: scrollController),
    );
    tester.state<ScrollableState>(find.byType(Scrollable)).position.jumpTo(200);
    await tester.pumpAndSettle();

    final start = tester.getCenter(find.text('file 6'));
    final gesture = await tester.startGesture(
      start,
      kind: PointerDeviceKind.touch,
    );
    await tester.pump(kLongPressTimeout + const Duration(milliseconds: 100));
    // Drag the pointer near the top edge of the list.
    const pointerDelta = Offset(0, -250);
    await gesture.moveBy(pointerDelta);
    await tester.pump();

    // Emulate the explorer's auto-scroll ticker: one frame of scrolling
    // toward the top edge.
    final listBox = tester.renderObject<RenderBox>(
      find
          .ancestor(
            of: find.byType(ScrollablePositionedList),
            matching: find.byType(ColoredBox),
          )
          .first,
    );
    final rect = listBox.localToGlobal(Offset.zero) & listBox.size;
    final offset = dragAutoScrollOffset(start + pointerDelta, rect);
    expect(offset, isNotNull);
    expect(offset, lessThan(0));
    final animation = scrollController.animateScroll(
      offset: offset!,
      duration: const Duration(milliseconds: 16),
    );
    await tester.pump(const Duration(milliseconds: 16));
    await tester.pump(const Duration(milliseconds: 16));
    expect(scrollPixels(tester), lessThan(200.0));
    // Let the activity reach the idle state before awaiting it.
    await tester.pump(const Duration(milliseconds: 16));
    await animation;

    await gesture.up();
    await tester.pumpAndSettle();
  });
}
