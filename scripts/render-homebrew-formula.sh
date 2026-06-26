#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -ne 7 ]; then
  echo "usage: $0 <output-path> <version> <arm-url> <arm-sha256> <x86-url> <x86-sha256> <homepage>" >&2
  exit 1
fi

output_path="$1"
version="$2"
arm_url="$3"
arm_sha256="$4"
x86_url="$5"
x86_sha256="$6"
homepage="$7"

mkdir -p "$(dirname "$output_path")"

cat >"$output_path" <<EOF
class Slidecli < Formula
  desc "Terminal slide editor and presenter"
  homepage "${homepage}"
  version "${version}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "${arm_url}"
      sha256 "${arm_sha256}"
    else
      url "${x86_url}"
      sha256 "${x86_sha256}"
    end
  end

  def install
    bin.install "slidecli"
    doc.install "LICENSE"
  end
end
EOF
