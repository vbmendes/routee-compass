from unittest import TestCase

from compass.road_network.base import PathWeight
from compass.road_network.tomtom_networkx import TomTomNetworkX
from compass.utils.geo_utils import Coordinate
from tests import test_dir


class TestTomTomRoadNetworkDual(TestCase):
    def setUp(self) -> None:
        self.road_network_file = test_dir() / "test_assets" / "denver_downtown_tomtom_network_dual.pickle"
        self.road_network = TomTomNetworkX(self.road_network_file)

        self.home_plate = Coordinate(lat=39.754372, lon=-104.994300)
        self.bk_lounge = Coordinate(lat=39.779098, lon=-104.951241)

    def test_shortest_path_distance(self):
        path, _ = self.road_network.shortest_path(self.home_plate, self.bk_lounge, weight=PathWeight.DISTANCE)
        start = path[0]
        end = path[-1]

        self.assertAlmostEqual(start.lat, self.home_plate.lat, places=2)
        self.assertAlmostEqual(start.lon, self.home_plate.lon, places=2)
        self.assertAlmostEqual(end.lat, self.bk_lounge.lat, places=2)
        self.assertAlmostEqual(end.lon, self.bk_lounge.lon, places=2)

    def test_shortest_path_time(self):
        # TODO: how can we actually test this is the shortest time route? -ndr

        path, _ = self.road_network.shortest_path(self.home_plate, self.bk_lounge, weight=PathWeight.TIME)
        start = path[0]
        end = path[-1]

        self.assertAlmostEqual(start.lat, self.home_plate.lat, places=2)
        self.assertAlmostEqual(start.lon, self.home_plate.lon, places=2)
        self.assertAlmostEqual(end.lat, self.bk_lounge.lat, places=2)
        self.assertAlmostEqual(end.lon, self.bk_lounge.lon, places=2)

    def test_shortest_path_energy(self):
        # TODO: how can we actually test this is the shortest energy route? -ndr

        path, _ = self.road_network.shortest_path(self.home_plate, self.bk_lounge, weight=PathWeight.ENERGY)
        start = path[0]
        end = path[-1]

        self.assertAlmostEqual(start.lat, self.home_plate.lat, places=2)
        self.assertAlmostEqual(start.lon, self.home_plate.lon, places=2)
        self.assertAlmostEqual(end.lat, self.bk_lounge.lat, places=2)
        self.assertAlmostEqual(end.lon, self.bk_lounge.lon, places=2)