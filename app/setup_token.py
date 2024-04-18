import os
from collections import defaultdict 

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
    token_path = os.path.join(script_dir, "android/mapbox-sdk-registry-token")
    with open(token_path, "w") as file:
        file.write(token_content)
    print(f"token file generated at: {token_path}")

def generate_token_dart(token_content):
    dart_content = f'Map token = {{"MAPBOX-ACCESS-TOKEN": "{token_content}"}};'
    script_dir = os.path.dirname(os.path.realpath(__file__))
    token_path = os.path.join(script_dir, "lib/token.dart")
    with open(token_path, "w") as file:
        file.write(dart_content)
    print(f"token.dart file generated at: {token_path}")

if __name__ == "__main__":
    data = {}
    env_path = ".env"
    if os.path.isfile(env_path):
        with open(".env", 'r') as file:
            for line in file:
                if '=' in line:
                    key, value = line.strip().split('=')
                    data[key.strip()] = value.strip()
    else:
        print(".env not found, generating empty files")
        data = defaultdict(str)

    generate_netrc("api.mapbox.com", "mapbox", data["MAPBOX-SDK-REGISTRY-TOKEN"])
    generate_gradle_token(data["MAPBOX-SDK-REGISTRY-TOKEN"])
    generate_token_dart(data["MAPBOX-ACCESS-TOKEN"])
