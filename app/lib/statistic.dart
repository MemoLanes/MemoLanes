import 'package:flutter/material.dart';

class StatisticBody extends StatelessWidget {
  const StatisticBody({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text('Statistics'),
      ),
      body: Center(
        child: Text(
          'To be continued',
          style: TextStyle(fontSize: 24),
        ),
      ),
    );
  }
}
