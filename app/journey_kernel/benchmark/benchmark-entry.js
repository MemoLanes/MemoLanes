import { MapController } from "../static/map-controller";
import { installJourneyBenchmark } from "./benchmark-api";

const initializeMapController = MapController.prototype.initialize;

MapController.prototype.initialize = async function () {
  this.disableAutoRefresh();
  await initializeMapController.call(this);
  const tileProvider = this.getTileProvider();
  if (!tileProvider) {
    throw new Error("Journey tile provider was not initialized");
  }
  installJourneyBenchmark(this.getMap(), tileProvider);
};

void import("../static/index");
