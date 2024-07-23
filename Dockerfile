FROM rust:1.79-bookworm

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update -y && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        wget \
        xdotool \
        gnupg2 \
        xvfb \
        pulseaudio \
        alsa-utils

# Install Wine
RUN dpkg --add-architecture i386 && \
    mkdir -pm755 /etc/apt/keyrings && \
    wget -O /etc/apt/keyrings/winehq-archive.key https://dl.winehq.org/wine-builds/winehq.key && \
    echo "deb [signed-by=/etc/apt/keyrings/winehq-archive.key] https://dl.winehq.org/wine-builds/debian/ bookworm main" > /etc/apt/sources.list.d/winehq.list && \
    apt-get update -y && \
    apt-get install -y --install-recommends winehq-stable

# Clean up
RUN apt-get clean && \
    rm -rf /var/lib/apt/lists/*

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# Set up Wine environment variables
ENV WINEDEBUG=-all
ENV WINEPREFIX=/root/.wine


# Add root user to pulse-access group
RUN adduser root pulse-access

# Create app directory
RUN mkdir -p /app
WORKDIR /app

# Copy extractor_tools into the image
COPY extractor_tools /app/extractor_tools

# Copy only the necessary Rust files
COPY Cargo.toml ./
COPY src ./src

# Build the Rust project
RUN cargo build --release

# Create entrypoint script
COPY entrypoint.sh /opt/bin/entrypoint.sh
RUN chmod +x /opt/bin/entrypoint.sh

ENTRYPOINT ["/opt/bin/entrypoint.sh"]
