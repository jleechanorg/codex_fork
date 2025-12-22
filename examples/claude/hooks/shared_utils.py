#!/usr/bin/env python3
"""
Shared utilities for hook implementations to eliminate code duplication
"""
import json
import sys
from datetime import datetime
from typing import Dict, Any, Optional


class HookRunner:
    """Base class for command-line hook execution with common patterns"""

    def __init__(self, hook_type: str):
        self.hook_type = hook_type

    def read_payload(self) -> Optional[Dict[str, Any]]:
        """Read and parse JSON payload from stdin"""
        try:
            input_data = sys.stdin.read().strip()
            if not input_data:
                return None
            return json.loads(input_data)
        except json.JSONDecodeError as e:
            self.log_error(f"Failed to parse JSON payload: {e}")
            return None

    def get_common_fields(self, payload: Dict[str, Any]) -> Dict[str, str]:
        """Extract common fields used by all hooks"""
        return {
            "timestamp": datetime.now().isoformat(),
            "session_id": payload.get("session_id", "unknown"),
        }

    def log_message(self, message: str) -> None:
        """Log a hook message to stdout"""
        print(f"[{self.hook_type} Hook] {message}")

    def log_error(self, error: str) -> None:
        """Log an error message"""
        print(f"[{self.hook_type} Hook Error] {error}")

    def output_feedback(self, feedback: Dict[str, Any]) -> None:
        """Output structured feedback as JSON"""
        print(json.dumps(feedback))

    def handle_error(self, error: Exception) -> None:
        """Handle and output error in standard format"""
        error_data = {
            "hook_executed": False,
            "hook_type": self.hook_type,
            "error": str(error),
            "status": "failed"
        }
        self.log_error(str(error))
        self.output_feedback(error_data)

    def run(self) -> None:
        """Main execution method - override in subclasses"""
        payload = self.read_payload()
        if payload is None:
            return

        try:
            common_fields = self.get_common_fields(payload)
            result = self.process_hook(payload, common_fields)
            self.output_feedback(result)
        except Exception as e:
            self.handle_error(e)

    def process_hook(self, payload: Dict[str, Any], common_fields: Dict[str, str]) -> Dict[str, Any]:
        """Override this method in specific hook implementations"""
        raise NotImplementedError("Subclasses must implement process_hook method")