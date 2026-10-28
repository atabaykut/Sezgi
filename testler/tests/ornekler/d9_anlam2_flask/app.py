from functools import wraps

from flask import Flask, jsonify, request

app = Flask(__name__)


def require_device_key(f):
    @wraps(f)
    def wrapper(*args, **kwargs):
        key = request.headers.get("X-Device-Key")
        if not key:
            return jsonify({"error": "missing key"}), 401
        return f(*args, **kwargs)

    return wrapper


@app.route("/api/readings", methods=["POST"])
@require_device_key
def create_reading():
    return jsonify({}), 201


@app.route("/api/devices")
def list_devices():
    return jsonify([])
