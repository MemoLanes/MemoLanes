import 'package:flutter/material.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:project_dv/src/rust/api/utils.dart';
import 'package:project_dv/src/rust/journey_header.dart';

class JourneyUiBody extends StatelessWidget {
  const JourneyUiBody({super.key});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<JourneyHeader>>(
      future: listAllJourneys(),
      builder:
          (BuildContext context, AsyncSnapshot<List<JourneyHeader>> snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const CircularProgressIndicator();
        } else if (snapshot.hasError) {
          throw Exception(snapshot.error);
        } else if (snapshot.hasData && snapshot.data!.isNotEmpty) {
          return ListView.builder(
            itemCount: snapshot.data!.length,
            itemBuilder: (BuildContext context, int index) {
              return ListTile(
                title: Text(
                    naiveDateToString(date: snapshot.data![index].journeyDate)),
                subtitle: Text(
                    snapshot.data![index].start?.toLocal().toString() ?? ""),
              );
            },
          );
        } else {
          return Container(); // Show nothing if the list is null or empty
        }
      },
    );
  }
}
