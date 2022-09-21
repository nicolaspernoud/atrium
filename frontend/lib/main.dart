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
  const MyApp({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
        home: const HomePage(),
        localizationsDelegates: const [
          MyLocalizationsDelegate(),
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
        ],
        supportedLocales: const [
          Locale('en', ''),
          Locale('fr', ''),
        ],
        theme: ThemeData(
          primarySwatch: Colors.indigo,
        ));
  }
}

class HomePage extends StatefulWidget {
  const HomePage({Key? key}) : super(key: key);

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
        return Scaffold(
          bottomNavigationBar: app.hasToken
              ? BottomNavigationBar(
                  onTap: _onItemTapped,
                  currentIndex: _selectedIndex,
                  selectedItemColor: Colors.amber[900],
                  unselectedItemColor: Colors.black,
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
                        icon: const Icon(Icons.group),
                        label: tr(context, "users"),
                      ),
                      BottomNavigationBarItem(
                        icon: const Icon(Icons.monitor_heart),
                        label: tr(context, "system_information"),
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
                const UsersList(),
                const SystemInfo()
              ]
            ],
          ),
        );
      },
    );
  }
}
