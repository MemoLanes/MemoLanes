import json
import os

def generate_netrc(machine, login, password):
    netrc_content = f"""
machine {machine}
login {login}
password {password}
"""

    script_dir = os.path.dirname(os.path.realpath(__file__))
    netrc_path = os.path.join(script_dir, ".netrc")
    with open(netrc_path, "w") as file:
        file.write(netrc_content)

    print(f".netrc file generated at: {netrc_path}")


def generate_gradle_token(token_content):
    script_dir = os.path.dirname(os.path.realpath(__file__))
    token_path = os.path.join(script_dir, "android/token")
    with open(token_path, "w") as file:
        file.write(token_content)
    print(f"token file generated at: {token_path}")


if __name__ == "__main__":
    data = {}
    with open(".env", 'r') as file:
        for line in file:
            line = line.strip()
            if '=' in line:
                key, value = line.split('=')
                key = key.strip()
                value = value.strip()
                data[key] = value
        generate_netrc("api.mapbox.com", "mapbox", data["MAPBOX-SDK-REGISTRY-TOKEN"])
        generate_gradle_token(data["MAPBOX-SDK-REGISTRY-TOKEN"])