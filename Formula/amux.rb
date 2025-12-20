# typed: false
# frozen_string_literal: true

class Amux < Formula
  desc "Terminal multiplexer for AI coding agents"
  homepage "https://github.com/raphi011/amux"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-darwin-arm64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # darwin-arm64
    end
    on_intel do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-darwin-amd64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # darwin-amd64
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-linux-arm64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # linux-arm64
    end
    on_intel do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-linux-amd64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # linux-amd64
    end
  end

  def install
    bin.install "amux"
  end

  test do
    assert_match "amux", shell_output("#{bin}/amux --version 2>&1", 1)
  end
end
