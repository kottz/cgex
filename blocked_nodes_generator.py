import os
import json
from collections import defaultdict
import argparse


def find_txt_files(root_dir):
    txt_files = []
    for dirpath, dirnames, filenames in os.walk(root_dir):
        for filename in filenames:
            if filename.startswith('P ') and filename.endswith('.txt'):
                txt_files.append(os.path.join(dirpath, filename))
    return txt_files


def parse_filename(filename):
    base = os.path.basename(filename)
    name = base.split('-')[0][2:]  # Remove 'P ' and everything after '-'
    return name


def load_level_data(filename):
    with open(filename, 'r') as f:
        return json.load(f)


def find_level_and_scene(level_data, name):
    for level in level_data['levels']:
        for scene in level['scenes']:
            if f"B {name}" in scene['background']:
                print("found", level['id'], scene['id'])
                return level['id'], scene['id']
    return None, None


def load_blocked_nodes(filename):
    with open(filename, 'r') as f:
        content = f.read().strip()
        return json.loads(content) if content else []


def generate_blocked_nodes_json(root_dir, level_data_file, minify=False):
    level_data = load_level_data(level_data_file)
    txt_files = find_txt_files(root_dir)

    blocked_node_data = defaultdict(lambda: defaultdict(list))

    for file in txt_files:
        name = parse_filename(file)
        level_id, scene_id = find_level_and_scene(level_data, name)

        if level_id is not None and scene_id is not None:
            blocked_nodes = load_blocked_nodes(file)
            blocked_node_data[level_id][scene_id] = blocked_nodes

    result = {"blocked_node_data": []}

    # Sort the data
    for level_id in sorted(blocked_node_data.keys()):
        for scene_id in sorted(blocked_node_data[level_id].keys()):
            level_data = {
                "level_id": level_id,
                "scene_id": scene_id,
                "blocked_nodes": blocked_node_data[level_id][scene_id]
            }
            result["blocked_node_data"].append(level_data)

    with open('blocked_nodes.json', 'w') as f:
        if minify:
            json.dump(result, f, separators=(',', ':'))
        else:
            json.dump(result, f, indent=2)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Generate blocked_nodes.json')
    parser.add_argument('root_dir', help='Path to the game directory')
    parser.add_argument('level_data_file', help='Path to level_data.json')
    parser.add_argument('--min', action='store_true',
                        help='Output minified JSON')

    args = parser.parse_args()

    generate_blocked_nodes_json(args.root_dir, args.level_data_file, args.min)
