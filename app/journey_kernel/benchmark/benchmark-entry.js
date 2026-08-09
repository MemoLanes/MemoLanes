import { MapController } from "../static/map-controller";
import { installJourneyBenchmark } from "./benchmark-api";

const initializeMapController = MapController.prototype.initialize;

MapController.prototype.initialize = async function () {
  this.disableAutoRefresh();
  await initializeMapController.call(this);
  installJourneyBenchmark(this.getMap());
};

void import("../static/index");
