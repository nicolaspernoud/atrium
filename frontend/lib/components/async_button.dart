import 'package:flutter/material.dart';

class AsyncButton extends StatefulWidget {
  final Function? onPressed;
  final String buttonText;

  const AsyncButton(
      {super.key, required this.onPressed, required this.buttonText});

  @override
  AsyncButtonState createState() => AsyncButtonState();
}

class AsyncButtonState extends State<AsyncButton> {
  bool isLoading = false;

  void _handleClick() async {
    setState(() {
      isLoading = true;
    });

    if (widget.onPressed != null) {
      await widget.onPressed!();
    }

    setState(() {
      isLoading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: isLoading ? null : _handleClick,
      child: AnimatedSwitcher(
        duration: const Duration(milliseconds: 200),
        transitionBuilder: (child, animation) {
          return FadeTransition(
            opacity: animation,
            child: child,
          );
        },
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: isLoading
              ? const SizedBox(
                  key: ValueKey('spinner'),
                  height: 20,
                  width: 20,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                  ),
                )
              : Text(
                  widget.buttonText,
                  key: const ValueKey('button'),
                ),
        ),
      ),
    );
  }
}
