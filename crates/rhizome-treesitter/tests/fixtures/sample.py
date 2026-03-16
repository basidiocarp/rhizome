"""Sample Python module for testing."""

import os
from pathlib import Path


class Config:
    """Configuration class."""

    def __init__(self, name: str, value: int):
        self.name = name
        self.value = value

    def get_value(self) -> int:
        """Return the value."""
        return self.value


def process(config: Config) -> str:
    """Process a config."""
    return f"{config.name}: {config.value}"


MAX_SIZE = 1024

class Status:
    ACTIVE = "active"
    INACTIVE = "inactive"
