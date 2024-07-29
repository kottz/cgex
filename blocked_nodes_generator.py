import os
import json
from collections import defaultdict
import argparse


def find_txt_files(root_dir):
    txt_files = {}
    for dirpath, dirnames, filenames in os.walk(root_dir):
        for filename in filenames:
            if filename.startswith('P ') and filename.endswith('.txt'):
                name = filename[2:].split('.')[0]  # Remove 'P ' and '.txt'
                txt_files[name] = os.path.join(dirpath, filename)
    return txt_files


def load_level_data(filename):
    with open(filename, 'r') as f:
        return json.load(f)


def load_blocked_nodes(filename):
    with open(filename, 'r') as f:
        content = f.read().strip()
        return json.loads(content) if content else []


def match_background_to_txt(background, txt_files):
    bg_name = background.split('.')[0]
    if '-' in bg_name:
        bg_name = bg_name.split('-')[0]
    for txt_name in txt_files:
        if txt_name.startswith(bg_name) and (len(txt_name) == len(bg_name) or txt_name[len(bg_name)] == '-'):
            return txt_name
    return None


def generate_blocked_nodes_json(root_dir, level_data_file, minify=False):
    level_data = load_level_data(level_data_file)
    txt_files = find_txt_files(root_dir)
    blocked_node_data = defaultdict(lambda: defaultdict(list))

    for level in level_data['levels']:
        for scene in level['scenes']:
            background = scene['background'].split()[-1]
            matched_txt = match_background_to_txt(background, txt_files)
            if matched_txt:
                blocked_nodes = load_blocked_nodes(txt_files[matched_txt])
                blocked_node_data[level['id']][scene['id']] = blocked_nodes

    result = {"blocked_node_data": []}
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
