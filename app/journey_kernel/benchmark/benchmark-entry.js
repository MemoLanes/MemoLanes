import { MapController } from "../static/map-controller";
import { installJourneyBenchmark } from "./benchmark-api";

const initializeMapController = MapController.prototype.initialize;

MapController.prototype.initialize = async function () {
  const result = await initializeMapController.call(this);

  if (this.pollIntervalId !== null) {
    window.clearInterval(this.pollIntervalId);
    this.pollIntervalId = null;
  }

  installJourneyBenchmark(this.getMap());
  return result;
};

void import("../static/index");
