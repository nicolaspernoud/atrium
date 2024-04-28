import 'package:atrium/components/davs_list.dart';
import 'package:atrium/components/system_info.dart';
import 'package:atrium/components/users_list.dart';
import 'package:atrium/components/welcome_screen.dart';
import 'package:flutter/material.dart';
import 'package:atrium/i18n.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:provider/provider.dart';

import 'components/apps_list.dart';
import 'globals.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await App().init();
  runApp(
    ChangeNotifierProvider.value(
      value: App(),
      child: const MyApp(),
    ),
  );
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'atrium',
      home: const HomePage(),
      localizationsDelegates: const [
        MyLocalizationsDelegate(),
        ...GlobalMaterialLocalizations.delegates,
        GlobalWidgetsLocalizations.delegate,
      ],
      supportedLocales: const [
        Locale('en', ''),
        Locale('fr', ''),
      ],
      themeMode: ThemeMode.system,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
            seedColor: Colors.indigo,
            background: Colors.grey.shade50,
            surfaceTint: Colors.white),
        cardTheme: const CardTheme(elevation: 2),
        appBarTheme: AppBarTheme(
          backgroundColor: Colors.indigo,
          foregroundColor: Colors.grey.shade50,
          elevation: 4,
          shadowColor: Theme.of(context).shadowColor,
        ),
      ),
      darkTheme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
            seedColor: Colors.indigo, brightness: Brightness.dark),
        cardTheme: const CardTheme(elevation: 2),
        appBarTheme: const AppBarTheme(
          backgroundColor: Colors.indigo,
          elevation: 4,
        ),
      ),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  int _selectedIndex = 0;
  late PageController _pageController;

  @override
  void initState() {
    super.initState();
    _pageController = PageController();
  }

  @override
  void dispose() {
    _pageController.dispose();
    super.dispose();
  }

  void _onItemTapped(int index) {
    setState(() {
      _selectedIndex = index;
      _pageController.animateToPage(index,
          duration: const Duration(milliseconds: 250), curve: Curves.easeOut);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<App>(
      builder: (context, app, child) {
        // Reset index on logout
        if (!app.hasToken) _selectedIndex = 0;
        return Scaffold(
          bottomNavigationBar: app.hasToken
              ? BottomNavigationBar(
                  onTap: _onItemTapped,
                  currentIndex: _selectedIndex,
                  selectedItemColor: Colors.amber[900],
                  unselectedItemColor: Colors.grey.shade600,
                  items: <BottomNavigationBarItem>[
                    BottomNavigationBarItem(
                      icon: const Icon(Icons.apps),
                      label: tr(context, "apps"),
                    ),
                    BottomNavigationBarItem(
                      icon: const Icon(Icons.folder_open),
                      label: tr(context, "files"),
                    ),
                    if (app.hasToken && app.isAdmin) ...[
                      BottomNavigationBarItem(
                        icon: const Icon(Icons.monitor_heart),
                        label: tr(context, "system_information"),
                      ),
                      BottomNavigationBarItem(
                        icon: const Icon(Icons.group),
                        label: tr(context, "users"),
                      ),
                    ]
                  ],
                )
              : null,
          body: PageView(
            controller: _pageController,
            onPageChanged: (index) {
              setState(() => _selectedIndex = index);
            },
            children: <Widget>[
              if (app.hasToken) ...[const AppsList(), const DavsList()] else
                const WelcomeScreen(),
              if (app.hasToken && app.isAdmin) ...[
                const SystemInfo(),
                const UsersList(),
              ]
            ],
          ),
        );
      },
    );
  }
}
