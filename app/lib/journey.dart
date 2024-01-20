import 'package:flutter/material.dart';

class JourneyUiBody extends StatelessWidget {
  const JourneyUiBody();

  @override
  Widget build(BuildContext context) {
    //下划线widget预定义以供复用。
    Widget divider1 = Divider(
      color: Colors.blue,
    );
    Widget divider2 = Divider(color: Colors.green);
    return ListView.separated(
      itemCount: 10,
      //列表项构造器
      itemBuilder: (BuildContext context, int index) {
        return ListTile(title: Text("$index"));
      },
      //分割器构造器
      separatorBuilder: (BuildContext context, int index) {
        return index % 2 == 0 ? divider1 : divider2;
      },
    );
  }
}
