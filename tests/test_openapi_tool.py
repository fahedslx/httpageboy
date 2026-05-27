import importlib.util
import unittest
from pathlib import Path


TOOL_PATH = Path(__file__).resolve().parents[1] / "tools" / "openapi_from_routes.py"
SPEC = importlib.util.spec_from_file_location("openapi_from_routes", TOOL_PATH)
openapi_from_routes = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
SPEC.loader.exec_module(openapi_from_routes)


class OpenApiToolTest(unittest.TestCase):
  def test_emit_openapi_from_minimal_routes(self):
    data = {
      "service": {
        "name": "api-sales",
        "version": "0.1.0",
        "server": "https://api-sales.eqeqo.pe",
      },
      "auth": {
        "headers": ["user-token", "business-id"],
      },
      "routes": [
        {
          "method": "GET",
          "path": "/products",
          "summary": "List products",
          "permission": "products.read",
          "responses": [
            {"status": "200", "description": "Product list"},
            {"status": "403", "description": "Forbidden"},
          ],
        },
      ],
    }

    output = openapi_from_routes.emit_openapi(data)

    self.assertIn("openapi: 3.0.3", output)
    self.assertIn('title: "api-sales"', output)
    self.assertIn("  /products:", output)
    self.assertIn("    get:", output)
    self.assertIn('name: "products.read"', output)
    self.assertIn('name: "user-token"', output)
    self.assertIn('"200":', output)


if __name__ == "__main__":
  unittest.main()
