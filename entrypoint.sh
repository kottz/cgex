#!/bin/bash

# Cleanup to be "stateless" on startup
rm -rf /var/run/pulse /var/lib/pulse /root/.config/pulse

# Start pulseaudio as system wide daemon
pulseaudio -D --verbose --exit-idle-time=-1 --system --disallow-exit

wineboot -i

Xvfb :99 -screen 0 1024x768x16 &
export DISPLAY=:99

# Xvfb needs some time to start or else we get weird silent errors
echo ""
sleep 5

# Check if RUN_BASH is set
if [ ! -z "$RUN_BASH" ]; then
    echo "RUN_BASH is set. Starting bash shell..."
    exec /bin/bash
else
    # Build the command with optional flags
    CMD="/app/target/release/cgex -i /input -o /output"
    if [ "$NO_UPSCALE" = "true" ]; then
        CMD="$CMD --no-upscale"
    fi
    if [ "$COMPRESSION" = "true" ]; then
        CMD="$CMD --compression"
    fi
    if [ "$NO_TRANSPARENT_BACKGROUND" = "true" ]; then
        CMD="$CMD --no-transparent-background"
    fi
    eval $CMD

    if [ ! -z "$HOST_UID" ] && [ ! -z "$HOST_GID" ]; then
        echo "Changing ownership of output files to $HOST_UID:$HOST_GID"
        chown -R $HOST_UID:$HOST_GID /output
    else
        echo "HOST_UID and/or HOST_GID not set. Skipping ownership change."
    fi
fi
