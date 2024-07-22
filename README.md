# cgex
cgex is a tool for extracting assets from Macromedia/Adobe Director games. Currently supports:

- Jönssonligan: Jakten på Mjölner

## Usage

### Windows

Step 1:
```bash
git clone https://github.com/kottz/cgex
cd cgex
mkdir disc_contents
mkdir output
```
2. Insert game CD or mount your .iso and copy everything on the CD into the `disc_contents` folder.
3. Run the program with `cargo run --release`
4. Assets can then be found in the `output` folder.

### Linux (using Docker)

1. Clone the repository
2. Insert game CD or mount your .iso and copy everything on the CD into the `disc_contents` folder.
3. Create an `output` folder for extracted assets.
4. cd into project folder.
5. Run the docker command below.

```bash
git clone https://github.com/kottz/cgex
cd cgex
mkdir disc_contents
mkdir output
docker run --rm \
  -v ./disc_contents:/input:ro \
  -v ./output:/output \
  -e HOST_UID=$(id -u) \
  -e HOST_GID=$(id -g) \
  kottz/cgex:latest
```

Extracted assets will be placed in the `output` folder, organized by type and game area. Extraction process may take a long time depending on your system.

## Legal

This tool is for personal use only. Ensure you have the right to extract and use game assets in your region.

## TODO
- Provide an executable for windows.
- Add support for Jönssonligan: Går På Djupet
