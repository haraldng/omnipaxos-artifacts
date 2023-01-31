# Shell script for installation of dependencies on Ubuntu 20.04

# Add Erlang and Sbt sources
echo "deb https://dl.bintray.com/sbt/debian /" | sudo tee -a /etc/apt/sources.list.d/sbt.list
curl -sL "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x2EE0EA64E40A89B84B2DF73499E82A75642AC823" | sudo apt-key add

# Builders
sudo apt-get update
sudo apt install make
sudo apt install build-essential
sudo apt-get install zip
sudo apt-get install unzip

# JDK
sudo apt-get install default-jdk -y

# Scala
sudo wget https://downloads.lightbend.com/scala/2.13.0/scala-2.13.0.deb
sudo dpkg -i scala-2.13.0

# Sbt
sudo apt-get install apt-transport-https curl gnupg -yqq
echo "deb https://repo.scala-sbt.org/scalasbt/debian all main" | sudo tee /etc/apt/sources.list.d/sbt.list
echo "deb https://repo.scala-sbt.org/scalasbt/debian /" | sudo tee /etc/apt/sources.list.d/sbt_old.list
curl -sL "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x2EE0EA64E40A89B84B2DF73499E82A75642AC823" | sudo -H gpg --no-default-keyring --keyring gnupg-ring:/etc/apt/trusted.gpg.d/scalasbt-release.gpg --import
sudo chmod 644 /etc/apt/trusted.gpg.d/scalasbt-release.gpg
sudo apt-get install sbt

# Rustup
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly
rustup toolchain install nightly-2021-04-25
rustup toolchain install nightly-2021-04-26 --profile minimal
rustup default nightly-2021-04-26
source "$HOME/.cargo/env"

# Ammonite
sudo sh -c '(echo "#!/usr/bin/env sh" && curl -L https://github.com/com-lihaoyi/Ammonite/releases/download/2.3.8/2.12-2.3.8) > /usr/local/bin/amm && chmod +x /usr/local/bin/amm'

# Protobuf
./travis_install_protobuf.sh