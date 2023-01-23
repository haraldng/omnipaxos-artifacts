#!/bin/bash
set -ex
VERSION="3.9.1"
OSX_VERSION="3.9"
NAME="protoc-$VERSION-linux-x86_64.zip"
URL="https://github.com/protocolbuffers/protobuf/releases/download/v$VERSION/$NAME"

if [ $TRAVIS_OS_NAME = 'osx' ]; then
	launchctl limit maxfiles;
	sysctl kern.maxfiles;
	sysctl kern.maxfilesperproc;
	ulimit -n 8192 8192; # max is apparently 10240
	ulimit -n; # verify result
    # Install protobuf on macOS
    (brew install protobuf@$OSX_VERSION && brew link --force --overwrite protobuf@$OSX_VERSION) || brew upgrade protobuf
else
    # Install protobuf on Linux
    ulimit -n; # just for reference
    # Make sure you grab the latest version
	wget "$URL"

	# Unzip
	unzip "$NAME" -d protoc3

	# Move protoc to /usr/local/bin/
	sudo mv protoc3/bin/* /usr/local/bin/

	# Move protoc3/include to /usr/local/include/
	sudo mv protoc3/include/* /usr/local/include/
fi
