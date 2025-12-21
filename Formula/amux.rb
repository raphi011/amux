# typed: false
# frozen_string_literal: true

class Amux < Formula
  desc "Terminal multiplexer for AI coding agents"
  homepage "https://github.com/raphi011/amux"
  version "0.3.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-darwin-arm64.tar.gz"
      sha256 "cd3e73a19073c235974c8fac7452b357af3e23655895f5714862d18fc9330fce" # darwin-arm64
    end
    on_intel do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-darwin-amd64.tar.gz"
      sha256 "d39009b7ced663bc7fa0d910f1f946a239385728b96e24aaa3b5c338b05f5aec" # darwin-amd64
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-linux-arm64.tar.gz"
      sha256 "1f07b3ffd381ef2acefade0300ed073a168ce56ba2fbffe708deb08e6112084e" # linux-arm64
    end
    on_intel do
      url "https://github.com/raphi011/amux/releases/download/v#{version}/amux-linux-amd64.tar.gz"
      sha256 "09d93b2a5a80f1d8177bd1999b2cdf92cefb3a10dc2d07dc7349ab146a98efd4" # linux-amd64
    end
  end

  def install
    bin.install "amux"
  end

  test do
    assert_match "amux", shell_output("#{bin}/amux --version 2>&1", 1)
  end
end
