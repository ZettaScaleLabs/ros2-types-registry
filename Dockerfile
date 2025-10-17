
FROM ros:kilted-ros-core

# Install basic tools and prerequisites
RUN apt-get update && \
    apt-get install -y \
        curl gpg ca-certificates wget \
        git build-essential cmake pkg-config \
        python3 python3-pip \
        just \
        libclang-dev \
        && rm -rf /var/lib/apt/lists/*

# Install ROS necessary packages
RUN apt-get update && \
    apt-get install -y --only-upgrade ros-kilted-* && \
    apt-get install -y \
      ros-kilted-rmw-zenoh-cpp \
      ros-kilted-demo-nodes-cpp

# Enable bash complete
RUN rm /etc/apt/apt.conf.d/docker-clean
RUN apt-get update && \
    apt-get install -y bash-completion

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy the code
WORKDIR /workspace
COPY . /workspace

# Build
RUN cargo build --release
