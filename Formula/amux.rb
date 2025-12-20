# typed: false
# frozen_string_literal: true

class Amux < Formula
  desc "Terminal multiplexer for AI coding agents"
  homepage "https://github.com/raphi011/amux"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-darwin-arm64.tar.gz"
      sha256 "13202caf5ffb74d2c0dac880326e2374985a2ce1d34ed11ed32bcc6e5f260b47" # darwin-arm64
    end
    on_intel do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-darwin-amd64.tar.gz"
      sha256 "8bb037f0b606b598aea8d2f1552fe5a10b4fdb683047b4d1b2315ebb3e8cc90b" # darwin-amd64
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-linux-arm64.tar.gz"
      sha256 "1c5894e74c22ac9f7917d9077b99de59564ac9d566ad569c05b964af28513c15" # linux-arm64
    end
    on_intel do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-linux-amd64.tar.gz"
      sha256 "fb5a0b1252b4a437ec5bd3cd94007b14abedc7044bad834acdcaaa917e1ff963" # linux-amd64
    end
  end

  def install
    bin.install "amux"
  end

  test do
    assert_match "amux", shell_output("#{bin}/amux --version 2>&1", 1)
  end
end
