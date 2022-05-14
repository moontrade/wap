
FROM ubuntu:22.04

MAINTAINER Moontrade LLC <info@moontrade.io>

WORKDIR /root

ENV TZ=GMT
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        git \
        ssh \
        libssl-dev \
    	pkg-config \
    	cmake \
    	libtool \
    	autoconf \
    	automake \
    	autotools-dev \
    	xutils-dev \
    	ninja-build \
    	zlib1g-dev \
    	podman \
    	docker \
    	certbot \
    	llvm-12 \
        golang-go && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# install toolchain
RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain stable -y

ENV PATH=/root/.cargo/bin:$PATH