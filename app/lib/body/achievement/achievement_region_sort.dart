enum AchievementRegionSort { area, coverage }

/// Area ranking uses the list's km² doubles, then its region sort key.
int compareAchievementRegionArea({
  required double aVisitedKm2,
  required String aSortKey,
  required double bVisitedKm2,
  required String bSortKey,
}) {
  final area = bVisitedKm2.compareTo(aVisitedKm2);
  return area != 0 ? area : aSortKey.compareTo(bSortKey);
}
