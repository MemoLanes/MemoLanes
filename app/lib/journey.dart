import 'package:flutter/material.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:project_dv/src/rust/journey_header.dart';

class JourneyUiBody extends StatelessWidget {
  const JourneyUiBody();

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<JourneyHeader>>(
      future: listAllJourneys(),
      builder:
          (BuildContext context, AsyncSnapshot<List<JourneyHeader>> snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const CircularProgressIndicator(); // Display a loading indicator while the future is being resolved
        } else if (snapshot.hasError) {
          return Text(
              'Error: ${snapshot.error}'); // Display an error message if the future completes with an error
        } else if (snapshot.hasData && snapshot.data!.isNotEmpty) {
          return ListView.builder(
            itemCount: snapshot.data!.length,
            itemBuilder: (BuildContext context, int index) {
              return ListTile(
                title: Text(snapshot.data![index].id),
                subtitle: Text(snapshot.data![index].end.toString()),
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
