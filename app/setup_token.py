import os
from collections import defaultdict 

def generate_json_token(token_content):
    import json
    script_dir = os.path.dirname(os.path.realpath(__file__))
    token_path = os.path.join(script_dir, "journey_kernel/static/token.json")
    
    # Create directory if it doesn't exist
    os.makedirs(os.path.dirname(token_path), exist_ok=True)
    
    token_data = {"MAPBOX-ACCESS-TOKEN": token_content}
    with open(token_path, "w") as file:
        json.dump(token_data, file, indent=2)
    print(f"token json file generated at: {token_path}")


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

    generate_json_token(data["MAPBOX-ACCESS-TOKEN"])
